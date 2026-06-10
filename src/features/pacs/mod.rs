use actix_web::{Result, web::{self, Data, Json}};

use crate::{bootstrap::state::AppState, shared::config::PacsSettings};


// -- PACS Routes -------------------------------------------------------------------------------- //

/// Routes for the PACS feature.
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/pacs")
            .route(web::get().to(pacs_handler)),
    );
}


// -- PACS Handlers -------------------------------------------------------------------------------- //

/// Handler for the /pacs endpoint, returning the list of PACS configurations.
pub async fn pacs_handler(state: Data<AppState>) -> Result<Json<Vec<PacsSettings>>, actix_web::Error> {

    Ok(Json(state.settings.pacs.clone()))
    
}


