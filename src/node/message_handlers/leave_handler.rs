use super::*;

pub async fn leave_handler(
    ns: &mut NodeState,
    node_id: String,
    app_state_clone: web::Data<Node>,
) -> Result<(), Box<dyn std::error::Error>> {
    log_message!(app_state_clone, "Node {} left the ring", node_id);
    if let Some(ref succ) = ns.successor {
        if *succ != node_id {
            send_post_request!(&format!("http://{}/msg", succ), Message::Leave { node_id })?;
        }
    }
    Ok(())
}
