use super::*;

pub async fn req_join_handler(
    ns: &mut NodeState,
    node_id: String,
    app_state_clone: web::Data<Node>,
) -> Result<(), Box<dyn std::error::Error>> {
    log_message!(app_state_clone, "Join request from node {}", node_id);

    let hash_node_id = hash(&ns.id);
    let hash_successor_id = hash(&ns.successor.get_first().unwrap());
    let hash_joining_node = hash(&node_id);

    // Check for hash collision
    if hash_node_id == hash_joining_node || hash_successor_id == hash_joining_node {
        log_message!(
            app_state_clone,
            "Node {} cannot join: hash collision detected",
            node_id
        );
        match send_post_request!(&format!("http://{}/msg", node_id), Message::NodeExists) {
            Ok(_) => {}
            Err(e) => {
                log_message!(
                    app_state_clone,
                    "Failed to notify node {}: {}",
                    node_id,
                    e.to_string()
                );
            }
        }
    } else {
        // Determine the appropriate successor for the joining node
        if is_between(hash_node_id, hash_joining_node, hash_successor_id) {
            // The joining node's hash falls between the current node and its successor
            let old_successor = ns.successor.get_first().unwrap().clone();
            ns.successor.insert_first(node_id.clone());

            // Notify the joining node of its successor
            send_post_request!(
                &format!("http://{}/msg", node_id),
                Message::ResJoin {
                    node_id: old_successor.clone(),
                    sender_id: ns.id.clone()
                }
            )?;

            // Notify the old successor for its new predecessor
            send_post_request!(
                &format!("http://{}/msg", old_successor.clone()),
                Message::Notify {
                    node_id: node_id.clone()
                }
            )?;
        } else {
            // Forward the join request to the successor
            send_post_request!(
                &format!("http://{}/msg", ns.successor.get_first().unwrap()),
                Message::ReqJoin {
                    node_id: node_id.clone()
                }
            )?;
        }
    }
    Ok(())
}

pub async fn res_join_handler(
    ns: &mut NodeState,
    node_id: String,
    sender_id: String,
    app_state_clone: web::Data<Node>,
    chord_ring: String,
) -> Result<(), Box<dyn std::error::Error>> {
    // Clear current successor list and add new successor
    ns.successor.clear();
    ns.successor.insert_first(node_id.clone());

    // Try to get successor's successor list
    if let Ok(response) = send_get_request!(&format!("http://{}/successors", node_id)) {
        if let Ok(succ_list) = response.json::<Vec<String>>().await {
            for (i, succ) in succ_list.into_iter().enumerate() {
                if i + 1 < *N {
                    // Skip first as we already have it
                    ns.successor.insert(i + 1, Some(succ));
                }
            }
        }
    }

    ns.predecessor = Some(sender_id.clone());

    log_message!(
        app_state_clone,
        "Updated successor to {} and predecessor to {}",
        node_id,
        sender_id
    );

    send_post_request!(
        &format!("http://{}/msg", chord_ring),
        Message::ResKnownNode {
            node_id: ns.id.clone()
        }
    )?;

    send_post_request!(
        &format!("http://{}/msg", node_id),
        Message::ReqFinger {
            from: ns.id.clone(),
            index: ns.finger_table.get_first_entry() as usize
        }
    )?;

    Ok(())
}
