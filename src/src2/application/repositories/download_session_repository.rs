use async_trait::async_trait;

use crate::src2::application::models::{DownloadSession, DownloadSessionFile};
use crate::src2::errors::AppError;



#[async_trait]
pub trait DownloadSessionRepository: Send + Sync {
    async fn create_session(&self, session: &DownloadSession) -> Result<(), AppError>;
    
    async fn add_files(&self, files: &[DownloadSessionFile]) -> Result<(), AppError>;

    async fn get_file(&self, session_id: &str, file_index: u32) -> Result<DownloadSessionFile, AppError>;

    /// Marks all files in the session as "downloaded".
    ///
    /// This is used to implement strict OneTime semantics for ZIP access
    /// (consume the whole session up-front).
    async fn consume_session(&self, session_id: &str) -> Result<(), AppError>;

    /// Atomically claims a file for download and returns its metadata.
    ///
    /// Implementations should:
    /// - Reject expired sessions
    /// - Reject already-downloaded files
    ///
    /// This is the hot-path for `JwtAuthMethod::OneTime`.
    async fn claim_file(&self, session_id: &str, file_index: u32) -> Result<DownloadSessionFile, AppError>;

    /// Claims a one-time /studyToken JWT so it cannot be reused.
    ///
    /// Default implementation returns a clear error so new backends don't accidentally
    /// ship without enforcing OneTime semantics.
    async fn claim_one_time_token(&self, _token: &str, _exp: usize) -> Result<(), AppError> {
        Err(AppError::OneTimeTokenStoreUnsupported)
    }

    /// Best-effort cleanup of expired OneTime data in the application DB.
    ///
    /// Implementations should delete only data that is safely past `cutoff`.
    ///
    /// Default implementation is a no-op so backends that don't persist OneTime
    /// data won't accidentally break at runtime.
    async fn cleanup_expired(&self, _cutoff: chrono::DateTime<chrono::Utc>) -> Result<(), AppError> {
        Ok(())
    }
}
