use super::*;
use crate::hash::*;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use circula_list::CircularList;
use msg::Message;
use std::fs;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

// Represents a node in the Chord ring network
#[derive(Debug, Clone)]
pub struct ChordRing {
    nodes: Arc<Mutex<CircularList<String>>>,
    size: usize,
    tx: mpsc::Sender<Message>,
    logs: Arc<Mutex<Vec<String>>>,
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
        };

        // Spawn a new thread to handle incoming messages
        let chord_ring_clone = chord_ring.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    Message::ReqKnownNode { node_id } => {
                        #[cfg(debug_assertions)]
                        println!("(DHT)Join request from node {}", node_id);
                        chord_ring_clone.logs.lock().unwrap().push(format!(
                            "{}: <span class=\"font-semibold\">Join request from node {}</span>",
                            chrono::Utc::now(),
                            node_id
                        ));
                        chord_ring_clone.handle_known_node_req(node_id).await;
                    }
                    Message::Kys => {
                        #[cfg(debug_assertions)]
                        println!("(DHT)Kys message received");
                        chord_ring_clone.logs.lock().unwrap().push(format!(
                            "{}: <span class=\"font-semibold\">(DHT)Kys message received</span>",
                            chrono::Utc::now()
                        ));
                        // drop the receiver to exit the loop
                        drop(rx);
                        break;
                    }
                    _ => {}
                }
            }
        });

        chord_ring
    }

    async fn handle_message(&self, msg: Message) -> impl Responder {
        #[cfg(debug_assertions)]
        println!("(DHT)Handling message: {:?}", msg);
        self.logs.lock().unwrap().push(format!(
            "{}: <span class=\"font-semibold\">(DHT)Handling message: {:?}</span>",
            chrono::Utc::now(),
            msg
        ));

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

        #[cfg(debug_assertions)]
        println!("Running ChordRing server on port: {}", port);
        self.logs.lock().unwrap().push(format!(
            "{}: <span class=\"font-semibold\">Running ChordRing server on port: {}</span>",
            chrono::Utc::now(),
            port
        ));

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
        #[cfg(debug_assertions)]
        println!("(DHT)Handling known node request from node: {}", node);
        self.logs.lock().unwrap().push(format!(
            "{}: <span class=\"font-semibold\">Handling known node request from node: {}</span>",
            chrono::Utc::now(),
            node
        ));

        // Check if ring is full
        if self.nodes.lock().unwrap().len() == self.size {
            #[cfg(debug_assertions)]
            println!("(DHT)Ring is full. Cannot add more nodes.");
            self.logs.lock().unwrap().push(format!(
                "{}: <span class=\"font-semibold\">Ring is full. Cannot add more nodes.</span>",
                chrono::Utc::now()
            ));
            send_post_request!(&format!("http://{}/msg", node), Message::RingIsFull);
            return;
        }

        // Check if node already exists in the ring
        if self.nodes.lock().unwrap().contains(&node) {
            #[cfg(debug_assertions)]
            println!("(DHT)Node already exists in the ring");
            send_post_request!(&format!("http://{}/msg", node), Message::NodeExists);
            return;
        }

        let node_to_join = {
            let nodes = self.nodes.lock().unwrap();

            // If the ring is empty, the node to join is the node itself
            if nodes.is_empty() {
                node.clone()
            } else {
                // Otherwise, the node to join is the first node in the ring
                nodes.front().unwrap().clone()
            }
        };
        send_post_request!(
            &format!("http://{}/msg", node),
            Message::ResKnownNode {
                node_id: node_to_join
            }
        );

        // Add the new node to the ring
        self.nodes.lock().unwrap().push_back(node);
        // rotate the ring to the left
        self.nodes.lock().unwrap().rotate_left(); // this is to implement the round-robin strategy
    }

    pub async fn req_known_node(&self, node: String) {
        #[cfg(debug_assertions)]
        println!("(DHT)Requesting node to join: {}", node);
        self.logs.lock().unwrap().push(format!(
            "{}: <span class=\"font-semibold\">{}: Requesting known node to join</span>",
            chrono::Utc::now(),
            node
        ));

        // let _ = self.tx.send(Message::ReqJoin { node_id: node }).await;
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
        .unwrap()
        .iter()
        .map(|node| {
            format!(
                "<li><a href=\"http://{0}\">{0} ({1})</a></li>",
                node,
                hash(node)
            )
        })
        .collect::<String>();

    let logs_html = state
        .logs
        .lock()
        .unwrap()
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
