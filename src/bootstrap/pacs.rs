use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use sqlx::{mysql::MySqlPoolOptions, postgres::PgPoolOptions};
use tracing::debug;

use crate::{
    pacs::{
        MetadataProvider, PacsConnector, PacsConnectorError, PacsRegistry, PacsRegistryError, dcm4chee2183::{
            Dcm4chee2183Connector,
            Dcm4chee2183DicomWebMetadataProvider,
            Dcm4chee2183DicomWebObjectProvider,
            Dcm4chee2183FilesystemObjectProvider,
            Dcm4chee2183MysqlMetadataProvider,
            Dcm4chee2183PostgresMetadataProvider,
        }
    },
    shared::config::{
        AppSettings,
        DatabaseType,
        DicomWebSettings,
        FilesystemSettings,
        PacsConnectionSettings,
        PacsKind,
        PacsSettings,
    },
};

fn build_filesystem_root_map(
    filesystems: &[FilesystemSettings],
    pacs_id: &str,
) -> Result<HashMap<i32, PathBuf>, PacsRegistryError> {
    let mut by_id = HashMap::new();

    for filesystem in filesystems {
        let filesystem_id = i32::try_from(filesystem.id).map_err(|_| {
            PacsRegistryError::InvalidConfig {
                pacs_id: pacs_id.to_string(),
                reason: format!("filesystem id {} is out of i32 range", filesystem.id),
            }
        })?;

        by_id.insert(filesystem_id, PathBuf::from(&filesystem.path));
    }

    Ok(by_id)
}

fn build_dicomweb_http_client(
    settings: &DicomWebSettings,
    pacs_id: &str,
) -> Result<reqwest::Client, PacsRegistryError> {
    let max_connections = settings.max_connections.unwrap_or(10) as usize;
    let timeout = Duration::from_secs(settings.timeout_seconds.unwrap_or(30));

    reqwest::Client::builder()
        .pool_max_idle_per_host(max_connections)
        .timeout(timeout)
        .build()
        .map_err(|source| PacsRegistryError::DicomwebClientBuild {
            pacs_id: pacs_id.to_string(),
            source,
        })
}

pub async fn build_registry(
    settings: &AppSettings,
) -> Result<Arc<PacsRegistry>, PacsRegistryError> {
    let mut connectors = Vec::new();

    for pacs in &settings.pacs {
        connectors.push(create_connector(pacs).await?);
    }

    Ok(Arc::new(PacsRegistry::new(connectors)))
}

pub async fn create_connector(
    config: &PacsSettings,
) -> Result<Arc<dyn PacsConnector>, PacsRegistryError> {
    debug!(
        id = %config.id,
        kind = %config.kind,
        r#type = %config.connection.type_name(),
        "Creating PACS connector"
    );

    match (&config.kind, &config.connection) {
        (
            PacsKind::Dcm4chee2183,
            PacsConnectionSettings::DatabaseFilesystem {
                database,
                filesystems,
            },
        ) => {

            let metadata_provider: Box<dyn MetadataProvider> = match database.r#type {
                DatabaseType::Mysql => {
                    let pool = MySqlPoolOptions::new()
                        .acquire_timeout(Duration::from_secs(2))
                        .connect(&database.url)
                        .await
                        .map_err(|source| PacsConnectorError::MysqlConnect {
                            pacs_id: config.id.clone(),
                            source,
                        })
                        .map_err(|source| PacsRegistryError::PacsConnectorError {
                            pacs_id: config.id.clone(),
                            source,
                        })?;

                    Box::new(Dcm4chee2183MysqlMetadataProvider::new(config.id.clone(), pool))
                }

                DatabaseType::Postgres => {
                    let pool = PgPoolOptions::new()
                        .acquire_timeout(Duration::from_secs(2))
                        .connect(&database.url)
                        .await
                        .map_err(|source| PacsConnectorError::PostgresConnect {
                            pacs_id: config.id.clone(),
                            source,
                        })
                        .map_err(|source| PacsRegistryError::PacsConnectorError {
                            pacs_id: config.id.clone(),
                            source,
                        })?;

                    Box::new(Dcm4chee2183PostgresMetadataProvider::new(config.id.clone(), pool))
                }
            };

            let filesystem_roots = build_filesystem_root_map(filesystems, &config.id)?;
            let object_provider = Box::new(Dcm4chee2183FilesystemObjectProvider::new(config.id.clone(), filesystem_roots));

            Ok(Arc::new(Dcm4chee2183Connector::new(
                config.id.clone(),
                metadata_provider,
                object_provider,
            )))
        }

        (PacsKind::Dcm4chee2183, PacsConnectionSettings::Dicomweb { dicomweb }) => {
            let http_client = build_dicomweb_http_client(dicomweb, &config.id)?;

            let metadata_provider = Box::new(
                Dcm4chee2183DicomWebMetadataProvider::new(config.id.clone(), http_client.clone(), dicomweb.url.clone())
            );

            let object_provider = Box::new(
                Dcm4chee2183DicomWebObjectProvider::new(config.id.clone(), http_client, dicomweb.url.clone())
            );

            Ok(Arc::new(Dcm4chee2183Connector::new(
                config.id.clone(),
                metadata_provider,
                object_provider,
            )))
        }

        _ => Err(PacsRegistryError::UnsupportedConfig {
            pacs_id: config.id.clone(),
            kind: config.kind,
            connection_type: config.connection.type_name().to_string(),
        }),
    }
}
