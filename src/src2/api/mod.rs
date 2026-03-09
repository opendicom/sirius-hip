mod study_token_handler;
mod files_handler;
mod qido_handler;
mod wado_handler;

pub use study_token_handler::*;
pub use files_handler::*;
pub use qido_handler::*;
pub use wado_handler::*;

use actix_web::HttpRequest;
use actix_web::http::header;

use crate::src2::errors::app_error::AppError;

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

