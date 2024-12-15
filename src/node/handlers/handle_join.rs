use super::*;
pub async fn handle_join(data: web::Data<Node>) -> impl Responder {
    // send a message to the ChordRing to request known nodes
    let node_state = data.node_state.lock().await;
    let node_id = node_state.id.clone();
    send_post_request!(
        &format!("http://{}/msg", format!("{}:{}", *IP, *PORT)),
        Message::ReqKnownNode { node_id }
    );

    HttpResponse::Ok().body("Join request sent")
}
