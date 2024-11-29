use super::*;
use crate::hash::*;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use circula_list::CircularList;
use msg::Message;
use std::fs;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

// Represents a node in the Chord ring network
#[derive(Debug, Clone)]
pub struct ChordRing {
    nodes: Arc<Mutex<CircularList<String>>>,
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
            nodes: Arc::new(Mutex::new(CircularList::new())),
            size: 2_usize.pow(*M as u32),
            tx,
            logs: Arc::new(Mutex::new(Vec::new())),
            last_used_index: Arc::new(Mutex::new(0)),
        };

        // Spawn a new thread to handle incoming messages
        let chord_ring_clone = chord_ring.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    Message::ReqKnownNode { node_id } => {
                        log_message!(chord_ring_clone, "Join request from node {}", node_id);
                        chord_ring_clone.handle_known_node_req(node_id).await;
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
                    _ => {}
                }
            }
        });

        chord_ring
    }

    async fn handle_message(&self, msg: Message) -> impl Responder {
        log_message!(self, "Handling message: {:?}", msg);

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
        let port = *PORT;
        println!("Running ChordRing server on http://{}:{}", "0.0.0.0", port);
        log_message!(self, "Running ChordRing server on port: {}", port);

        let app_state = AppState {
            logs: self.logs.clone(),
            nodes: self.nodes.clone(),
        };

        let chord_ring = self.clone();

        HttpServer::new(move || {
            let chord_ring = chord_ring.clone();
            App::new()
                .app_data(web::Data::new(app_state.clone()))
                .route("/", web::get().to(handle_index))
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
        .bind(("0.0.0.0", port))?
        .run()
        .await?;

        Ok((String::from("0.0.0.0"), port))
    }
}

impl ChordRing {
    async fn handle_known_node_req(&self, node: String) {
        log_message!(self, "Handling known node request from node: {}", node);

        // Check if ring is full
        if self.nodes.lock().await.len() == self.size {
            log_message!(self, "Ring is full. Cannot add more nodes.");
            send_post_request!(&format!("http://{}/msg", node), Message::RingIsFull);
            return;
        }

        // Check if node already exists in the ring
        if self.nodes.lock().await.contains(&node) {
            log_message!(self, "Node already exists in the ring");
            send_post_request!(&format!("http://{}/msg", node), Message::NodeExists);
            return;
        }

        let node_to_join = {
            let mut index = self.last_used_index.lock().await;
            let nodes = self.nodes.lock().await;

            if nodes.is_empty() {
                node.clone()
            } else {
                *index = (*index + 1) % nodes.len();
                nodes.get(*index).unwrap().clone()
            }
        };
        send_post_request!(
            &format!("http://{}/msg", node),
            Message::ResKnownNode {
                node_id: node_to_join.clone()
            }
        );

        // Log the response node id
        log_message!(
            self,
            "Sent ResKnownNode with node_id: {} to node: {}",
            node_to_join,
            node
        );

        // Add the new node to the ring
        self.nodes.lock().await.push_back(node);
    }

    pub async fn req_known_node(&self, node: String) {
        log_message!(self, "Requesting known node to join: {}", node);

        let _ = self.handle_known_node_req(node).await;
    }
}

// State shared across HTTP handlers
#[derive(Clone)]
struct AppState {
    logs: Arc<Mutex<Vec<String>>>,
    nodes: Arc<Mutex<CircularList<String>>>,
}

// Handler for the index route
async fn handle_index(state: web::Data<AppState>) -> impl Responder {
    let nodes_html = state
        .nodes
        .lock()
        .await
        .iter()
        .map(|node| {
            format!(
                "<li><a href=\"http://{0}\">{0} ({1})</a></li>",
                node,
                hash(&node)
            )
        })
        .collect::<String>();

    let logs_html = state
        .logs
        .lock()
        .await
        .iter()
        .map(|log| format!("<li>{}</li>", log))
        .collect::<String>();

    let html_template = fs::read_to_string(HTML_PATH).expect("Unable to read template file");

    let html = html_template
        .replace("{nodes}", &nodes_html)
        .replace("{logs}", &logs_html);

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}
