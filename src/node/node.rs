use std::{
    fs,
    sync::{atomic::AtomicBool, Condvar},
};

use finger_table::finger_table::FingerTable;
use reqwest::Client;

use super::*;

#[derive(Debug)]
pub struct Node {
    db: Mutex<Connection>,
    pub node_state: Arc<Mutex<NodeState>>,
    tx: mpsc::Sender<Message>,
    logs: Arc<Mutex<Vec<String>>>,
    lock: Arc<Mutex<bool>>,
}

impl Node {
    pub fn new(port: Option<u16>) -> web::Data<Node> {
        let conn = in_mem_db();
        let node_id = match port {
            Some(p) => format!("0.0.0.0:{}", p),
            None => format!("0.0.0.0:3000"),
        };
        let node_state = Arc::new(Mutex::new(NodeState::new(node_id.clone())));
        // set node state successor and predecessor to itself

        let (tx, mut rx) = mpsc::channel(*DEFAULT_CHANNEL_SIZE);

        let app_state = web::Data::new(Node {
            db: Mutex::new(conn),
            node_state: node_state.clone(),
            tx: tx.clone(),
            logs: Arc::new(Mutex::new(Vec::new())),
            lock: Arc::new(Mutex::new(false)),
        });

        // Spawn a task to handle messages from the channel
        let node_state_clone = node_state.clone();
        let app_state_clone = app_state.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                // if the lock is set, wait for it to be released
                let lock = app_state_clone.lock.clone();
                let mut locked = lock.lock().await;
                while *locked {
                    locked = app_state_clone.lock.lock().await;
                }

                match message {
                    Message::UpdateNodeState { new_state } => {
                        let mut ns = node_state_clone.lock().await;
                        *ns = new_state;
                        log_message!(app_state_clone, "Node state updated");
                    }
                    Message::ResKnownNode { node_id } => {
                        let mut ns = node_state_clone.lock().await;
                        if node_id != ns.id {
                            send_post_request!(
                                &format!("http://{}/msg", node_id),
                                Message::ReqJoin {
                                    node_id: ns.id.clone()
                                }
                            );
                        } else {
                            log_message!(
                                app_state_clone,
                                "Node is currently the only node in the ring"
                            );
                        }
                    }
                    Message::ReqJoin { node_id } => {
                        // Handle join request
                        log_message!(app_state_clone, "Join request from node {}", node_id);

                        let ns = node_state_clone.lock().await.clone();
                        let hash_node_id = hash(&ns.id);
                        let hash_successor_id = hash(&ns.successor.as_ref().unwrap());

                        // Determine the appropriate successor for the joining node
                        if hash_node_id < hash_successor_id {
                            log_message!(
                                app_state_clone,
                                "Current node's hash is less than its successor's hash"
                            );
                            // Case 1: The current node's hash is less than its successor's hash
                            if hash_node_id < hash(&node_id) && hash(&node_id) <= hash_successor_id
                            {
                                log_message!(
                                    app_state_clone,
                                    "Joining node's hash falls between the current node and its successor"
                                );
                                // The joining node's hash falls between the current node and its successor
                                send_post_request!(
                                    &format!("http://{}/msg", node_id),
                                    Message::ResJoin {
                                        node_id: ns.successor.as_ref().unwrap().clone()
                                    }
                                );
                            } else {
                                // The joining node's hash does not fall between the current node and its successor
                                log_message!(
                                    app_state_clone,
                                    "Joining node's hash does not fall between the current node and its successor"
                                );
                                send_post_request!(
                                    &format!("http://{}/msg", ns.successor.as_ref().unwrap()),
                                    Message::ReqJoin {
                                        node_id: node_id.clone()
                                    }
                                );
                            }
                        } else {
                            // Case 2: The current node's hash is greater than its successor's hash (wrap-around case)
                            log_message!(
                                app_state_clone,
                                "Current node's hash is greater than its successor's hash"
                            );
                            if hash_node_id < hash(&node_id) || hash(&node_id) <= hash_successor_id
                            {
                                // The joining node's hash falls between the current node and its successor, considering wrap-around
                                log_message!(
                                    app_state_clone,
                                    "Joining node's hash falls between the current node and its successor, considering wrap-around"
                                );
                                send_post_request!(
                                    &format!("http://{}/msg", node_id),
                                    Message::ResJoin {
                                        node_id: ns.successor.as_ref().unwrap().clone()
                                    }
                                );
                            } else {
                                // The joining node's hash does not fall between the current node and its successor, considering wrap-around
                                log_message!(
                                    app_state_clone,
                                    "Joining node's hash does not fall between the current node and its successor, considering wrap-around"
                                );
                                send_post_request!(
                                    &format!("http://{}/msg", ns.successor.as_ref().unwrap()),
                                    Message::ReqJoin {
                                        node_id: node_id.clone()
                                    }
                                );
                            }
                        }
                    }
                    Message::ResJoin { node_id } => {
                        // Handle join response
                        log_message!(
                            app_state_clone,
                            "Join response, connect to node {}",
                            node_id
                        );
                        // update the successor of the current node to be the node_id

                        let mut ns = node_state_clone.lock().await;
                        ns.successor = Some(node_id.clone());
                        log_message!(app_state_clone, "Update successor to node {}", node_id);

                        // tell the node to update its predecessor to be the current node
                        let client = reqwest::Client::new(); // So we can make HTTP requests
                        send_post_request!(
                            &format!("http://{}/msg", node_id),
                            Message::IAmYourPredecessor {
                                node_id: ns.id.clone()
                            }
                        );
                    }
                    Message::IAmYourPredecessor { node_id } => {
                        // Handle predecessor update
                        log_message!(app_state_clone, "Update predecessor to node {}", node_id);

                        let mut ns = node_state_clone.lock().await;

                        // Check if the predecessor is already set to the desired value
                        if ns.predecessor.as_ref() != Some(&node_id) {
                            let client = reqwest::Client::new(); // So we can make HTTP requests

                            // transfer data to the predecessor that just joined
                            // send_post_request!(
                            //     &format!("http://{}/msg", node_id),
                            //     Message::Data {
                            //         from: ns.id.clone(),
                            //         data: app_state_clone
                            //             .select_data(hash(&node_id).into())
                            //             .await
                            //             .unwrap()
                            //     }
                            // );
                            send_post_request!(
                                &format!("http://{}/msg", ns.predecessor.as_ref().unwrap()),
                                Message::IAmYourSuccessor {
                                    node_id: node_id.clone()
                                }
                            );
                            send_post_request!(
                                &format!("http://{}/msg", node_id),
                                Message::IAmYourSuccessor {
                                    node_id: ns.id.clone()
                                }
                            );

                            // update the predecessor of the current node to be the node_id
                            ns.predecessor = Some(node_id.clone());
                        }
                    }
                    Message::IAmYourSuccessor { node_id } => {
                        // Handle successor update
                        log_message!(app_state_clone, "Update successor to node {}", node_id);

                        let mut ns = node_state_clone.lock().await;

                        // Check if the successor is already set to the desired value
                        if ns.successor.as_ref() != Some(&node_id) {
                            // update the successor of the current node to be the node_id
                            ns.successor = Some(node_id.clone());
                        }
                    }
                    Message::Data { from, data } => {
                        // Handle data transfer
                        log_message!(app_state_clone, "Transfer data to node {}", from);
                        // Insert the data into the current node
                        app_state_clone.insert_data(data).await;
                    }
                    Message::Kys => {
                        // Handle kill message
                        log_message!(app_state_clone, "Kys message received");
                        // drop the receiver to exit the loop
                        drop(rx);
                        break;
                    }
                    _ => (),
                }
            }
        });

        app_state
    }

    pub async fn select_data(&self, hash: u64) -> Result<Vec<Data>, rusqlite::Error> {
        let conn = self.db.lock().await;
        let mut stmt = conn.prepare("SELECT * FROM data WHERE hash <= ?")?;
        let data_iter = stmt.query_map(params![hash], |row| {
            Ok(Data {
                key: row.get(1)?,
                value: row.get(2)?,
            })
        })?;

        let mut data = Vec::new();
        for d in data_iter {
            data.push(d?);
        }

        Ok(data)
    }

    pub async fn remove_data(&self, hash: u64) -> Result<(), rusqlite::Error> {
        let conn = self.db.lock().await;
        let mut stmt = conn.prepare("DELETE FROM data WHERE hash <= ?")?;
        stmt.execute(params![hash])?;

        Ok(())
    }

    pub async fn insert_data(&self, data: Vec<Data>) -> Result<(), rusqlite::Error> {
        let conn = self.db.lock().await;
        let mut stmt = conn.prepare("INSERT INTO data (key, value, hash) VALUES (?, ?, ?)")?;
        for d in data {
            stmt.execute(params![d.key, d.value, hash(&d.key) as u64])?;
        }

        Ok(())
    }
}

