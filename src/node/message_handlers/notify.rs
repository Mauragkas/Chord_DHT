use super::*;

pub async fn notify_handler(
    ns: &mut NodeState,
    node_id: String,
    app_state_clone: web::Data<Node>,
) -> Result<(), Box<dyn std::error::Error>> {
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

        let data_to_transfer = app_state_clone
            .select_data(Some(hash_predecessor_id), Some(hash_sender))
            .await?;

        log_message!(app_state_clone, "Transfer data to node {}", node_id);
        send_post_request!(
            &format!("http://{}/msg", node_id),
            Message::Data {
                from: ns.id.clone(),
                data: data_to_transfer
            }
        )?;

        log_message!(app_state_clone, "Remove data from node");
        app_state_clone
            .remove_data(Some(hash_predecessor_id), Some(hash_sender))
            .await?;
    } else {
        log_message!(app_state_clone, "Did not update predecessor");
    }

    Ok(())
}
