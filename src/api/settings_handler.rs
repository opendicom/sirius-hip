use actix_web::{Responder, HttpResponse, web::Data};
use crate::settings::Settings;

pub async fn settings_handler(settings: Data<Settings>) -> impl Responder {
    HttpResponse::Ok().json(settings)
}
