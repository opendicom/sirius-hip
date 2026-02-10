
// -- Sub-modules rexports ------------------------------------------------------------- //

pub mod study_token;
pub mod settings;
pub mod qido;
pub mod wado;
// ------------------------------------------------------------------------------------ //


pub mod echo { 
    use actix_web::Responder;

    pub async fn endpoint() -> impl Responder {
        "echo"
    }
}

pub mod setting { 
    use actix_web::{Responder, HttpResponse, web::Data};
    use crate::settings::Settings;

    pub async fn endpoint(settings: Data<Settings>) -> impl Responder {
       HttpResponse::Ok().json(settings)
    }
}