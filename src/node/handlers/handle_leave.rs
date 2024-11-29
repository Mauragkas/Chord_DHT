use super::*;
pub async fn handle_leave(data: web::Data<Node>) -> impl Responder {
    match data.leave().await {
        Ok(_) => HttpResponse::Ok().body("Node left the ring"),
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}
