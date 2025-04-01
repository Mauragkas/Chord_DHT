use super::*;

pub async fn handle_successors(data: web::Data<Node>) -> impl Responder {
    let node = data.get_ref();
    let successors: Vec<String> = node
        .node_state
        .lock()
        .await
        .successor
        .entries
        .iter()
        .filter_map(|x| x.clone())
        .collect();
    HttpResponse::Ok().json(successors)
}

pub async fn handle_predecessor(data: web::Data<Node>) -> impl Responder {
    let node = data.get_ref();
    let predecessor = &node.node_state.lock().await.predecessor;
    HttpResponse::Ok().json(predecessor)
}
