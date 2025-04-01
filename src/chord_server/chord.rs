use super::*;
use crate::hash::*;
use actix_multipart::Multipart;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use circula_buffer::CircularBuffer;
use data::Data;
use futures::{StreamExt, TryStreamExt};
use msg::Message;
use std::sync::Arc;
use std::{fs, io::Write};
use tokio::sync::{mpsc, Mutex};

// Represents a node in the Chord ring network
#[derive(Debug, Clone)]
pub struct ChordRing {
    nodes: Arc<Mutex<CircularBuffer<String>>>,
    size: usize,
    tx: mpsc::Sender<Message>,
    logs: Arc<Mutex<Vec<String>>>,
    last_used_index: Arc<Mutex<usize>>,
}

// Trait defining the core functionality for ChordRing
pub trait ChordRingInterface {
    fn new() -> Self;
    async fn run(&self) -> std::io::Result<(String, u16)>;
    async fn handle_message(&self, msg: Message) -> impl Responder;
}

impl ChordRingInterface for ChordRing {
    fn new() -> Self {
        let (tx, mut rx) = mpsc::channel(*DEFAULT_CHANNEL_SIZE);

        #[cfg(debug_assertions)]
        println!("Creating a new ChordRing");

        let chord_ring = ChordRing {
            nodes: Arc::new(Mutex::new(CircularBuffer::new())),
            size: 2_usize.pow(*M as u32),
            tx,
            logs: Arc::new(Mutex::new(Vec::new())),
            last_used_index: Arc::new(Mutex::new(0)),
        };

        // Spawn a new thread to check nodes every 30 seconds
        let chord_ring_clone = chord_ring.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30)); // Check every 30 seconds

            loop {
                interval.tick().await;

                // Get a copy of current nodes to check
                let nodes_to_check = {
                    let nodes = chord_ring_clone.nodes.lock().await;
                    nodes.iter().cloned().collect::<Vec<String>>()
                };

                // Check each node
                for node in nodes_to_check {
                    let tx = chord_ring_clone.tx.clone();

                    // Send CheckNode message through the channel
                    if let Err(e) = tx
                        .send(Message::CheckNode {
                            node_id: node.clone(),
                        })
                        .await
                    {
                        log_message!(
                            chord_ring_clone,
                            "Failed to send CheckNode message for node {}: {}",
                            node,
                            e
                        );
                    }
                }
            }
        });

        // Spawn a new thread to handle incoming messages
        let chord_ring_clone = chord_ring.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    Message::CheckNode { node_id } => {
                        let mut nodes = chord_ring_clone.nodes.lock().await;
                        if nodes.contains(&node_id) {
                            let node_id_clone = node_id.clone();
                            match send_post_request!(
                                &format!("http://{}/msg", node_id),
                                Message::Ping
                            ) {
                                Ok(_) => {}
                                Err(_) => {
                                    log_message!(
                                        chord_ring_clone,
                                        "Failed to send Ping message to node {}",
                                        node_id
                                    );
                                    // Find position first, then use it to remove if found
                                    let pos = nodes.iter().position(|n| n == &node_id_clone);
                                    if let Some(index) = pos {
                                        nodes.remove(index);
                                    }
                                }
                            }
                        } else {
                            log_message!(
                                chord_ring_clone,
                                "Node {} is not present in the ring",
                                node_id
                            );
                        }
                    }
                    Message::ReqKnownNode { node_id } => {
                        log_message!(chord_ring_clone, "Join request from node {}", node_id);
                        chord_ring_clone.handle_known_node_req(node_id).await;
                    }
                    Message::ResKnownNode { node_id } => {
                        log_message!(
                            chord_ring_clone,
                            "ResKnownNode message received from node {}",
                            node_id
                        );
                        let mut nodes = chord_ring_clone.nodes.lock().await;
                        nodes.push_back(node_id);
                    }
                    Message::Leave { node_id } => {
                        log_message!(
                            chord_ring_clone,
                            "Leave message received from node {}",
                            node_id
                        );
                        let mut nodes = chord_ring_clone.nodes.lock().await;

                        let index_to_remove = nodes.iter().position(|n| n == &node_id);
                        if let Some(index) = index_to_remove {
                            nodes.remove(index);
                        }
                    }
                    Message::LookupRes { key, hops, data } => {
                        log_message!(
                            chord_ring_clone,
                            "Lookup response received for key: {} ({} hops)",
                            key,
                            hops
                        );
                        log_message!(
                            chord_ring_clone,
                            "Data found: {:?}",
                            data.clone().unwrap_or_default()
                        );
                    }
                    _ => {}
                }
            }
        });

        chord_ring
    }

    async fn handle_message(&self, msg: Message) -> impl Responder {
        let tx = self.tx.clone();
        if let Err(err) = tx.send(msg).await {
            return HttpResponse::InternalServerError().json(serde_json::json!(
                Message::ErrorMessage {
                    error: err.to_string(),
                }
            ));
        }
        HttpResponse::Ok().json(serde_json::json!(Message::Success {
            message: "Message sent successfully".to_string(),
        }))
    }

    async fn run(&self) -> std::io::Result<(String, u16)> {
        print!(
            "\x1b[2K\r\x1b[33mRunning ChordRing server on \x1b[35mhttp://{}:{}\x1b[0m\n",
            IP.clone(),
            *PORT
        );
        std::io::stdout().flush().unwrap();
        log_message!(self, "Running ChordRing server on port: {}", *PORT);

        let app_state = AppState {
            logs: self.logs.clone(),
            nodes: self.nodes.clone(),
            tx: Some(self.tx.clone()),
        };

        let chord_ring = self.clone();

        HttpServer::new(move || {
            let chord_ring = chord_ring.clone();
            App::new()
                .app_data(web::Data::new(app_state.clone()))
                .route("/", web::get().to(handle_index))
                .route("/data", web::get().to(data))
                .route("/upload", web::post().to(handle_upload))
                .route("/lookup", web::post().to(handle_lookup))
                .route(
                    "/msg",
                    web::post().to(move |msg: web::Json<Message>| {
                        let chord_ring = chord_ring.clone();
                        async move {
                            chord_ring.handle_message(msg.into_inner()).await;
                            HttpResponse::Ok().body("Message handled")
                        }
                    }),
                )
        })
        .bind((IP.clone(), *PORT))?
        .run()
        .await?;

        Ok((String::from(IP.clone()), *PORT))
    }
}

