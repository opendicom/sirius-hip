use actix_web::{HttpRequest, HttpResponse, web};
use serde_querystring_actix::QueryString;

use crate::src2::errors::app_error::AppError;
use crate::src2::application::use_cases::execute_qido_studies;
use crate::src2::state2::AppState2;
use crate::api::qido::QidoStudiesParams;
use super::extract_token_from_headers;

// =============================================================================================== //
// HTTP HANDLER - /qido/studies                                                                    //
// =============================================================================================== //

/// Handles HTTP requests for the QIDO studies endpoint (/qido/studies), which returns study metadata 
/// in DICOM JSON format based on query parameters and `includefield options.
pub async fn qido_studies_handler(
    req: HttpRequest,
    params: QueryString<QidoStudiesParams>,
    state: web::Data<AppState2>,
) -> Result<HttpResponse, AppError> {
    let mut params = params.into_inner();
    if let Some(token) = extract_token_from_headers(&req)? {
        params.token = Some(token);
    }

    let qido = execute_qido_studies(
        params, 
        state.pacs.study_repo.clone(),
        state.download_session_repo.clone(),
        state.settings.clone()
    ).await?;
    
    Ok(HttpResponse::Ok().json(qido))
}
