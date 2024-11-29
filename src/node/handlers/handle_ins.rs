use super::*;
pub async fn handle_insert(data: web::Data<Node>, data_to_ins: web::Json<Data>) -> impl Responder {
    let node_state = data.node_state.lock().await;
    let data_hash = hash(&data_to_ins.key);
    let node_hash = hash(&node_state.id);
    let prev_hash = node_state
        .predecessor
        .as_ref()
        .map(|id| hash(id))
        .unwrap_or(node_hash);

    log_message!(data, "Handling insert request for key: {}", data_to_ins.key);
    log_message!(
        data,
        "Hash values - Data: [{}], Node: [{}], Predecessor: [{}]",
        data_hash,
        node_hash,
        prev_hash
    );

    if is_between(prev_hash, data_hash, node_hash) {
        log_message!(data, "Data belongs to this node, inserting locally");
        match data.insert_data(vec![data_to_ins.into_inner()]).await {
            Ok(_) => {
                log_message!(data, "Data inserted successfully");
                HttpResponse::Ok().body("Data inserted successfully")
            }
            Err(err) => {
                log_message!(data, "Error inserting data: {}", err);
                HttpResponse::InternalServerError().body(err.to_string())
            }
        }
    } else {
        // Forward to successor if data doesn't belong here
        if let Some(successor) = &node_state.successor {
            log_message!(data, "Forwarding data to successor node: {}", successor);
            send_post_request!(
                &format!("http://{}/insert", successor),
                data_to_ins.into_inner()
            );
            log_message!(data, "Data forwarded successfully to successor");
            HttpResponse::Ok().body("Data forwarded to successor")
        } else {
            log_message!(data, "No successor found, cannot forward data");
            HttpResponse::InternalServerError().body("No successor found")
        }
    }
}
