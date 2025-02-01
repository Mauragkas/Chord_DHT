use super::*;

pub async fn finger_req_handler(
    ns: &NodeState,
    from: String,
    index: usize,
    app_state: web::Data<Node>,
) -> Result<(), Box<dyn std::error::Error>> {
    let target_id = ns.id.clone();
    let successor_id = match ns.successor.get_first() {
        Some(id) => id.clone(),
        None => return Ok(()),
    };

    if is_between(hash(&target_id), index as u32, hash(&successor_id)) {
        let _ = send_post_request!(
            &format!("http://{}/msg", from),
            Message::ResFinger {
                node_id: successor_id,
                index
            }
        );
    } else {
        if let Err(e) = send_post_request!(
            &format!("http://{}/msg", successor_id),
            Message::ReqFinger { from, index }
        ) {
            log_message!(
                &app_state,
                "Failed to send finger request to successor: {}",
                e
            );
        }
    }
    Ok(())
}

pub async fn finger_res_handler(
    ns: &mut NodeState,
    node_id: String,
    index: usize,
    app_state: web::Data<Node>,
) -> Result<(), Box<dyn std::error::Error>> {
    if ns
        .finger_table
        .update_entry(index.try_into().unwrap(), node_id.clone())
    {
        log_message!(
            app_state,
            "Updated finger table entry {} to {}",
            index,
            node_id
        );
    }

    if let Some(next_index) = ns.finger_table.get_next_entry(index.try_into().unwrap()) {
        if next_index != index as u32 {
            send_post_request!(
                &format!("http://{}/msg", ns.id.clone()),
                Message::ReqFinger {
                    from: ns.id.clone(),
                    index: next_index as usize
                }
            )?;
        }
    }
    Ok(())
}
