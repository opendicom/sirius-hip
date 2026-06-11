use thiserror::Error;

#[derive(Error, Debug)]
pub enum PacsConnectorError {
    
    #[error("missing required triggers for PACS id={pacs_id}: {missing:?}")]
    MissingRequiredTriggers { 
        pacs_id: String,
        missing: Vec<String> 
    },

    #[error("mysql connection failed for PACS id={pacs_id}")]
    MysqlConnect {
        pacs_id: String,
        #[source]
        source: sqlx::Error,
    },

    #[error("postgres connection failed for PACS id={pacs_id}")]
    PostgresConnect {
        pacs_id: String,
        #[source]
        source: sqlx::Error,
    },

    #[error("PACS backend `{pacs_id}` is not configured")]
	BackendNotConfigured { pacs_id: String },

	#[error("missing required field `{field}` for operation `{operation}`")]
	MissingField {
        pacs_id: String,
		field: &'static str,
		operation: &'static str,
	},

	#[error("filesystem mapping is not configured for filesystem id {filesystem_id}")]
	FilesystemMappingMissing { 
        pacs_id: String,
        filesystem_id: i32 
    },

	#[error("invalid dicomweb base URL: {base_url} for PACS id={pacs_id}")]
	InvalidDicomwebBaseUrl { 
        pacs_id: String, 
        base_url: String 
    },

	#[error("unsupported PACS operation `{operation}` for backend id={pacs_id}: {reason}")]
	UnsupportedOperation {
		pacs_id: String,
		operation: &'static str,
		reason: &'static str,
	},

	#[error("network error")]
	Reqwest {
        pacs_id: String,
        #[source]
        source: reqwest::Error,
    },

	#[error("I/O error")]
	Io {
        pacs_id: String,
        #[source]
        source: std::io::Error,
    },

}
