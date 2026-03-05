use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use chrono::{NaiveDate, NaiveDateTime};

use crate::database::DBVersion;
use crate::utils::{url_password_hidden, password_hidden};

mod naive_datetime_opt_serde {
    use super::*;
    use serde::de::Error as _;

    pub fn serialize<S>(value: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match value {
            None => serializer.serialize_none(),
            Some(dt) => serializer.serialize_str(&dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        let Some(s) = opt else {
            return Ok(None);
        };

        // IMPORTANT: interpreted as PACS *local* DATETIME cutoff.
        // We intentionally do NOT accept timezone-bearing timestamps (RFC3339 with Z/+HH:MM)
        // because PACS `study.created_time` is a MySQL DATETIME in local time.
        // Requiring local/no-TZ formats avoids accidental UTC/local mismatches.
        if s.contains('Z') || s.contains('+') {
            return Err(D::Error::custom(
                "dicomarchive.filesystem_cutoff_date must NOT include a timezone; use local datetime like 'YYYY-MM-DD' or 'YYYY-MM-DD HH:MM:SS'",
            ));
        }

        // MySQL DATETIME-like.
        if let Ok(dt) = NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S") {
            return Ok(Some(dt));
        }

        // ISO8601-ish without timezone.
        if let Ok(dt) = NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S") {
            return Ok(Some(dt));
        }

        // Date-only.
        if let Ok(d) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
            return Ok(Some(d.and_hms_opt(0, 0, 0).unwrap()));
        }

        Err(D::Error::custom(
            "dicomarchive.filesystem_cutoff_date must be local time: 'YYYY-MM-DD' or 'YYYY-MM-DD[ T]HH:MM:SS' (no timezone)",
        ))
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetadataOverride {
    pub keyword: String,
    pub source: String,
}

/// JWT Authentication methods supported by Sirius HIP
/// Used in the Settings struct
/// Defines how JWT authentication is handled
/// - None: No JWT authentication is performed
/// - Standard: Standard JWT authentication without sessions
/// - WzSession: JWT authentication tied to WZ sessions
/// Used to control access to resources based on JWT tokens
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum JwtAuthMethod {
    /// No JWT authentication
    None,

    /// Standard JWT authentication
    Standard,
    
    /// JWT authentication tied to WZ sessions
    OneTime,
}


#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct OneTimeCleanupSettings {
    /// Enables the background cleanup job.
    pub enabled: bool,

    /// How often the cleanup job runs.
    pub interval_secs: u64,

    /// Retention window *after* expiration before deleting DB rows.
    /// Example: 24 means delete sessions/tokens older than 24 hours past their `expires_at`.
    pub retention_hours: i64,

    /// Max number of sessions to delete per batch.
    pub session_batch: u32,

    /// Max number of session batches per run.
    pub max_batches: u32,

    /// Max number of token rows to delete per run.
    pub token_delete_limit: u32,

    /// Max initial delay added on startup to desynchronize instances.
    pub initial_jitter_max_secs: u64,
}

impl Default for OneTimeCleanupSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: 300,
            retention_hours: 24,
            session_batch: 200,
            max_batches: 20,
            token_delete_limit: 5000,
            initial_jitter_max_secs: 60,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    #[serde(serialize_with = "url_password_hidden")]
    pub loglevel: String,
    pub max_default: u64,

    pub app_database_url: Option<String>,
    pub app_database_max_connections: Option<u32>,

    pub studytoken_exclude_mods: Option<Vec<String>>,
    
    pub jwt_auth: JwtAuthMethod,
    #[serde(serialize_with = "password_hidden")]
    pub jwt_secret: String,
    pub jwt_algorithm: jsonwebtoken::Algorithm,

    /// Cleanup configuration for OneTime persistence (app DB).
    #[serde(default)]
    pub onetime_cleanup: OneTimeCleanupSettings,

    pub dicomarchive: DicomArchive,
    pub cors_whitelist: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DicomArchive {
    pub custodianoid: Option<String>,
    pub pacsoid: Option<String>,
    pub pacsaet: Option<String>,
    pub version: DBVersion,
    pub wadouri: String,
    pub manifest_base_url: Option<String>,
    pub transfer_syntax: String,

    /// Cutoff for enabling filesystem delivery.
    ///
    /// - Studies with `study.created_time` < cutoff are forced to WADO.
    /// - Studies with `study.created_time` >= cutoff can use filesystem when not marked dirty.
    #[serde(default, with = "naive_datetime_opt_serde")]
    pub filesystem_cutoff_date: Option<NaiveDateTime>,
    pub stow:  Option<String>,
    pub qido: Option<String>,
    pub number_frames_field: Option<String>,
    pub institution_field: Option<String>,
    pub metadata_overrides: Option<Vec<MetadataOverride>>,
    pub filesystems: Vec<FileSystem>,
    pub database_url: String,
    pub database_max_connections: u32,
}


