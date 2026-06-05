use std::fs;
use std::path::PathBuf;

use thiserror::Error;

use super::settings::AppSettings;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Read(#[from] std::io::Error),

    #[error("failed to parse config TOML: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("invalid config: {0}")]
    Validation(String),
}

pub fn load_settings(path: PathBuf) -> Result<AppSettings, ConfigError> {
    
    if path.exists() {
        let raw = fs::read_to_string(&path)?;
        let settings = toml::from_str::<AppSettings>(&raw)?;
        settings
            .validate()
            .map_err(ConfigError::Validation)?;
        return Ok(settings);
    } else {
        Err(ConfigError::Read(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("config file not found at `{}`", path.display()),
        )))
    }
}

