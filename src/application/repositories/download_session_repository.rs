use async_trait::async_trait;

use crate::application::models::{DownloadSession, DownloadSessionFile};
use crate::errors::AppError;



#[async_trait]
pub trait DownloadSessionRepository: Send + Sync {
    async fn create_session(&self, session: &DownloadSession) -> Result<(), AppError>;
    
    async fn add_files(&self, files: &[DownloadSessionFile]) -> Result<(), AppError>;

    /// Creates a session and inserts its files as a single optimized operation.
    ///
    /// Backends may override this to use a transaction and reduce commit overhead.
    /// Default implementation falls back to calling `create_session()` + `add_files()`.
    async fn create_session_with_files(
        &self,
        session: &DownloadSession,
        files: &[DownloadSessionFile],
    ) -> Result<(), AppError> {
        self.create_session(session).await?;
        self.add_files(files).await
    }

    async fn get_file(&self, session_id: &str, file_index: u32) -> Result<DownloadSessionFile, AppError>;

    /// Fetches file metadata by SOP Instance UID within a session.
    async fn get_file_by_instance_uid(
        &self,
        session_id: &str,
        instance_uid: &str,
    ) -> Result<DownloadSessionFile, AppError>;

    /// Ensures the provided JWT token is the one bound to the session.
    ///
    /// Implementations should also reject expired/missing sessions.
    ///
    /// Default implementation fails closed so backends don't accidentally ship without binding.
    async fn assert_session_token_bound(
        &self,
        _session_id: &str,
        _token: &str,
    ) -> Result<(), AppError> {
        Err(AppError::SessionTokenBindingUnsupported)
    }

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

    /// Atomically claims a file for download by SOP Instance UID.
    async fn claim_file_by_instance_uid(
        &self,
        session_id: &str,
        instance_uid: &str,
    ) -> Result<DownloadSessionFile, AppError>;

    /// Atomically claims a WADO download for a specific SOP Instance UID and `contentType`.
    ///
    /// This exists mainly to support Weasis which may request the same instance multiple
    /// times with different `contentType` values (e.g. `application/dicom` then `image/jpeg`).
    ///
    /// Default implementation falls back to strict per-instance claiming.
    async fn claim_wado_by_instance_uid_and_content_type(
        &self,
        session_id: &str,
        instance_uid: &str,
        _content_type: &str,
    ) -> Result<DownloadSessionFile, AppError> {
        self.claim_file_by_instance_uid(session_id, instance_uid).await
    }

    /// Claims a one-time /studyToken JWT so it cannot be reused.
    ///
    /// Default implementation returns a clear error so new backends don't accidentally
    /// ship without enforcing OneTime semantics.
    async fn claim_one_time_token(&self, _token: &str, _exp: usize) -> Result<(), AppError> {
        Err(AppError::OneTimeTokenStoreUnsupported)
    }

    /// Checks whether a one-time /studyToken JWT was already consumed.
    ///
    /// This should be a fast read (no commit) so callers can reject replays
    /// without paying write/fsync latency.
    async fn is_one_time_token_used(&self, _token: &str) -> Result<bool, AppError> {
        Err(AppError::OneTimeTokenStoreUnsupported)
    }

    /// Creates a session, inserts its files, and claims the one-time token in a single transaction.
    ///
    /// Implementations should insert the token first (so duplicates fail fast) and commit once.
    /// Default implementation falls back to claiming the token and then creating the session.
    async fn create_session_with_files_and_claim_token(
        &self,
        session: &DownloadSession,
        files: &[DownloadSessionFile],
        token: &str,
        exp: usize,
    ) -> Result<(), AppError> {
        self.claim_one_time_token(token, exp).await?;
        self.create_session_with_files(session, files).await
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
