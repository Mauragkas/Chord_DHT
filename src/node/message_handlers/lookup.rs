use super::*;

pub async fn lookup_req_handler(
    ns: &NodeState,
    app_state: web::Data<Node>,
    key: String,
    hops: usize,
    chord_ring: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let hash_key = hash(&key);
    let hash_node_id = hash(&ns.id);
    let hash_successor_id = hash(&ns.successor.get_first().unwrap());
    let hash_predecessor_id = hash(&ns.predecessor.as_ref().unwrap());

    if is_between(hash_predecessor_id, hash_key, hash_node_id) {
        // case 1: key belongs to the current node
        let data = app_state.select_specific_data(key.clone()).await.unwrap();
        send_post_request!(
            &format!("http://{}/msg", chord_ring),
            Message::LookupRes {
                key,
                hops,
                data: Some(data)
            }
        )?;
    } else if is_between(hash_node_id, hash_key, hash_successor_id) {
        // case 2: key belongs to the successor
        match send_post_request!(
            &format!("http://{}/msg", ns.successor.get_first().unwrap()),
            Message::LookupReq {
                key: key.clone(),
                hops: hops + 1
            }
        ) {
            Ok(_) => (),
            Err(e) => {
                send_post_request!(
                    &format!("http://{}/msg", chord_ring),
                    Message::LookupRes {
                        key: key.clone(),
                        hops,
                        data: None
                    }
                )?;
                log_message!(app_state, "Failed to send lookup request: {}", e)
            }
        }
    } else {
        // Iterate over finger table entries 2 by 2
        let finger_entries = &ns.finger_table.entries;
        let mut found = false;

        for i in 0..finger_entries.len() - 1 {
            let entry1 = &finger_entries[i];
            let entry2 = &finger_entries[i + 1];

            if is_between(entry1.start, hash_key, entry2.start) {
                if let Some(ref node_id) = entry1.id {
                    match send_post_request!(
                        &format!("http://{}/msg", node_id),
                        Message::LookupReq {
                            key: key.clone(),
                            hops: hops + 1
                        }
                    ) {
                        Ok(_) => (),
                        Err(e) => {
                            send_post_request!(
                                &format!("http://{}/msg", chord_ring),
                                Message::LookupRes {
                                    key: key.clone(),
                                    hops,
                                    data: None
                                }
                            )?;
                            log_message!(app_state, "Failed to send lookup request: {}", e)
                        }
                    }
                    found = true;
                    break;
                }
            }
        }

        if !found {
            // Loop through finger table in reverse until successful
            for entry in finger_entries.iter().rev() {
                if let Some(ref node_id) = entry.id {
                    match send_post_request!(
                        &format!("http://{}/msg", node_id),
                        Message::LookupReq {
                            key: key.clone(),
                            hops: hops + 1
                        }
                    ) {
                        Ok(_) => {
                            found = true;
                            break;
                        }
                        Err(_) => continue,
                    }
                }
            }

            // If still not found after trying all fingers, send lookup failure
            if !found {
                send_post_request!(
                    &format!("http://{}/msg", chord_ring),
                    Message::LookupRes {
                        key: key.clone(),
                        hops,
                        data: None
                    }
                )?;
                log_message!(
                    app_state,
                    "Failed to find node for key after trying all fingers"
                );
            }
        }
    }
    Ok(())
}
