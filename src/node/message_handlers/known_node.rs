use super::*;

pub async fn known_node_handler(
    ns: &mut NodeState,
    node_id: String,
    app_state_clone: web::Data<Node>,
    chrod_ring: String,
) -> Result<(), Box<dyn std::error::Error>> {
    if node_id != ns.id {
        match send_post_request!(
            &format!("http://{}/msg", node_id),
            Message::ReqJoin {
                node_id: ns.id.clone()
            }
        ) {
            Ok(_) => Ok(()),
            Err(_) => {
                log_message!(
                    app_state_clone,
                    "Failed to send join request to node {}",
                    node_id
                );
                send_post_request!(
                    &format!("http://{}/msg", chrod_ring),
                    Message::CheckNode {
                        node_id: node_id.clone()
                    }
                )?;
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                send_post_request!(
                    &format!("http://{}/msg", chrod_ring),
                    Message::ReqKnownNode {
                        node_id: ns.id.clone()
                    }
                )?;
                Ok(())
            }
        }
    } else {
        log_message!(
            app_state_clone,
            "Node is currently the only node in the ring"
        );
        Ok(())
    }
}
