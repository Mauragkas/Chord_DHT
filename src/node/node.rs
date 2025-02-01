use super::*;
use finger_table::finger_table::FingerTable;
use message_handlers::known_node;
use tokio::time::interval;

lazy_static::lazy_static! {
    pub static ref CHORD_RING: Mutex<String> = Mutex::new("".to_string());
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
        let tx_clone = tx.clone();

        // update fingers periodically
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let mut interval = interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                let ns = node_state_clone.lock().await.clone();
                if let Some(succ) = ns.successor.get_first() {
                    if *succ != ns.id {
                        let _ = tx_clone
                            .send(Message::ReqFinger {
                                from: ns.id.clone(),
                                index: ns.finger_table.get_first_entry() as usize,
                            })
                            .await;
                    }
                }
            }
        });

        // stabilize the ring periodically
        let node_state_clone = node_state.clone();
        let app_state_clone = app_state.clone();
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let mut interval = interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                let mut ns = node_state_clone.lock().await;

                // Skip if we're alone in the ring
                if ns.successor.get_first() == Some(&ns.id) {
                    continue;
                }

                // Check current successor list and remove dead nodes
                let mut i = 0;
                while i < *N {
                    if let Some(succ) = ns.successor.entries[i].clone() {
                        // Try to ping the successor
                        let is_alive = match send_post_request!(
                            &format!("http://{}/msg", succ),
                            Message::Ping
                        ) {
                            Ok(response) => response.status().is_success(),
                            Err(_) => false,
                        };

                        if !is_alive {
                            log_message!(
                                app_state_clone,
                                "Successor {} is dead, removing from successor list",
                                succ
                            );
                            // Remove dead successor
                            ns.successor.remove_successor(&succ);
                            // Don't increment i as we need to check the shifted successor
                            continue;
                        }
                    }
                    i += 1;
                }

                // If we lost our immediate successor, promote the next alive successor
                if ns.successor.get_first().is_none() {
                    for i in 1..*N {
                        if let Some(succ) = ns.successor.entries[i].clone() {
                            log_message!(
                                app_state_clone,
                                "Promoting {} to immediate successor",
                                succ
                            );
                            ns.successor.clear();
                            ns.successor.insert_first(succ);
                            break;
                        }
                    }
                }

                // Update successor list with successors' successors
                if let Some(immediate_succ) = ns.successor.get_first().cloned() {
                    let res = send_get_request!(&format!("http://{}/successors", immediate_succ));
                    if let Ok(response) = res {
                        if let Ok(succ_list) = response.json::<Vec<String>>().await {
                            let mut new_successors = vec![Some(immediate_succ)];

                            // Add successors' successors to our list
                            for succ in succ_list.into_iter().take(*N - 1) {
                                // Verify each successor is alive before adding
                                if send_post_request!(
                                    &format!("http://{}/msg", succ),
                                    Message::Ping
                                )
                                .is_ok()
                                {
                                    new_successors.push(Some(succ));
                                }
                            }

                            // Pad with None if we don't have enough successors
                            while new_successors.len() < *N {
                                new_successors.push(None);
                            }

                            // Update successor list
                            for (i, succ) in new_successors.into_iter().enumerate() {
                                ns.successor.insert(i, succ);
                            }
                        }
                    }
                }

                // If we still have a valid immediate successor, notify it
                if let Some(succ) = ns.successor.get_first() {
                    let _ = send_post_request!(
                        &format!("http://{}/msg", succ),
                        Message::Notify {
                            node_id: ns.id.clone()
                        }
                    );
                }
            }
        });

        let node_state_clone = node_state.clone();
        let app_state_clone = app_state.clone();
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                match message {
                    Message::ResKnownNode { node_id } => {
                        let mut ns = node_state_clone.lock().await;
                        known_node_handler(
                            &mut ns,
                            node_id,
                            app_state_clone.clone(),
                            CHORD_RING.lock().await.clone(),
                        )
                        .await
                        .unwrap();
                    }
                    Message::Leave { node_id } => {
                        let mut ns = node_state_clone.lock().await;
                        leave_handler(&mut ns, node_id, app_state_clone.clone())
                            .await
                            .unwrap();
                    }
                    Message::ReqJoin { node_id } => {
                        let mut ns = node_state_clone.lock().await;
                        req_join_handler(&mut ns, node_id, app_state_clone.clone())
                            .await
                            .unwrap();
                    }
                    Message::ResJoin { node_id, sender_id } => {
                        let mut ns = node_state_clone.lock().await;
                        res_join_handler(
                            &mut ns,
                            node_id,
                            sender_id,
                            app_state_clone.clone(),
                            CHORD_RING.lock().await.clone(),
                        )
                        .await
                        .unwrap();
                    }
                    Message::LookupReq { key, hops } => {
                        let ns = node_state_clone.lock().await;
                        lookup_req_handler(
                            &ns,
                            app_state_clone.clone(),
                            key,
                            hops,
                            CHORD_RING.lock().await.clone(),
                        )
                        .await
                        .unwrap();
                    }
                    Message::ReqFinger { from, index } => {
                        let ns = node_state_clone.lock().await;
                        finger_req_handler(&ns, from, index, app_state_clone.clone())
                            .await
                            .unwrap();
                    }
                    Message::ResFinger { node_id, index } => {
                        let mut ns = node_state_clone.lock().await;
                        finger_res_handler(&mut ns, node_id, index, app_state_clone.clone())
                            .await
                            .unwrap();
                    }
                    Message::Notify { node_id } => {
                        let mut ns = node_state_clone.lock().await;
                        notify_handler(&mut ns, node_id, app_state_clone.clone())
                            .await
                            .unwrap();
                    }
                    Message::IAmYourPredecessor { node_id } => {
                        log_message!(app_state_clone, "Update predecessor to node {}", node_id);
                        let mut ns = node_state_clone.lock().await;
                        ns.predecessor = Some(node_id.clone());
                    }
                    Message::IAmYourSuccessor { node_id } => {
                        log_message!(app_state_clone, "Update successor to node {}", node_id);
                        let mut ns = node_state_clone.lock().await;
                        ns.successor.clear();
                        ns.successor.insert_first(node_id.clone());
                    }
                    Message::Data { from, data } => {
                        log_message!(app_state_clone, "Transfer data to node {}", from);
                        app_state_clone.insert_batch_data(data).await;
                    }
                    Message::NodeExists => {
                        log_message!(app_state_clone, "Node already exists in the ring");
                        // println!("Node hash collision detected - exiting");
                        std::process::exit(1);
                    }
                    Message::Kys => {
                        log_message!(app_state_clone, "Received KYS message");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        std::process::exit(1);
                    }
                    _ => {
                        log_message!(app_state_clone, "Something unexpected was sent");
                    }
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
        )?;

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
        let successor = node_state.successor.get_first().unwrap().clone();
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
            // node_state.successor = Some(node_id.clone());
            node_state.successor.clear();
            node_state.successor.insert_first(node_id.clone());
            node_state.predecessor = Some(node_id.clone());
            node_state.finger_table.clear();

            log_message!(self, "Node left the ring");
        } else {
            log_message!(self, "Node is the only node in the ring");
        }

        Ok(())
    }

    pub async fn select_specific_data(&self, key: String) -> Result<Vec<Data>, rusqlite::Error> {
        let conn = self.db.lock().await;
        let mut stmt = conn.prepare("SELECT key, value FROM data WHERE key = ?")?;
        let data_iter = stmt.query_map(params![key], |row| {
            Ok(Data {
                key: row.get(0)?,
                value: row.get(1)?,
            })
        })?;

        let mut data = Vec::new();
        for d in data_iter {
            data.push(d?);
        }

        Ok(data)
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
    pub async fn insert_batch_data(&self, data: Vec<Data>) -> Result<(), rusqlite::Error> {
        let conn = self.db.lock().await;
        let mut stmt = conn.prepare("INSERT INTO data (key, value, hash) VALUES (?, ?, ?)")?;
        for d in data {
            stmt.execute(params![d.key, d.value, hash(&d.key) as u64])?;
        }

        Ok(())
    }
    async fn is_node_alive(&self, node_id: &str) -> bool {
        match send_post_request!(&format!("http://{}/msg", node_id), Message::Ping) {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}
