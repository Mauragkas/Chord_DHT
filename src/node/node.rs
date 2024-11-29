use super::*;
use finger_table::finger_table::FingerTable;

#[derive(Debug)]
pub struct Node {
    pub db: Mutex<Connection>,
    pub node_state: Arc<Mutex<NodeState>>,
    pub tx: mpsc::Sender<Message>,
    pub logs: Arc<Mutex<Vec<String>>>,
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
        });

        // Spawn a task to handle messages from the channel
        let node_state_clone = node_state.clone();
        let app_state_clone = app_state.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
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
                    Message::Leave { node_id } => {
                        log_message!(app_state_clone, "Node {} left the ring", node_id);
                        // Optionally propagate the leave message
                        let ns = node_state_clone.lock().await;
                        if let Some(ref succ) = ns.successor {
                            if *succ != node_id {
                                send_post_request!(
                                    &format!("http://{}/msg", succ),
                                    Message::Leave { node_id }
                                );
                            }
                        }
                    }
                    Message::ReqJoin { node_id } => {
                        // Handle join request
                        log_message!(app_state_clone, "Join request from node {}", node_id);

                        let mut ns = node_state_clone.lock().await;
                        let hash_node_id = hash(&ns.id);
                        let hash_successor_id = hash(&ns.successor.as_ref().unwrap());
                        let hash_joining_node = hash(&node_id);

                        // Determine the appropriate successor for the joining node
                        if is_between(hash_node_id, hash_joining_node, hash_successor_id) {
                            // The joining node's hash falls between the current node and its successor
                            let old_successor = ns.successor.clone();
                            ns.successor = Some(node_id.clone());

                            // Notify the joining node of its successor
                            send_post_request!(
                                &format!("http://{}/msg", node_id),
                                Message::ResJoin {
                                    node_id: old_successor.unwrap(),
                                    sender_id: ns.id.clone()
                                }
                            );
                        } else {
                            // Forward the join request to the successor
                            send_post_request!(
                                &format!("http://{}/msg", ns.successor.as_ref().unwrap()),
                                Message::ReqJoin {
                                    node_id: node_id.clone()
                                }
                            );
                        }
                    }
                    Message::ResJoin { node_id, sender_id } => {
                        // This becomes more of a fallback or initial contact mechanism
                        let mut ns = node_state_clone.lock().await;
                        ns.successor = Some(node_id.clone());
                        ns.predecessor = Some(sender_id.clone());

                        log_message!(
                            app_state_clone,
                            "Updated successor to {} and predecessor to {}",
                            node_id,
                            sender_id
                        );
                        // Notify the new successor about this node
                        send_post_request!(
                            &format!("http://{}/msg", node_id),
                            Message::Notify {
                                node_id: ns.id.clone()
                            }
                        );
                    }
                    Message::Notify { node_id } => {
                        let mut ns = node_state_clone.lock().await;
                        let hash_node_id = hash(&ns.id);
                        let hash_predecessor_id = ns
                            .predecessor
                            .as_ref()
                            .map(|id| hash(id))
                            .unwrap_or(hash_node_id);
                        let hash_sender = hash(&node_id);

                        if is_between(hash_predecessor_id, hash_sender, hash_node_id) {
                            ns.predecessor = Some(node_id.clone());
                            log_message!(app_state_clone, "Updated predecessor to {}", node_id);

                            // Transfer only the data that should belong to the new predecessor
                            let data_to_transfer = app_state_clone
                                .select_data(Some(hash_sender))
                                .await
                                .unwrap();

                            // Send the filtered data to the new predecessor
                            log_message!(app_state_clone, "Transfer data to node {}", node_id);
                            send_post_request!(
                                &format!("http://{}/msg", node_id),
                                Message::Data {
                                    from: ns.id.clone(),
                                    data: data_to_transfer
                                }
                            );

                            // Remove transferred data from current node
                            log_message!(app_state_clone, "Remove data from node");
                            app_state_clone
                                .remove_data(Some(hash_sender))
                                .await
                                .unwrap();
                        } else {
                            log_message!(app_state_clone, "Did not update predecessor");
                        }
                    }
                    Message::IAmYourPredecessor { node_id } => {
                        // Handle predecessor update
                        log_message!(app_state_clone, "Update predecessor to node {}", node_id);

                        let mut ns = node_state_clone.lock().await;

                        // update the predecessor of the current node to be the node_id
                        ns.predecessor = Some(node_id.clone());
                    }
                    Message::IAmYourSuccessor { node_id } => {
                        // Handle successor update
                        log_message!(app_state_clone, "Update successor to node {}", node_id);

                        let mut ns = node_state_clone.lock().await;
                        // update the successor of the current node to be the node_id
                        ns.successor = Some(node_id.clone());
                    }
                    Message::Data { from, data } => {
                        // Handle data transfer
                        log_message!(app_state_clone, "Transfer data to node {}", from);
                        // Insert the data into the current node
                        app_state_clone.insert_data(data).await;
                    }
                    _ => (),
                }
            }
        });

        app_state
    }

    pub async fn leave(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut node_state = self.node_state.lock().await;
        let node_id = node_state.id.clone();
        let successor = node_state.successor.clone().unwrap();
        let predecessor = node_state.predecessor.clone().unwrap();

        // Only proceed if the node is not the only node in the ring
        if successor != node_id.clone() && predecessor != node_id.clone() {
            // 1. Transfer data to the successor
            send_post_request!(
                &format!("http://{}/msg", successor),
                Message::Data {
                    from: node_id.clone(),
                    data: self.select_data(None).await?
                }
            );

            // 2. Notify the successor of the node's departure
            send_post_request!(
                &format!("http://{}/msg", successor),
                Message::IAmYourPredecessor {
                    node_id: predecessor.clone()
                }
            );

            // 3. Notify the predecessor of the node's departure
            send_post_request!(
                &format!("http://{}/msg", predecessor),
                Message::IAmYourSuccessor {
                    node_id: successor.clone()
                }
            );

            // 4. Send leave message to the ChordRing
            send_post_request!(
                &format!("http://{}/msg", format!("{}:{}", "0.0.0.0", *PORT)),
                Message::Leave {
                    node_id: node_id.clone()
                }
            );

            // 5. Clear the data in the node
            self.db.lock().await.execute("DELETE FROM data", [])?;

            // 6. Update node_state's successor and predecessor
            node_state.successor = Some(node_id.clone());
            node_state.predecessor = Some(node_id.clone());

            log_message!(self, "Node left the ring");
        } else {
            log_message!(self, "Node is the only node in the ring");
        }

        Ok(())
    }

    async fn select_data(&self, hash: Option<u32>) -> Result<Vec<Data>, rusqlite::Error> {
        let conn = self.db.lock().await;
        let mut stmt = match hash {
            Some(h) => conn.prepare("SELECT * FROM data WHERE hash <= ? ORDER BY hash")?,
            None => conn.prepare("SELECT * FROM data ORDER BY hash")?,
        };

        let data_iter = {
            let map_fn = |row: &rusqlite::Row| {
                Ok(Data {
                    key: row.get(1)?,
                    value: row.get(2)?,
                })
            };

            match hash {
                Some(h) => stmt.query_map(params![h], map_fn)?,
                None => stmt.query_map([], map_fn)?,
            }
        };

        let mut data = Vec::new();
        for d in data_iter {
            data.push(d?);
        }

        Ok(data)
    }

    async fn remove_data(&self, hash: Option<u32>) -> Result<(), rusqlite::Error> {
        let conn = self.db.lock().await;
        match hash {
            Some(h) => conn.execute("DELETE FROM data WHERE hash <= ?", params![h])?,
            None => conn.execute("DELETE FROM data", [])?,
        };

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
