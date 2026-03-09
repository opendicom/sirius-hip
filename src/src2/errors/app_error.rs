use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {

    // ---------- Request ----------

    #[error("bad request")]
    BadRequest { reason: String },

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
    Unauthorized { reason: String },

    #[error("invalid or expired jwt token")]
    InvalidJwt(#[from] jsonwebtoken::errors::Error),

    #[error("one-time token already used")]
    TokenAlreadyUsed,

    #[error("one-time token store not supported by this backend")]
    OneTimeTokenStoreUnsupported,

    #[error("session token binding not supported by this backend")]
    SessionTokenBindingUnsupported,

    // ---------- Infrastructure ----------

    #[error("database error")]
    Database(#[from] sqlx::Error),

    // #[error("mongodb error")]
    // Mongo(#[from] mongodb::error::Error),

    #[error("io error")]
    Io(#[from] std::io::Error),

    // ---------- Internal ----------

    #[error("missing filesystem reference for stable row (study={study_uid}, series={series_uid}, sop={sop_uid})")]
    MissingFilesystemReference {
        study_uid: String,
        series_uid: String,
        sop_uid: String,
    },

    #[error("internal application error")]
    Internal(#[from] anyhow::Error),

    // ---------- Pacs --------------
    #[error("PACS error")]
    Pacs(#[from] PacsError),
}


use actix_web::{
    App, HttpResponse, ResponseError, http::StatusCode
};
use serde_json::json;
use uuid::Uuid;

use crate::src2::errors::PacsError;

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::BadRequest { .. } => StatusCode::BAD_REQUEST,

            AppError::Unauthorized { .. }
            | AppError::InvalidJwt(_) => StatusCode::UNAUTHORIZED,

            AppError::TokenAlreadyUsed => StatusCode::CONFLICT,

            AppError::OneTimeTokenStoreUnsupported => StatusCode::NOT_IMPLEMENTED,

            AppError::SessionTokenBindingUnsupported => StatusCode::NOT_IMPLEMENTED,

            AppError::DownloadSessionNotFound
            | AppError::FileIndexNotFound(_) => StatusCode::NOT_FOUND,

            AppError::DownloadSessionExpired 
            | AppError::FileAlreadyDownloaded(_)=> StatusCode::CONFLICT,

            AppError::Pacs(_) => StatusCode::INTERNAL_SERVER_ERROR,

            AppError::Database(_)
            // | AppError::Mongo(_)
            | AppError::Io(_)
            | AppError::MissingFilesystemReference { .. }
            | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        let error_id = Uuid::new_v4().to_string();

        // Log everything, but do not leak internals to clients.
        match status.as_u16() {
            500..=599 => log::error!("{status} [{error_id}]\n{:#?}", self),
            400..=499 => log::warn!("{status} [{error_id}]\n{:#?}", self),
            _ => log::info!("{status} [{error_id}] {:?}", self),
        }

        let (code, message) = match self {
            AppError::BadRequest { reason } => (
                "BAD_REQUEST",
                if reason.trim().is_empty() { "bad request".to_string() } else { reason.clone() },
            ),

            AppError::Unauthorized { reason } => (
                "UNAUTHORIZED",
                if reason.trim().is_empty() { "unauthorized".to_string() } else { reason.clone() },
            ),

            AppError::InvalidJwt(_) => (
                "INVALID_JWT", "invalid or expired jwt token".to_string()
            ),
            
            AppError::TokenAlreadyUsed => (
                "TOKEN_USED", "one-time token already used".to_string()
            ),
            
            AppError::OneTimeTokenStoreUnsupported => (
                "ONE_TIME_TOKEN_STORE_UNSUPPORTED", "one-time token store not supported by this backend".to_string(),
            ),

            AppError::SessionTokenBindingUnsupported => (
                "SESSION_TOKEN_BINDING_UNSUPPORTED",
                "session token binding not supported by this backend".to_string(),
            ),

            AppError::DownloadSessionNotFound => (
                "SESSION_NOT_FOUND", "download session not found".to_string()
            ),
            
            AppError::DownloadSessionExpired => (
                "SESSION_EXPIRED", "download session expired".to_string()
            ),
            
            AppError::FileIndexNotFound(_) => (
                "FILE_NOT_FOUND", "file not found in download session".to_string()
            ),
            
            AppError::FileAlreadyDownloaded(_) => (
                "ALREADY_DOWNLOADED", "file already downloaded".to_string()
            ),

            // Internal / infrastructure
            AppError::Database(_)
            | AppError::Io(_)
            | AppError::Internal(_)
            | AppError::Pacs(_) 
            | AppError::MissingFilesystemReference { .. } => (
                "INTERNAL", "internal error".to_string()
            ),
        };

        HttpResponse::build(status)
            .insert_header(("X-Error-Id", error_id.clone()))
            .json(json!({
            "error": {
                "id": error_id,
                "code": code,
                "message": message,
            }
        }))
    }
}

impl AppError {
    pub fn bad_request(reason: impl Into<String>) -> Self {
        Self::BadRequest { reason: reason.into() }
    }

    pub fn unauthorized(reason: impl Into<String>) -> Self {
        Self::Unauthorized { reason: reason.into() }
    }
}
