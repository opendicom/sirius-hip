use actix_web::HttpRequest;
use actix_web::{web, HttpResponse};
use crate::api::study_token::params::StudyTokenParams;
use crate::src2::application::use_cases::{execute_study_token, StudyTokenOutput};
use crate::src2::errors::app_error::AppError;
use crate::src2::state2::AppState2;
use super::extract_token_from_headers;

// =============================================================================================== //
// HTTP HANDLER - /studyToken                                                                      //
// =============================================================================================== //

/// Handles HTTP requests for the study token endpoint, which generates a token 
/// that can be used to access study data in various formats 
/// - Cornerstone manifest
/// - Weasis manifest
/// - OHIF manifest
/// - DICOM ZIP
pub async fn study_token_handler(
    state: web::Data<AppState2>,
    req: HttpRequest,
    params: web::Query<StudyTokenParams>,
) -> Result<HttpResponse, AppError> {

    // --------------------------------------------------------------
    // 1. GET SERVER BASE URL
    // --------------------------------------------------------------
    let conn = req.connection_info();
    let server_base_url = format!("{}://{}",
        conn.scheme(),
        conn.host(),
    );


    // --------------------------------------------------------------
    // 2. EXTRACT TOKEN (headers first, then query param)
    // --------------------------------------------------------------
    let mut params = params.into_inner();
    if let Some(token) = extract_token_from_headers(&req)? {
        params.token = Some(token);
    }
    
    
    // --------------------------------------------------------------
    // 3. PROCESS REQUEST
    // --------------------------------------------------------------
    let output = execute_study_token(
        state.pacs.study_repo.clone(),
        state.download_session_repo.clone(),
        state.tmp_pool.clone(),
        state.settings.clone(),
        params,
        &server_base_url,
    )
    .await?;


    // --------------------------------------------------------------
    // 4. BUILD RESPONSE
    // --------------------------------------------------------------
    Ok(match output {
        StudyTokenOutput::Json(value) => HttpResponse::Ok().json(value),
        StudyTokenOutput::Xml(xml) => HttpResponse::Ok()
            .content_type("application/xml; charset=utf-8")
            .body(xml),
        StudyTokenOutput::Zip { filename, zip } => HttpResponse::Ok()
            .content_type("application/zip")
            .append_header((
                "Content-Disposition",
                format!("attachment; filename=\"{}\"", filename),
            ))
            .streaming(zip.build()),
    })
}