use actix_web::{HttpResponse, web};
use serde_querystring_actix::QueryString;

use crate::src2::errors::app_error::AppError;
use crate::src2::application::use_cases::execute_qido_studies;
use crate::src2::state2::AppState2;
use crate::api::qido::QidoStudiesParams;

// =============================================================================================== //
// HTTP HANDLER - /qido/studies                                                                    //
// =============================================================================================== //

/// Handles HTTP requests for the QIDO studies endpoint (/qido/studies), which returns study metadata 
/// in DICOM JSON format based on query parameters and `includefield options.
pub async fn qido_studies_handler(
    params: QueryString<QidoStudiesParams>,
    state: web::Data<AppState2>,
) -> Result<HttpResponse, AppError> {
    let params = params.into_inner();

    let qido = execute_qido_studies(
        params, 
        state.pacs.study_repo.clone(),
        state.download_session_repo.clone(),
        state.settings.clone()
    ).await?;
    
    Ok(HttpResponse::Ok().json(qido))
}