fn in_mem_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();

    #[cfg(debug_assertions)]
    println!("Database connection established.");

    let schema = std::fs::read_to_string("./misc/schema.sql").expect("Failed to read schema.sql");
    conn.execute(&schema, []).expect("Failed to execute schema");

    #[cfg(debug_assertions)]
    {
        println!("Table 'data' created.");
        let mut stmt = conn.prepare("PRAGMA table_info(data)").unwrap();
        let table_info = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })
            .unwrap();

        println!("Schema of 'data' table:");
        for info in table_info {
            let (column_name, column_type) = info.unwrap();
            println!("{}: {}", column_name, column_type);
        }
    }

    conn
}

pub async fn get_data(data: web::Data<Node>) -> impl Responder {
    let conn = data.db.lock().await;
    let mut stmt = conn.prepare("SELECT * FROM data").unwrap();
    let data_iter = stmt
        .query_map([], |row| {
            // println!("Row: {:?}", row);
            Ok(Data::new(
                row.get::<_, String>(1).unwrap().as_str(),
                row.get::<_, String>(2).unwrap().as_str(),
            ))
        })
        .unwrap();

    let mut data_vec = Vec::new();
    for data in data_iter {
        data_vec.push(data.unwrap());
    }

    HttpResponse::Ok().json(data_vec)
}

