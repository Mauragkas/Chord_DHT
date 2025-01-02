use super::*;
pub async fn handle_insert(
    data: web::Data<Node>,
    data_to_ins: web::Json<Vec<Data>>,
) -> impl Responder {
    let node_state = data.node_state.lock().await;
    let node_hash = hash(&node_state.id);
    let prev_hash = node_state
        .predecessor
        .as_ref()
        .map(|id| hash(id))
        .unwrap_or(node_hash);

    let mut local_data = Vec::new();
    let mut forward_data = Vec::new();

    for item in data_to_ins.iter() {
        let data_hash = hash(&item.key);
        log_message!(data, "Handling insert request for key: {}", item.key);
        log_message!(
            data,
            "Hash values - Data: [{}], Node: [{}], Predecessor: [{}]",
            data_hash,
            node_hash,
            prev_hash
        );

        if is_between(prev_hash, data_hash, node_hash) {
            local_data.push(item.clone());
        } else {
            forward_data.push(item.clone());
        }
    }

    if !local_data.is_empty() {
        log_message!(data, "Data belongs to this node, inserting locally");
        match data.insert_batch_data(local_data).await {
            Ok(_) => {
                log_message!(data, "Data inserted successfully");
                HttpResponse::Ok().body("Data inserted successfully")
            }
            Err(err) => {
                log_message!(data, "Error inserting data: {}", err);
                HttpResponse::InternalServerError().body(err.to_string())
            }
        }
    } else if !forward_data.is_empty() {
        // Forward to successor if data doesn't belong here
        if let Some(successor) = &node_state.successor {
            log_message!(data, "Forwarding data to successor node: {}", successor);
            send_post_request!(&format!("http://{}/insert", successor), forward_data);
            log_message!(data, "Data forwarded successfully to successor");
            HttpResponse::Ok().body("Data forwarded to successor")
        } else {
            log_message!(data, "No successor found, cannot forward data");
            HttpResponse::InternalServerError().body("No successor found")
        }
    } else {
        HttpResponse::Ok().body("No data to process")
    }
}
