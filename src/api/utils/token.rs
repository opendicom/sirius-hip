use actix_web::http::header;
use actix_web::HttpRequest;

use crate::errors::AppError;

/// Extracts a Bearer token from the Authorization header of the HTTP request.
/// Returns `Ok(Some(token))` if a valid Bearer token is found,
/// `Ok (None)` if the header is not present, and `Err(AppError)` if the header
/// is malformed or contains an invalid token.
pub(crate) fn extract_token_from_headers(req: &HttpRequest) -> Result<Option<String>, AppError> {
    if let Some(value) = req.headers().get(header::AUTHORIZATION) {
        let auth = value
            .to_str()
            .map_err(|_| AppError::unauthorized("invalid token"))?
            .trim()
            .to_string();

        let mut parts = auth.split_whitespace();
        let scheme = parts.next().unwrap_or("");
        if !scheme.eq_ignore_ascii_case("bearer") {
            return Err(AppError::unauthorized("invalid token"));
        }

        let token = parts.next().unwrap_or("").trim();
        if token.is_empty() {
            return Err(AppError::unauthorized("missing token"));
        }
        if parts.next().is_some() {
            return Err(AppError::unauthorized("invalid token"));
        }
        return Ok(Some(token.to_string()));
    }

    Ok(None)
}

/// Token extraction with fixed precedence:
/// 1) Authorization: Bearer <token>
/// 2) ?token=<token>
pub(crate) fn extract_token(req: &HttpRequest, query_token: Option<&str>) -> Option<String> {
    if let Some(hv) = req
        .headers()
        .get(actix_web::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        // Case-insensitive "Bearer "
        if hv.len() >= 7 && hv[..7].eq_ignore_ascii_case("bearer ") {
            let t = hv[7..].trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }

    query_token
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
}
