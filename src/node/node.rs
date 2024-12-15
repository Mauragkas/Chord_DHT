use super::*;
use finger_table::finger_table::FingerTable;

lazy_static::lazy_static! {
    static ref CHORD_RING: Mutex<String> = Mutex::new("".to_string());
}

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
            Some(p) => format!("{}:{}", *IP, p),
            None => format!("{}:3000", *IP),
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

                        // Check for hash collision
                        if hash_node_id == hash_joining_node
                            || hash_successor_id == hash_joining_node
                        {
                            log_message!(
                                app_state_clone,
                                "Node {} cannot join: hash collision detected",
                                node_id
                            );
                            send_post_request!(
                                &format!("http://{}/msg", node_id),
                                Message::NodeExists
                            );
                        } else {
                            // Determine the appropriate successor for the joining node
                            if is_between(hash_node_id, hash_joining_node, hash_successor_id) {
                                // The joining node's hash falls between the current node and its successor
                                let old_successor = ns.successor.clone();
                                ns.successor = Some(node_id.clone());

                                // Notify the joining node of its successor
                                send_post_request!(
                                    &format!("http://{}/msg", node_id),
                                    Message::ResJoin {
                                        node_id: old_successor.clone().unwrap(),
                                        sender_id: ns.id.clone()
                                    }
                                );

                                // Notify the old successor for its new predecessor
                                send_post_request!(
                                    &format!("http://{}/msg", old_successor.unwrap()),
                                    Message::Notify {
                                        node_id: node_id.clone()
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

                        send_post_request!(
                            &format!("http://{}/msg", CHORD_RING.lock().await.clone()),
                            Message::ResKnownNode {
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
                                // .select_data(Some(hash_sender))
                                .select_data(Some(hash_predecessor_id), Some(hash_sender))
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
                                // .remove_data(Some(hash_sender))
                                .remove_data(Some(hash_predecessor_id), Some(hash_sender))
                                .await
                                .unwrap();
                        } else {
                            log_message!(app_state_clone, "Did not update predecessor");
                        }
                    }
                    Message::IAmYourPredecessor { node_id } => {
                        // Handle predecessor update
                        log_message!(app_state_clone, "Update predecessor to node {}", node_id);

                        // update the predecessor of the current node to be the node_id
                        let mut ns = node_state_clone.lock().await;
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
                    Message::NodeExists => {
                        log_message!(app_state_clone, "Node already exists in the ring");
                    }
                    _ => (),
                }
            }
        });

        app_state
    }

    pub async fn req_known_node(&self, node: String) -> Result<(), Box<dyn std::error::Error>> {
        log_message!(self, "Requesting known node from node: {}", node);
        *CHORD_RING.lock().await = node.clone();
        let res = send_post_request!(
            &format!("http://{}/msg", node),
            Message::ReqKnownNode {
                node_id: self.node_state.lock().await.id.clone()
            }
        );

        if res.status().is_success() {
            Ok(())
        } else {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to send request",
            )))
        }
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
                    data: self.select_data(None, None).await?
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
            let chord_ring = CHORD_RING.lock().await.to_string();
            send_post_request!(
                &format!("http://{}/msg", chord_ring),
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

    // major bug fix
    pub async fn select_data(
        &self,
        start_hash: Option<u32>,
        end_hash: Option<u32>,
    ) -> Result<Vec<Data>, rusqlite::Error> {
        let conn = self.db.lock().await;
        let mut stmt = match (start_hash, end_hash) {
            (Some(start), Some(end)) => {
                if start <= end {
                    // Normal case - no wraparound
                    conn.prepare("SELECT * FROM data WHERE hash > ? AND hash <= ? ORDER BY hash")?
                } else {
                    // Wraparound case - get data outside the excluded range
                    conn.prepare("SELECT * FROM data WHERE hash > ? OR hash <= ? ORDER BY hash")?
                }
            }
            _ => conn.prepare("SELECT * FROM data ORDER BY hash")?,
        };

        let data_iter = {
            let map_fn = |row: &rusqlite::Row| {
                Ok(Data {
                    key: row.get(1)?,
                    value: row.get(2)?,
                })
            };

            match (start_hash, end_hash) {
                (Some(start), Some(end)) => stmt.query_map(params![start, end], map_fn)?,
                _ => stmt.query_map([], map_fn)?,
            }
        };

        let mut data = Vec::new();
        for d in data_iter {
            data.push(d?);
        }

        Ok(data)
    }

    // major bug fix
    pub async fn remove_data(
        &self,
        start_hash: Option<u32>,
        end_hash: Option<u32>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.db.lock().await;
        match (start_hash, end_hash) {
            (Some(start), Some(end)) => {
                if start <= end {
                    conn.execute(
                        "DELETE FROM data WHERE hash > ? AND hash <= ?",
                        params![start, end],
                    )?
                } else {
                    conn.execute(
                        "DELETE FROM data WHERE hash > ? OR hash <= ?",
                        params![start, end],
                    )?
                }
            }
            _ => conn.execute("DELETE FROM data", [])?,
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
