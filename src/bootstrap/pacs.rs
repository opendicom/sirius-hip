use std::{collections::HashMap, path::PathBuf, sync::Arc};

use sqlx::mysql::MySqlPoolOptions;
use thiserror::Error;
use tracing::debug;

use crate::{
    pacs::{
        dcm4chee2183::{
            Dcm4chee2183DicomWebObjectProvider,
            Dcm4chee2183FilesystemObjectProvider,
            Dcm4chee2183MySqlConnector,
        },
        ObjectProvider,
        PacsConnector,
        PacsRegistry,
    },
    shared::config::{
        AppSettings,
        PacsConnectionSettings,
        PacsFilesystemSettings,
        PacsKind,
        PacsObjectMode,
        PacsSettings,
    },
};


// -- Bootstrap PACS Registry Error implementations -------------------------------------------------------------------------------------- //

/// Errors that can occur during the building of the PACS registry, such as unsupported configurations or database 
/// connection failures.
#[derive(Debug, Error)]
pub enum PacsRegistryBuildError {
    #[error("unsupported PACS config for id={pacs_id}, kind={kind}, connection={connection_type}")]
    UnsupportedConfig {
        pacs_id: String,
        kind: PacsKind,
        connection_type: String,
    },

    #[error("mysql connection failed for PACS id={pacs_id}")]
    MysqlConnect {
        pacs_id: String,
        #[source]
        source: sqlx::Error,
    },

    #[error("invalid PACS config for id={pacs_id}: {reason}")]
    InvalidConfig {
        pacs_id: String,
        reason: String,
    },

}

fn build_filesystem_root_map(filesystems: &[PacsFilesystemSettings]) -> HashMap<i32, PathBuf> {
    filesystems
        .iter()
        .map(|filesystem| (filesystem.id, PathBuf::from(&filesystem.path)))
        .collect()
}


// -- Bootstrap PACS Registry implementations -------------------------------------------------------------------------------------- //

/// Build the PACS Registry based on the provided application settings, constructing connectors for each configured PACS.
pub async fn build_registry(
    settings: &AppSettings,
) -> Result<Arc<PacsRegistry>, PacsRegistryBuildError>
{
    let mut connectors = Vec::new();

    for pacs in &settings.pacs {
        connectors.push(create_connector(pacs).await?);
    }

    Ok(
        Arc::new(
            PacsRegistry::new(
                connectors
            )
        )
    )
}


/// Factory function to create a PACS connector based on the provided configuration
pub async fn create_connector(
    config: &PacsSettings,
) -> Result<Arc<dyn PacsConnector>, PacsRegistryBuildError>
{
    debug!(id = %config.id, kind = %config.kind, r#type = %config.connection.type_name(), "Creating PACS Connector");
    
    match (&config.kind, &config.connection) {

        (
            PacsKind::Dcm4chee2183, 
            PacsConnectionSettings::Mysql {
                url,
                wadouri,
                object_mode,
                filesystems,
            },
        ) => {

            let pool = MySqlPoolOptions::new()
                .acquire_timeout(std::time::Duration::from_secs(2))
                .connect(url)
                .await
                .map_err(|source| PacsRegistryBuildError::MysqlConnect {
                    pacs_id: config.id.clone(),
                    source,
                })?;

            let object_provider: Arc<dyn ObjectProvider> = match object_mode {
                PacsObjectMode::Dicomweb => Arc::new(Dcm4chee2183DicomWebObjectProvider::new(
                    reqwest::Client::new(),
                    wadouri.clone(),
                )),

                PacsObjectMode::Filesystem => {
                    if filesystems.is_empty() {
                        return Err(PacsRegistryBuildError::InvalidConfig {
                            pacs_id: config.id.clone(),
                            reason: "object_mode=filesystem requires at least one filesystem mapping".to_string(),
                        });
                    }

                    Arc::new(Dcm4chee2183FilesystemObjectProvider::new(
                        build_filesystem_root_map(filesystems),
                    ))
                }
            };

            Ok(Arc::new(Dcm4chee2183MySqlConnector::new_with_object_provider(
                config.id.clone(),
                pool,
                object_provider,
            )))
        }

        // (
        //     PacsKind::Dcm4chee440,
        //     PacsConnectionSettings::Mysql {
        //         url,
        //         wadouri,
        //     },
        // ) => {

        //     let pool =
        //         sqlx::MySqlPool::connect(url)
        //             .await?;

        //     Ok(
        //         Arc::new(
        //             Dcm4chee440MySqlConnector::new(
        //                 config.id.clone(),
        //                 pool,
        //                 wadouri.clone(),
        //             )
        //         )
        //     )
        // }

        // (
        //     PacsKind::Siriuship,
        //     PacsConnectionSettings::Rest {
        //         url,
        //     },
        // ) => {

        //     let client =
        //         reqwest::Client::new();

        //     Ok(
        //         Arc::new(
        //             SiriusHipConnector::new(
        //                 config.id.clone(),
        //                 client,
        //                 url.clone(),
        //             )
        //         )
        //     )
        // }

        _ => {
            Err(PacsRegistryBuildError::UnsupportedConfig {
                pacs_id: config.id.clone(),
                kind: config.kind.clone(),
                connection_type: config.connection.type_name().to_string(),
            })
        }
    }
}


