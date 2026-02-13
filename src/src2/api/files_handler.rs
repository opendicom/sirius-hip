use actix_web::HttpResponse;
use actix_web::web;
use futures::StreamExt;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use reqwest;

use crate::src2::errors::app_error::AppError;
use crate::src2::state2::AppState2;
use crate::settings::JwtAuthMethod;
use crate::auth;

/// Constructs a WADO URL from the given UIDs and settings.
fn wado_url_from_uids(
    settings: &crate::settings::Settings,
    study_uid: &str,
    series_uid: &str,
    sop_uid: &str,
) -> String {
    format!(
        "{}?requestType=WADO&studyUID={}&seriesUID={}&objectUID={}&contentType=application/dicom&transferSyntax={}",
        settings.dicomarchive.wadouri,
        study_uid,
        series_uid,
        sop_uid,
        settings.dicomarchive.transfer_syntax,
    )
}

/// Proxies a WADO response by streaming the bytes back to the client while preserving headers and status code.
fn proxy_wado_response(res: reqwest::Response) -> Result<HttpResponse, AppError> {
    let mut out = HttpResponse::build(res.status());
    for (header_name, header_value) in res
        .headers()
        .iter()
        .filter(|(h, _)| *h != "connection")
    {
        out.insert_header((header_name.clone(), header_value.clone()));
    }

    Ok(out.streaming(res.bytes_stream().map(|chunk| {
        chunk.map_err(|e| actix_web::error::ErrorInternalServerError(e))
    })))
}

/// Proxies a WADO request by forwarding the query parameters to the WADO service and streaming the response back.
async fn proxy_wado_url(client: &reqwest::Client, url: String) -> Result<HttpResponse, AppError> {
    let res = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    proxy_wado_response(res)
}


// =============================================================================================== //
// HTTP HANDLER - /file/{token}                                                                    //
// =============================================================================================== //


/// Handles HTTP requests for the download file endpoint, which serves DICOM files for download based on the token in the URL. 
/// It supports both direct filesystem access and WADO proxying as a fallback.
pub async fn download_token_handler(
    path: web::Path<String>,
    state: web::Data<AppState2>,
) -> Result<HttpResponse, AppError> {
    let token = path.into_inner();
    let claims = auth::validate_download_token(&token, &state.settings)
        .map_err(|_| AppError::unauthorized("unauthorized"))?;

    
    // --------------------------------------------------------------
    // 1. TRY FILESYSTEM FIRST WHEN TOKEN INCLUDES FILESYSTEM REFERENCE
    // --------------------------------------------------------------
    if let (Some(fs_id), Some(rel)) = (claims.filesystem_fk, claims.relative_file_path.as_deref()) {
        if let Some(base) = state.settings.dicomarchive.get_fs_path_by_id(fs_id) {
            let abs_path = format!(
                "{}/{}",
                base.trim_end_matches('/'),
                rel.trim_start_matches('/')
            );

            if let Ok(file) = File::open(&abs_path).await {
                let stream = ReaderStream::new(file).map(|chunk| {
                    chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                });
                return Ok(HttpResponse::Ok()
                    .content_type("application/dicom")
                    .append_header((
                        "Content-Disposition",
                        format!("attachment; filename=\"{}.dcm\"", claims.sop_uid),
                    ))
                    .streaming(stream));
            }
        }
    }

    // --------------------------------------------------------------
    // 2. FALL BACK TO WADO PROXY
    // --------------------------------------------------------------
    let url = wado_url_from_uids(&state.settings, &claims.study_uid, &claims.series_uid, &claims.sop_uid);
    proxy_wado_url(&state.http_client, url).await
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
) -> Result<HttpResponse, AppError> {
    let (session_id, file_index) = path.into_inner();

    // --------------------------------------------------------------
    // 1. CLAIM FILE (ONETIME) OR JUST FETCH METADATA (STANDARD/NONE)
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
            let abs_path = format!(
                "{}/{}",
                base.trim_end_matches('/'),
                rel.trim_start_matches('/')
            );

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

    // --------------------------------------------------------------
    // 3. FALL BACK TO WADO PROXY
    // --------------------------------------------------------------
    let url = wado_url_from_uids(&state.settings, &f.study_uid, &f.series_uid, &f.instance_uid);
    proxy_wado_url(&state.http_client, url).await
}
