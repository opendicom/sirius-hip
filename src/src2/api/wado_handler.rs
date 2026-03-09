use actix_web::{web, HttpRequest, HttpResponse};
use futures::StreamExt;
use serde::Deserialize;
use std::path::{Component, Path};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::auth;
use crate::settings::JwtAuthMethod;
use crate::src2::errors::app_error::AppError;
use crate::src2::state2::AppState2;
use crate::src2::utils::wado_proxy::proxy_wado_response;

use super::extract_token_from_headers;

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct WadoQueryParams {
    pub studyUID: Option<String>,
    pub seriesUID: Option<String>,
    pub objectUID: String,

    pub contentType: Option<String>,

    pub token: Option<String>,
    pub session: Option<String>,
}

fn safe_join_filesystem_path(base: &str, rel: &str) -> Option<String> {
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return None;
    }
    for comp in rel_path.components() {
        match comp {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Path::new(base).join(rel_path).to_str().map(|s| s.to_string())
}

fn build_upstream_url(wadouri_base: &str, query_string: &str) -> String {
    if query_string.is_empty() {
        return wadouri_base.to_string();
    }
    if wadouri_base.contains('?') {
        format!("{wadouri_base}&{query_string}")
    } else {
        format!("{wadouri_base}?{query_string}")
    }
}

fn normalize_content_type(v: Option<&str>) -> &str {
    // WADO-URI: contentType is optional; treat missing/empty as DICOM.
    // We only differentiate semantics by contentType (per user requirement),
    // not by imageQuality/rows/columns/etc.
    match v.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some(ct) => ct,
        None => "application/dicom",
    }
}

fn strip_internal_query_params(query_string: &str) -> String {
    // Never leak our internal security params to the upstream PACS.
    // Keep everything else intact (percent-encoding included).
    let mut out = Vec::new();
    for part in query_string.split('&') {
        if part.is_empty() {
            continue;
        }
        let key = part.split_once('=').map(|(k, _)| k).unwrap_or(part);
        if key == "token" || key == "session" {
            continue;
        }
        out.push(part);
    }
    out.join("&")
}

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
    // 1) Validate auth according to settings
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

    // --------------------------------------------------------------
    // 2) Resolve file from session (and claim per instance in OneTime)
    // --------------------------------------------------------------
    let f = if state.settings.jwt_auth == JwtAuthMethod::OneTime {
        state
            .download_session_repo
            .claim_wado_by_instance_uid_and_content_type(session_id, &query.objectUID, content_type)
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
    // 3) Try filesystem first (fast-path).
    // --------------------------------------------------------------
    // Rendered content-types (e.g. image/jpeg) must be handled by the upstream PACS.
    if content_type.eq_ignore_ascii_case("application/dicom") {
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
    // 4) Fallback to WADO backend proxy (stream, preserve headers)
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
