use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {

    // ---------- Request ----------

    #[error("bad request")]
    BadRequest,

    // ---------- Download session ----------

    #[error("download session not found")]
    DownloadSessionNotFound,

    #[error("download session expired")]
    DownloadSessionExpired,

    #[error("file index {0} not found in download session")]
    FileIndexNotFound(u32),

    #[error("file index {0} already downloaded")]
    FileAlreadyDownloaded(u32),

    // ---------- Authentication ----------

    #[error("unauthorized")]
    Unauthorized,

    #[error("invalid or expired jwt token")]
    InvalidJwt(#[from] jsonwebtoken::errors::Error),

    #[error("one-time token already used")]
    TokenAlreadyUsed,

    #[error("one-time token store not supported by this backend")]
    OneTimeTokenStoreUnsupported,

    // ---------- Infrastructure ----------

    #[error("database error")]
    Database(#[from] sqlx::Error),

    // #[error("mongodb error")]
    // Mongo(#[from] mongodb::error::Error),

    #[error("io error")]
    Io(#[from] std::io::Error),

    // ---------- Internal ----------

    #[error("internal application error")]
    Internal(#[from] anyhow::Error),

    // ---------- Pacs --------------
    #[error("PACS error")]
    Pacs(#[from] PacsError),
}


use actix_web::{
    HttpResponse,
    ResponseError,
    http::StatusCode,
};
use serde_json::json;

use crate::src2::errors::PacsError;

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::BadRequest => StatusCode::BAD_REQUEST,

            AppError::Unauthorized 
            | AppError::InvalidJwt(_) => StatusCode::UNAUTHORIZED,

            AppError::TokenAlreadyUsed => StatusCode::CONFLICT,

            AppError::OneTimeTokenStoreUnsupported => StatusCode::NOT_IMPLEMENTED,

            AppError::DownloadSessionNotFound
            | AppError::FileIndexNotFound(_) => StatusCode::NOT_FOUND,

            AppError::DownloadSessionExpired 
            | AppError::FileAlreadyDownloaded(_)=> StatusCode::CONFLICT,

            AppError::Pacs(_) => StatusCode::INTERNAL_SERVER_ERROR,

            AppError::Database(_)
            // | AppError::Mongo(_)
            | AppError::Io(_)
            | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();

        // Log everything, but do not leak internals to clients.
        match status.as_u16() {
            500..=599 => log::error!("{status}\n {:#?}", self),
            400..=499 => log::warn!("{status}\n {:#?}", self),
            _ => log::info!("{status} {:?}", self),
        }

        let (code, message) = match self {
            AppError::BadRequest => ("BAD_REQUEST", "bad request".to_string()),

            AppError::Unauthorized => ("UNAUTHORIZED", "unauthorized".to_string()),
            AppError::InvalidJwt(_) => ("INVALID_JWT", "invalid or expired jwt token".to_string()),
            AppError::TokenAlreadyUsed => ("TOKEN_USED", "one-time token already used".to_string()),
            AppError::OneTimeTokenStoreUnsupported => (
                "ONE_TIME_TOKEN_STORE_UNSUPPORTED",
                "one-time token store not supported by this backend".to_string(),
            ),

            AppError::DownloadSessionNotFound => ("SESSION_NOT_FOUND", "download session not found".to_string()),
            AppError::DownloadSessionExpired => ("SESSION_EXPIRED", "download session expired".to_string()),
            AppError::FileIndexNotFound(_) => ("FILE_NOT_FOUND", "file not found in download session".to_string()),
            AppError::FileAlreadyDownloaded(_) => ("ALREADY_DOWNLOADED", "file already downloaded".to_string()),

            // Internal / infrastructure
            AppError::Database(_)
            | AppError::Io(_)
            | AppError::Internal(_)
            | AppError::Pacs(_) => ("INTERNAL", "internal error".to_string()),
        };

        HttpResponse::build(status).json(json!({
            "error": {
                "code": code,
                "message": message,
            }
        }))
    }
}
