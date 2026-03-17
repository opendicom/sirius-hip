use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a one-time download session.
///
/// A DownloadSession is created when the system runs in `JwtAuthMethod::OneTime`
/// mode and a valid study token is requested.
///
/// The session:
/// - Is immutable once created (except for download state)
/// - Controls access to a fixed set of files
/// - Uses per-file claiming (`HIP_download_session_claims`) to track downloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadSession {
    /// Public identifier (UUID or JWT `jti`)
    pub session_id: String,

    /// Expiration timestamp (usually equals JWT `exp`)
    pub expires_at: DateTime<Utc>,

    /// Total number of files in this session
    pub total_files: u32,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// SHA-256 hash of the JWT token that created this session.
    ///
    /// This binds `/wado` and `/files/{session}/{index}` requests to the same JWT.
    pub token_hash: Option<Vec<u8>>,
}

impl DownloadSession {
    /// Create a new download session.
    pub fn new(
        session_id: String,
        expires_at: DateTime<Utc>,
        total_files: u32,
        token_hash: Option<Vec<u8>>,
    ) -> Self {
        Self {
            session_id,
            expires_at,
            total_files,
            created_at: Utc::now(),
            token_hash,
        }
    }
}
