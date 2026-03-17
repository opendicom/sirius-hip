use actix_web::Responder;

pub async fn echo_handler() -> impl Responder {
    "echo"
}