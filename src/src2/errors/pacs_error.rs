use thiserror::Error;

#[derive(Debug, Error)]
pub enum PacsError {
    #[error("Database error")]
    Database(#[from] sqlx::Error),

    #[error("Unsupported database: {0}")]
    UnsupportedDatabase(String),
}