impl ChordRing {
    async fn handle_known_node_req(&self, node: String) {
        log_message!(self, "Handling known node request from node: {}", node);

        // Check if ring is full
        if self.nodes.lock().await.len() == self.size {
            log_message!(self, "Ring is full. Cannot add more nodes.");

            match send_post_request!(&format!("http://{}/msg", node), Message::RingIsFull) {
                Ok(_) => {}
                Err(_) => {
                    log_message!(self, "Failed to send RingIsFull message to node: {}", node);
                }
            }
            return;
        }

        // Check if node already exists in the ring
        if self.nodes.lock().await.contains(&node) {
            log_message!(self, "Node already exists in the ring");

            match send_post_request!(&format!("http://{}/msg", node), Message::NodeExists) {
                Ok(_) => {}
                Err(_) => {
                    log_message!(self, "Failed to send NodeExists message to node: {}", node);
                }
            }
            return;
        }

        let node_to_join = {
            let mut index = self.last_used_index.lock().await;
            let mut nodes = self.nodes.lock().await;

            if nodes.is_empty() {
                nodes.push_back(node.clone());
                node.clone()
            } else {
                *index = (*index + 1) % nodes.len();
                nodes.get(*index).unwrap().clone()
            }
        };

        match send_post_request!(
            &format!("http://{}/msg", node),
            Message::ResKnownNode {
                node_id: node_to_join.clone()
            }
        ) {
            Ok(_) => {
                log_message!(
                    self,
                    "Sent ResKnownNode with node_id: {} to node: {}",
                    node_to_join,
                    node
                );
            }
            Err(_) => {
                log_message!(
                    self,
                    "Failed to send ResKnownNode with node_id: {} to node: {}",
                    node_to_join,
                    node
                );
            }
        }
    }
}