impl Settings {
    pub fn validate(&self) -> anyhow::Result<()> {
        // Filesystem delivery requires an explicit cutoff date to keep legacy studies on WADO.
        if !self.dicomarchive.filesystems.is_empty() && self.dicomarchive.filesystem_cutoff_date.is_none() {
            anyhow::bail!(
                "dicomarchive.filesystem_cutoff_date must be set when dicomarchive.filesystems is configured"
            );
        }
        self.dicomarchive.validate_metadata_overrides()?;
        Ok(())
    }
}

impl DicomArchive {
    fn validate_metadata_overrides(&self) -> anyhow::Result<()> {
        let Some(list) = self.metadata_overrides.as_ref() else {
            return Ok(());
        };

        fn is_simple_identifier(s: &str) -> bool {
            let mut chars = s.chars();
            let Some(first) = chars.next() else {
                return false;
            };
            if !(first.is_ascii_alphabetic() || first == '_') {
                return false;
            }
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }

        fn is_simple_keyword(s: &str) -> bool {
            // DICOM keywords are typically ASCII alnum, starting with a letter.
            // Keep this strict so we can safely embed keywords into SQL aliases.
            is_simple_identifier(s)
        }

        fn parse_qualified_ident(source: &str) -> Option<(&str, &str)> {
            let (table, col) = source.split_once('.')?;
            if table.is_empty() || col.is_empty() {
                return None;
            }
            if !is_simple_identifier(table) || !is_simple_identifier(col) {
                return None;
            }
            Some((table, col))
        }

        let mut seen = HashSet::new();

        for ov in list {
            if !is_simple_keyword(&ov.keyword) {
                anyhow::bail!("Invalid metadata_overrides.keyword: {}", ov.keyword);
            }
            if !seen.insert(ov.keyword.as_str()) {
                anyhow::bail!("Duplicate metadata_overrides.keyword: {}", ov.keyword);
            }
            if parse_qualified_ident(&ov.source).is_none() {
                anyhow::bail!("Invalid metadata_overrides.source (expected table.column): {}", ov.source);
            }
        }

        Ok(())
    }
}


impl DicomArchive {
    /// Get path defined in settings file for that file system `id`
    pub fn get_fs_path_by_id(&self, id: i32) -> Option<&str> {
        for fs in &self.filesystems {
            if fs.id == id {
                return Some(fs.path.as_str())
            }
        }
        None
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileSystem {
    id: i32,
    path: String,
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn parse_settings_from_file(rel_path: &str) -> anyhow::Result<Settings> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push(rel_path);
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path.display(), e))?;
        let settings = toml::from_str::<Settings>(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path.display(), e))?;
        settings.validate()?;
        Ok(settings)
    }

    #[test]
    fn shipped_toml_configs_parse() -> anyhow::Result<()> {
        // Main sample config.
        let _ = parse_settings_from_file("sirius-hip.toml")?;
        // Dev configs.
        let _ = parse_settings_from_file("sirius-hip.dev.docker.toml")?;
        let _ = parse_settings_from_file("sirius-hip.dev.ridi.preprod.toml")?;
        let _ = parse_settings_from_file("sirius-hip.dev.ridi.testing.toml")?;
        Ok(())
    }

    #[test]
    fn docker_like_config_parses() -> anyhow::Result<()> {
        // Mirrors the keys/types produced by docker/build/sirius-hip.conf.template.
        let toml = r#"
loglevel = "info,hyper=info,reqwest=info,actix_web=info"
max_default = 2000
cors_whitelist = ["*"]

app_database_url = "mysql://pacs:pacs@opendicom_pacs_db:3306/pacsdb"
app_database_max_connections = 20

jwt_auth = "onetime"
jwt_secret = "secret"
jwt_algorithm = "HS256"

[onetime_cleanup]
enabled = true
interval_secs = 300
retention_hours = 24
session_batch = 200
max_batches = 20
token_delete_limit = 5000
initial_jitter_max_secs = 60

[dicomarchive]
version = "dcm4chee2183"
database_url = "mysql://pacs:pacs@opendicom_pacs_db:3306/pacsdb"
database_max_connections = 40
wadouri = "http://opendicom_pacs:8080/wado"
transfer_syntax = "1.2.840.10008.1.2.1"
filesystem_cutoff_date = "2026-03-01"
filesystems = [{ id = 1, path = "/DICOM/archive" }]
"#;

        let settings = toml::from_str::<Settings>(toml)?;
        settings.validate()?;
        Ok(())
    }
}

