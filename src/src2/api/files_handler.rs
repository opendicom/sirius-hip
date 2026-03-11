use actix_web::{HttpRequest, HttpResponse};
use actix_web::web;
use futures::StreamExt;
use serde::Deserialize;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::src2::errors::app_error::AppError;
use crate::src2::state2::AppState2;
use crate::src2::api::utils::path::safe_join_filesystem_path;
use crate::src2::api::utils::wado::wado_url_from_uids;
use crate::src2::api::utils::wado_proxy::proxy_wado_url;
use crate::settings::JwtAuthMethod;
use crate::auth;

use super::extract_token_from_headers;

#[derive(Debug, Deserialize)]
pub struct FilesQueryParams {
    pub token: Option<String>,
}

// =============================================================================================== //
// HTTP HANDLER - /files/{session_id}/{file_index}                                                 //
// =============================================================================================== //

/// Handles HTTP requests for the /files/{session_id}/{file_index} endpoint, which serves DICOM files 
/// for download based on the session and file index.
/// It supports both direct filesystem access and WADO proxying as a fallback.
/// The session and file index are used to look up the file metadata in the download session repository, 
/// which indicates whether the file can be served directly from the filesystem or if it needs to be 
/// proxied from WADO.
pub async fn download_file_handler(
    path: web::Path<(String, u32)>,
    state: web::Data<AppState2>,
    req: HttpRequest,
    query: web::Query<FilesQueryParams>,
) -> Result<HttpResponse, AppError> {
    let (session_id, file_index) = path.into_inner();

    // --------------------------------------------------------------
    // 0. VALIDATE JWT TOKEN (STANDARD/ONETIME) + BIND TOKEN TO SESSION
    // --------------------------------------------------------------
    if matches!(state.settings.jwt_auth, JwtAuthMethod::Standard | JwtAuthMethod::OneTime) {
        let mut token = query.token.clone();
        if let Some(header_token) = extract_token_from_headers(&req)? {
            token = Some(header_token);
        }

        let token = token
            .as_ref()
            .ok_or_else(|| AppError::unauthorized("missing token"))?;

        auth::validate_jwt_token(token, state.settings.as_ref())
            .map_err(|_| AppError::unauthorized("unauthorized"))?;

        // Prevent session_id leakage from being sufficient to download.
        state
            .download_session_repo
            .assert_session_token_bound(&session_id, token)
            .await?;
    }

    // --------------------------------------------------------------
    // 1. CLAIM FILE (ONETIME) OR JUST FETCH METADATA (STANDARD)
    // --------------------------------------------------------------
    let f = if state.settings.jwt_auth == JwtAuthMethod::OneTime {
        state
            .download_session_repo
            .claim_file(&session_id, file_index)
            .await?
    } else {
        state
            .download_session_repo
            .get_file(&session_id, file_index)
            .await?
    };

    // --------------------------------------------------------------
    // 2. SERVE FILE FROM FILESYSTEM OR PROXY WADO AND STREAM BYTES BACK
    // --------------------------------------------------------------
    // Prefer filesystem when indicated, but fall back to WADO if the file is missing.

    // Try filesystem first.
    if let (Some(fs_id), Some(rel)) = (f.filesystem_fk, f.relative_file_path.as_deref()) {
        if let Some(base) = state.settings.dicomarchive.get_fs_path_by_id(fs_id) {
            if let Some(abs_path) = safe_join_filesystem_path(base, rel) {
                if let Ok(file) = File::open(&abs_path).await {
                let stream = ReaderStream::new(file).map(|chunk| {
                    chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                });
                return Ok(HttpResponse::Ok()
                    .content_type("application/dicom")
                    .append_header((
                        "Content-Disposition",
                        format!("attachment; filename=\"{}.dcm\"", f.instance_uid),
                    ))
                    .streaming(stream));
                }
            }
        }
    }

    // --------------------------------------------------------------
    // 3. FALL BACK TO WADO PROXY
    // --------------------------------------------------------------
    let url = wado_url_from_uids(&state.settings, &f.study_uid, &f.series_uid, &f.instance_uid);
    proxy_wado_url(&state.http_client, url).await
}
