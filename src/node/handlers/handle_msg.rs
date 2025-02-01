use super::*;
pub async fn handle_message(data: web::Data<Node>, message: web::Json<Message>) -> impl Responder {
    match message.0 {
        Message::Ping => {
            return HttpResponse::Ok().json(serde_json::json!(Message::Pong));
        }
        _ => {
            let tx = data.tx.clone();
            if let Err(err) = tx.send(message.into_inner()).await {
                log_message!(data, "ERROR sending message: {}", err.to_string());

                return HttpResponse::InternalServerError().json(serde_json::json!(
                    Message::ErrorMessage {
                        error: err.to_string(),
                    }
                ));
            }

            HttpResponse::Ok().json(serde_json::json!(Message::Success {
                message: "Message sent successfully".to_string(),
            }))
        }
    }
}
