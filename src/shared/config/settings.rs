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

            match &pacs.connection {
                PacsConnectionSettings::Mysql {
                    object_mode,
                    filesystems,
                    ..
                }
                | PacsConnectionSettings::Postgres {
                    object_mode,
                    filesystems,
                    ..
                } => {
                    if matches!(object_mode, PacsObjectMode::Filesystem) {
                        if filesystems.is_empty() {
                            return Err(format!(
                                "PACS backend `{}` requires at least one filesystem mapping when object_mode=filesystem",
                                pacs.id
                            ));
                        }

                        let mut filesystem_ids = std::collections::HashSet::new();

                        for filesystem in filesystems {
                            if filesystem.path.trim().is_empty() {
                                return Err(format!(
                                    "PACS backend `{}` has a filesystem mapping with empty path",
                                    pacs.id
                                ));
                            }

                            if !filesystem_ids.insert(filesystem.id) {
                                return Err(format!(
                                    "PACS backend `{}` has duplicated filesystem id `{}`",
                                    pacs.id, filesystem.id
                                ));
                            }
                        }
                    }
                }

                PacsConnectionSettings::Rest { .. } => {}
            }
        }

        Ok(())
    }

    pub fn public_view(&self) -> PublicSettings {
        PublicSettings {
            server_bind: self.server.bind.clone(),
            jwt_mode: self.jwt.mode,
            jwt_secret: "******* (hidden)".to_string(),
            pacs: self
                .pacs
                .iter()
                .map(|backend| PublicPacsBackendSettings {
                    id: backend.id.clone(),
                    kind: backend.kind,
                })
                .collect(),
        }
    }
}

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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PacsKind {
    Dcm4chee2183,
    Dcm4chee440,
}


#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PacsObjectMode {
    #[default]
    Dicomweb,
    Filesystem,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PacsFilesystemSettings {
    pub id: i32,
    pub path: String,
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



#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PacsSettings {
    pub id: String,
    pub kind: PacsKind,
    pub connection: PacsConnectionSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PacsConnectionSettings {

    #[serde(rename = "mysql")]
    Mysql {
        url: String,
        wadouri: String,
        #[serde(default)]
        object_mode: PacsObjectMode,
        #[serde(default)]
        filesystems: Vec<PacsFilesystemSettings>,
    },

    #[serde(rename = "postgres")]
    Postgres {
        url: String,
        wadouri: String,
        #[serde(default)]
        object_mode: PacsObjectMode,
        #[serde(default)]
        filesystems: Vec<PacsFilesystemSettings>,
    },

    #[serde(rename = "rest")]
    Rest {
        url: String,
    },
}

impl PacsConnectionSettings {
    pub fn type_name(&self) -> &'static str {
        match self {
            PacsConnectionSettings::Mysql { .. } => "mysql",
            PacsConnectionSettings::Postgres { .. } => "postgres",
            PacsConnectionSettings::Rest { .. } => "rest",
        }
    }
}


#[derive(Clone, Debug, Serialize)]
pub struct PublicSettings {
    pub server_bind: String,
    pub jwt_mode: JwtAuthMode,
    pub jwt_secret: String,
    pub pacs: Vec<PublicPacsBackendSettings>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PublicPacsBackendSettings {
    pub id: String,
    pub kind: PacsKind,
}



