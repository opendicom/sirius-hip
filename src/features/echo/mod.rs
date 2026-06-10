use actix_web::web;

/// Configure the echo feature routes.
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/echo")
            .route(web::get().to(echo)),
    );
}

/// A simple handler that returns "OK" to confirm the application is running.
async fn echo() -> &'static str {
    "OK"
}