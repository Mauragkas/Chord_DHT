use super::*;

pub async fn notify_handler(
    ns: &mut NodeState,
    node_id: String,
    app_state: web::Data<Node>,
) -> Result<(), Box<dyn std::error::Error>> {
    let hash_node_id = hash(&ns.id);
    let hash_predecessor_id = ns.predecessor.as_ref().map_or(hash_node_id, |id| hash(id));
    let hash_sender = hash(&node_id);

    if !is_between(hash_predecessor_id, hash_sender, hash_node_id) {
        return Ok(());
    }

    ns.predecessor = Some(node_id.clone());
    log_message!(app_state, "Updated predecessor to {}", node_id);

    let data_to_transfer = app_state
        .select_data(Some(hash_predecessor_id), Some(hash_sender))
        .await?;

    if data_to_transfer.is_empty() {
        return Ok(());
    }

    log_message!(app_state, "Transfer data to node {}", node_id);

    send_post_request!(
        &format!("http://{}/msg", node_id),
        Message::Data {
            from: ns.id.clone(),
            data: data_to_transfer
        }
    )?;

    log_message!(app_state, "Remove data from node");
    app_state
        .remove_data(Some(hash_predecessor_id), Some(hash_sender))
        .await?;

    Ok(())
}