async fn handle_lookup(
    state: web::Data<AppState>,
    lookup_req: web::Json<serde_json::Value>,
) -> impl Responder {
    let key = match lookup_req.get("key") {
        Some(k) => k.as_str().unwrap_or_default().to_string(),
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "message": "Missing key in request"
            }));
        }
    };

    let mut nodes = state.nodes.lock().await;

    if nodes.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "No nodes available"
        }));
    }

    // Get first node and rotate list
    let node = match nodes.front() {
        Some(n) => n.clone(),
        None => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": "Failed to get node from ring"
            }));
        }
    };
    nodes.rotate();

    // Send lookup request to the node
    match send_post_request!(
        &format!("http://{}/msg", node),
        Message::LookupReq {
            key: key.clone(),
            hops: 0
        }
    ) {
        Ok(_) => {}
        Err(_) => {
            // Send CheckNode message through the channel when lookup fails
            if let Some(tx) = state.tx.as_ref() {
                if let Err(e) = tx
                    .send(Message::CheckNode {
                        node_id: node.clone(),
                    })
                    .await
                {
                    log_message!(
                        state,
                        "Failed to send CheckNode message after lookup failure: {}",
                        e
                    );
                }
            }

            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": "Failed to send lookup request to node"
            }));
        }
    }

    log_message!(
        state,
        "Lookup request for key '{}' sent to node {}",
        key,
        node
    );

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "message": format!("Lookup request for key '{}' sent to node {}", key, node),
        "node": node
    }))
}

async fn handle_upload(state: web::Data<AppState>, mut payload: Multipart) -> impl Responder {
    log_message!(state, "Started handling file upload");

    // Collect CSV data
    let mut csv_data = String::new();
    while let Ok(Some(mut field)) = payload.try_next().await {
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            csv_data.push_str(std::str::from_utf8(&data).unwrap());
        }
    }

    log_message!(state, "Finished reading CSV data");

    // Split into lines and remove header
    let lines: Vec<String> = csv_data.lines().skip(1).map(|s| s.to_string()).collect();
    log_message!(state, "Processed {} lines from CSV", lines.len());

    // Get available nodes
    let nodes = state.nodes.lock().await;
    let node_count = nodes.len();

    if node_count == 0 {
        log_message!(state, "Error: No nodes available in the network");
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "No nodes available in the network"
        }));
    }

    log_message!(state, "Found {} available nodes", node_count);

    // Convert CSV lines to Data objects
    let data_objects: Vec<Data> = lines
        .iter()
        .map(|line| {
            let mut parts = line.splitn(2, ',');
            let key = parts.next().unwrap_or("").trim().to_string();
            let value = parts.next().unwrap_or("").trim().to_string();
            Data { key, value }
        })
        .collect();

    log_message!(
        state,
        "Converted CSV data to {} Data objects",
        data_objects.len()
    );

    let mut success_count = 0;
    let mut error_count = 0;

    // Send data to all nodes
    for node in nodes.iter() {
        match send_post_request!(&format!("http://{}/insert", node), data_objects.clone()) {
            Ok(_) => {
                success_count += 1;
            }
            Err(_) => {
                error_count += 1;
                log_message!(state, "Failed to send data to node {}", node);
            }
        }
    }

    log_message!(
        state,
        "Completed upload - {} successful nodes, {} failed nodes",
        success_count,
        error_count
    );

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "message": format!("Data sent to {} nodes successfully ({} failed)", success_count, error_count)
    }))
}

// State shared across HTTP handlers
#[derive(Clone)]
struct AppState {
    logs: Arc<Mutex<Vec<String>>>,
    nodes: Arc<Mutex<CircularBuffer<String>>>,
    tx: Option<mpsc::Sender<Message>>, // Add this field
}

// Handler for the index route
async fn handle_index() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(fs::read_to_string(HTML_PATH).expect("Unable to read template file"))
}

async fn data(state: web::Data<AppState>) -> impl Responder {
    let nodes_lock = state.nodes.lock().await;
    let logs_lock = state.logs.lock().await;

    let node_count = nodes_lock.len();
    let log_count = logs_lock.len();

    let nodes = nodes_lock
        .iter()
        .map(|node| {
            serde_json::json!({
                "id": node.to_string(),
                "hash": hash(node)
            })
        })
        .collect::<Vec<_>>();

    let logs = logs_lock
        .iter()
        .map(|log| log.to_string())
        .collect::<Vec<_>>();

    let response = serde_json::json!({
        "nodes": nodes,
        "logs": logs,
        "node_count": node_count,
        "log_count": log_count
    });

    HttpResponse::Ok()
        .content_type("application/json")
        .json(response)
}
