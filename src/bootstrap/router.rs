use actix_web::web;

/// Build the application router, configuring routes for all features.
pub fn build_router(cfg: &mut web::ServiceConfig) {

    // Configure routes for the study token feature.
    crate::features::study_token::routes(cfg);
    
}