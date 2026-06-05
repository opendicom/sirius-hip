use actix_web::web;

use super::handler::search_studies;

/// Routes for the study token feature.
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/studyToken")
            .route(web::get().to(search_studies)),
    );
}