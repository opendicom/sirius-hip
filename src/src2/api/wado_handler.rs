use actix_web::{web, HttpRequest, HttpResponse};
use futures::StreamExt;
use serde::Deserialize;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::auth;
use crate::settings::JwtAuthMethod;
use crate::src2::errors::app_error::AppError;
use crate::src2::state2::AppState2;
use crate::src2::api::utils::path::safe_join_filesystem_path;
use crate::src2::api::utils::wado::{
    build_upstream_url,
    normalize_content_type,
    normalize_transfer_syntax,
    strip_internal_query_params,
};
use crate::src2::api::utils::wado_proxy::proxy_wado_response;

use super::extract_token_from_headers;

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
/// Represents the query parameters for a WADO-URI request.
/// This struct is used to deserialize the query parameters from the HTTP request and provides a structured way to access them in the handler logic.
pub struct WadoQueryParams {
    pub studyUID: Option<String>,
    pub seriesUID: Option<String>,
    pub objectUID: String,

    pub contentType: Option<String>,
    pub transferSyntax: Option<String>,

    pub token: Option<String>,
    pub session: Option<String>,
}

// =============================================================================================== //
// HTTP HANDLER - /wado                                                                            //
// =============================================================================================== //

/// Handles HTTP requests for the /wado endpoint, which serves DICOM files for download based on WADO-URI query parameters.
/// It first validates the JWT token and ensures that the token is bound to the session. Then it attempts to resolve the file 
/// from the session and claim it if using OneTime auth.
pub async fn wado_handler(
    state: web::Data<AppState2>,
    req: HttpRequest,
    query: web::Query<WadoQueryParams>,
) -> Result<HttpResponse, AppError> {
    let mut query = query.into_inner();

    // Token: headers first, then query param.
    if let Some(token) = extract_token_from_headers(&req)? {
        query.token = Some(token);
    }

    // --------------------------------------------------------------
    // 1. Validate auth according to settings
    // --------------------------------------------------------------
    match state.settings.jwt_auth {
        JwtAuthMethod::Standard | JwtAuthMethod::OneTime => {
            let token = query
                .token
                .as_ref()
                .ok_or_else(|| AppError::unauthorized("missing token"))?;
            auth::validate_jwt_token(token, state.settings.as_ref())
                .map_err(|_| AppError::unauthorized("unauthorized"))?;
        }
    }

    // Session is required for secure mapping (filesystem vs WADO) and OneTime claiming.
    let session_id = query
        .session
        .as_deref()
        .ok_or_else(|| AppError::unauthorized("missing session parameter"))?;

    // Bind token to session so session_id alone is not sufficient.
    let token = query
        .token
        .as_deref()
        .ok_or_else(|| AppError::unauthorized("missing token"))?;
    state
        .download_session_repo
        .assert_session_token_bound(session_id, token)
        .await?;

    let content_type = normalize_content_type(query.contentType.as_deref());
    let transfer_syntax = normalize_transfer_syntax(query.transferSyntax.as_deref(), &state.settings.dicomarchive.transfer_syntax);

    // --------------------------------------------------------------
    // 2. Resolve file from session (and claim per instance in OneTime)
    // --------------------------------------------------------------
    let f = if state.settings.jwt_auth == JwtAuthMethod::OneTime {
        state
            .download_session_repo
            .claim_wado_by_instance_uid_and_content_type(session_id, &query.objectUID, content_type.as_ref())
            .await?
    } else {
        state
            .download_session_repo
            .get_file_by_instance_uid(session_id, &query.objectUID)
            .await?
    };

    // Defensive: if client provides study/series UIDs, enforce consistency.
    if let Some(study_uid) = query.studyUID.as_deref() {
        if study_uid != f.study_uid {
            return Err(AppError::unauthorized("unauthorized"));
        }
    }
    if let Some(series_uid) = query.seriesUID.as_deref() {
        if series_uid != f.series_uid {
            return Err(AppError::unauthorized("unauthorized"));
        }
    }

    // --------------------------------------------------------------
    // 3. Try filesystem first (fast-path).
    // --------------------------------------------------------------
    // - `Content-Type` must be application/dicom or fall back to WADO for other content types (e.g. image/jpeg).
    // - `TransferSyntax` must match application config or fall back to WADO for other transfer syntaxes.
    if content_type.eq_ignore_ascii_case("application/dicom") 
        && transfer_syntax.eq_ignore_ascii_case(&state.settings.dicomarchive.transfer_syntax) 
    {
        if let (Some(fs_id), Some(rel)) = (f.filesystem_fk, f.relative_file_path.as_deref()) {
            if let Some(base) = state.settings.dicomarchive.get_fs_path_by_id(fs_id) {
                if let Some(abs_path) = safe_join_filesystem_path(base, rel) {
                    if let Ok(file) = File::open(&abs_path).await {
                        let stream = ReaderStream::new(file).map(|chunk| {
                            chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                        });
                        return Ok(HttpResponse::Ok()
                            .content_type("application/dicom")
                            .streaming(stream));
                    }
                }
            }
        }
    }

    // --------------------------------------------------------------
    // 4. Fallback to WADO backend proxy (stream, preserve headers)
    // --------------------------------------------------------------
    let filtered_qs = strip_internal_query_params(req.query_string());
    let upstream_url = build_upstream_url(&state.settings.dicomarchive.wadouri, &filtered_qs);

    let res = state
        .http_client
        .get(upstream_url)
        .send()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    proxy_wado_response(res)
}
