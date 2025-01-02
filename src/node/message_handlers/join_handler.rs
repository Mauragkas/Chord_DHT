use super::*;

pub async fn req_join_handler(
    ns: &mut NodeState,
    node_id: String,
    app_state_clone: web::Data<Node>,
) -> Result<(), Box<dyn std::error::Error>> {
    log_message!(app_state_clone, "Join request from node {}", node_id);

    let hash_node_id = hash(&ns.id);
    let hash_successor_id = hash(&ns.successor.as_ref().unwrap());
    let hash_joining_node = hash(&node_id);

    // Check for hash collision
    if hash_node_id == hash_joining_node || hash_successor_id == hash_joining_node {
        log_message!(
            app_state_clone,
            "Node {} cannot join: hash collision detected",
            node_id
        );
        send_post_request!(&format!("http://{}/msg", node_id), Message::NodeExists)?;
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
            )?;

            // Notify the old successor for its new predecessor
            send_post_request!(
                &format!("http://{}/msg", old_successor.clone().unwrap()),
                Message::Notify {
                    node_id: node_id.clone()
                }
            )?;
        } else {
            // Forward the join request to the successor
            send_post_request!(
                &format!("http://{}/msg", ns.successor.as_ref().unwrap()),
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
    ns.successor = Some(node_id.clone());
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
