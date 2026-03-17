use thiserror::Error;

#[derive(Debug, Error)]
pub enum PacsError {
    #[error("Database error")]
    Database(#[from] sqlx::Error),

    #[error("Missing required MySQL triggers in PACS DB: {missing:?}")]
    MissingRequiredTriggers { missing: Vec<String> },

    #[error("Unsupported database: {0}")]
    UnsupportedDatabase(String),
}
