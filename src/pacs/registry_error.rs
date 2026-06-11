use thiserror::Error;

use crate::{pacs::PacsConnectorError, shared::config::PacsKind};

#[derive(Debug, Error)]
pub enum PacsRegistryError {

    #[error("invalid PACS config for id={pacs_id}: {reason}")]
    InvalidConfig {
        pacs_id: String,
        reason: String,
    },

    #[error("unsupported PACS config for id={pacs_id}, kind={kind}, connection={connection_type}")]
    UnsupportedConfig {
        pacs_id: String,
        kind: PacsKind,
        connection_type: String,
    },

    #[error("failed to build DICOMweb HTTP client for PACS id={pacs_id}")]
    DicomwebClientBuild {
        pacs_id: String,
        #[source]
        source: reqwest::Error,
    },
    
    #[error("PACS Connector error id={pacs_id}")]
    PacsConnectorError {
        pacs_id: String,
        #[source]
        source: PacsConnectorError,
    },

}