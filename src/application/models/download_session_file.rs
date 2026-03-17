use serde::{Deserialize, Serialize};

/// Represents a single downloadable file within a DownloadSession.
///
/// This entity maps:
/// - A session
/// - A deterministic file index
/// - The real PACS file (instance/file PK or UID)
///
/// The index is used to:
/// - Resolve the BitSet position
/// - Generate deterministic download URLs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadSessionFile {
    /// Session identifier
    pub session_id: String,

    /// Deterministic index within the session
    ///
    /// Used as the BitSet position.
    pub file_index: u32,

    /// SOP Instance UID (or PACS instance PK)
    pub instance_uid: String,

    /// Study Instance UID
    pub study_uid: String,

    /// Series Instance UID
    pub series_uid: String,

    /// Where the file must be obtained from
    ///
    /// - `true`  => download via WADO URL
    /// - `false` => download from filesystem
    pub use_wado: bool,

    /// Optional filesystem reference (when use_wado = false)
    ///
    /// This mirrors the stateless download token approach:
    /// - `filesystem_fk` selects a configured filesystem base path
    /// - `relative_file_path` is appended to that base
    pub filesystem_fk: Option<i32>,

    /// Relative file path within the filesystem base (when use_wado = false)
    pub relative_file_path: Option<String>,
}