pub async fn handle_message(data: web::Data<Node>, message: web::Json<Message>) -> impl Responder {
    #[cfg(debug_assertions)]
    println!("(Node)Handling message: {:?}", message);
    log_message!(data, "Handling message: {:?}", message);

    let tx = data.tx.clone();
    if let Err(err) = tx.send(message.into_inner()).await {
        log_message!(data, "Error sending message: {}", err.to_string());

        return HttpResponse::InternalServerError().json(serde_json::json!(
            Message::ErrorMessage {
                error: err.to_string(),
            }
        ));
    }
    log_message!(data, "Message sent successfully");

    HttpResponse::Ok().json(serde_json::json!(Message::Success {
        message: "Message sent successfully".to_string(),
    }))
}

pub async fn handle_index(data: web::Data<Node>) -> impl Responder {
    let node_state = data.node_state.lock().await;
    let logs = data.logs.lock().await;
    let conn = data.db.lock().await;
    let mut stmt = conn.prepare("SELECT * FROM data").unwrap();
    let data_iter = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .unwrap();

    let mut data_vec = Vec::new();
    for data in data_iter {
        data_vec.push(data.unwrap());
    }
    // Read the HTML template
    let mut html = fs::read_to_string(HTML_PATH).expect("Failed to read HTML template");

    // Replace placeholders with actual data
    html = html.replace(
        "{{node_id}}",
        format!("{} ({})", node_state.id, hash(&node_state.id)).as_str(),
    );
    html = html.replace(
        "{{predecessor}}",
        format!(
            "{} ({})",
            node_state
                .predecessor
                .as_ref()
                .unwrap_or(&"None".to_string()),
            hash(
                node_state
                    .predecessor
                    .as_ref()
                    .unwrap_or(&"None".to_string())
            )
        )
        .as_str(),
    );
    html = html.replace(
        "{{successor}}",
        format!(
            "{} ({})",
            node_state.successor.as_ref().unwrap_or(&"None".to_string()),
            hash(node_state.successor.as_ref().unwrap_or(&"None".to_string()))
        )
        .as_str(),
    );

    let log_html = logs
        .iter()
        .map(|log| format!("<li>{}</li>", log))
        .collect::<Vec<String>>()
        .join("");
    html = html.replace(
        "{logs}",
        if log_html.is_empty() {
            "<li>No logs</li>"
        } else {
            &log_html
        },
    );

    let finger_table_html = node_state
        .finger_table
        .entries
        .iter()
        .map(|entry| {
            format!(
                "<li>{}): <span class=\"font-semibold\">{}</span></li>",
                entry.start,
                entry.id.as_ref().unwrap_or(&"None".to_string())
            )
        })
        .collect::<Vec<String>>()
        .join("");
    html = html.replace(
        "{finger_table}",
        if finger_table_html.is_empty() {
            "<li>No entries</li>"
        } else {
            &finger_table_html
        },
    );

    let data_html = data_vec
        .iter()
        .map(|data| format!("<li>H({}) {}: {}</li>", data.0, data.1, data.2))
        .collect::<Vec<String>>()
        .join("");
    html = html.replace(
        "{node_data}",
        if data_html.is_empty() {
            "<li>No data</li>"
        } else {
            &data_html
        },
    );

    // Return the updated HTML content
    HttpResponse::Ok().content_type("text/html").body(html)
}

pub async fn run_server(app_state: web::Data<Node>) -> std::io::Result<()> {
    let id = app_state.node_state.lock().await.id.clone();
    #[cfg(debug_assertions)]
    println!("(Node)Server running at http://{}", id.clone());

    app_state.node_state.lock().await.predecessor = Some(id.clone());
    app_state.node_state.lock().await.successor = Some(id.clone());

    let bind_address = {
        let node_state = app_state.node_state.lock().await;
        node_state.id.clone()
    };
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/", web::get().to(handle_index))
            .route("/data", web::get().to(get_data))
            .route(
                "/msg",
                web::post().to(move |data: web::Data<Node>, message: web::Json<Message>| {
                    handle_message(data, message)
                }),
            )
    })
    .bind(bind_address.clone())?
    .run()
    .await
}
