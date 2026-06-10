use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub server: ServerSettings,
    pub jwt: JwtSettings,
    pub pacs: Vec<PacsSettings>,
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.pacs.is_empty() {
            return Err("at least one PACS backend must be configured".to_string());
        }

        let mut pacs_ids = std::collections::HashSet::new();

        for pacs in &self.pacs {
            if pacs.id.trim().is_empty() {
                return Err("PACS backend id cannot be empty".to_string());
            }

            if !pacs_ids.insert(pacs.id.as_str()) {
                return Err(format!("duplicated PACS id `{}`", pacs.id));
            }

            if pacs.aet.trim().is_empty() {
                return Err(format!("PACS backend `{}` has empty AET", pacs.id));
            }

            match &pacs.connection {
                PacsConnectionSettings::DatabaseFilesystem {
                    database,
                    filesystems,
                } => {
                    if database.url.trim().is_empty() {
                        return Err(format!(
                            "PACS backend `{}` has empty database url",
                            pacs.id
                        ));
                    }

                    if filesystems.is_empty() {
                        return Err(format!(
                            "PACS backend `{}` requires at least one filesystem mapping",
                            pacs.id
                        ));
                    }

                    let mut filesystem_ids = std::collections::HashSet::new();

                    for filesystem in filesystems {
                        if filesystem.path.as_os_str().is_empty() {
                            return Err(format!(
                                "PACS backend `{}` has a filesystem mapping with empty path",
                                pacs.id
                            ));
                        }

                        if !filesystem_ids.insert(filesystem.id) {
                            return Err(format!(
                                "PACS backend `{}` has duplicated filesystem id `{}`",
                                pacs.id,
                                filesystem.id
                            ));
                        }
                    }
                }

                PacsConnectionSettings::Dicomweb { dicomweb } => {
                    if dicomweb.url.trim().is_empty() {
                        return Err(format!(
                            "PACS backend `{}` has empty dicomweb url",
                            pacs.id
                        ));
                    }
                }
            }
        }

        Ok(())
    }

}


// -- General settings -------------------------------------------------------------------------------- //


#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JwtAuthMode {
    Standard,
    Onetime,
}

impl Default for JwtAuthMode {
    fn default() -> Self {
        Self::Standard
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerSettings {
    pub bind: String,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:5001".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct JwtSettings {
    pub mode: JwtAuthMode,
    pub secret: String,
}

impl Default for JwtSettings {
    fn default() -> Self {
        Self {
            mode: JwtAuthMode::Standard,
            secret: "change-me".to_string(),
        }
    }
}

// PACS-related settings -------------------------------------------------------------------------------- //

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PacsSettings {
    pub id: String,
    pub kind: PacsKind,
    pub aet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing)]
    pub connection: PacsConnectionSettings,
}


#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PacsKind {
    Dcm4chee2183,
    Dcm4chee440,
}


// -- Pacs connection settings -------------------------------------------------------------------------------- //

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum PacsConnectionSettings {

    #[serde(rename = "database_filesystem")]
    DatabaseFilesystem {
        database: DatabaseSettings,
        filesystems: Vec<FilesystemSettings>,
    },

    #[serde(rename = "dicomweb")]
    Dicomweb {
        dicomweb: DicomWebSettings,
    },
}

impl PacsConnectionSettings {
    pub fn type_name(&self) -> &'static str {
        match self {
            PacsConnectionSettings::DatabaseFilesystem { .. } => "database_filesystem",
            PacsConnectionSettings::Dicomweb { .. } => "dicomweb",
        }
    }
}


#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    Mysql,
    Postgres,
}

impl std::fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DatabaseType::Mysql => "mysql",
            DatabaseType::Postgres => "postgres",
        };

        write!(f, "{}", s)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct DatabaseSettings {
    pub r#type: DatabaseType,
    pub url: String,

    #[serde(default)]
    pub metadata_overrides: Vec<MetadataOverrideSettings>,
}

#[derive(Clone, Debug, Deserialize)]

pub struct DicomWebSettings {
    pub url: String,

    pub max_connections: Option<u32>,

    pub timeout_seconds: Option<u64>,
}


#[derive(Deserialize, Debug, Clone)]
pub struct MetadataOverrideSettings {
    pub keyword: String,
    pub source: String,
}


#[derive(Clone, Debug, Deserialize)]
pub struct FilesystemSettings {
    pub id: u32,
    pub path: PathBuf,
}

impl std::fmt::Display for PacsKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PacsKind::Dcm4chee2183 => "dcm4chee2183",
            PacsKind::Dcm4chee440 => "dcm4chee440",
        };
        write!(f, "{}", s)
    }
}





