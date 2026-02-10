use async_trait::async_trait;
use mongodb::Database;

use crate::src2::application::repositories::DownloadSessionRepository;
use crate::src2::application::models::{
    DownloadSession,
    DownloadSessionFile,
};
use crate::src2::errors::AppError;

pub struct MongoDownloadSessionRepository {
    _db: Database,
}

impl MongoDownloadSessionRepository {
    pub fn new(db: Database) -> Self {
        Self { _db: db }
    }
}



#[async_trait]
impl DownloadSessionRepository for MongoDownloadSessionRepository {

    async fn create_session(&self, _session: &DownloadSession) -> Result<(), AppError> {
        // TODO: Implement this method
        Err(AppError::Internal(anyhow::anyhow!("Not implemented")))
    }

    async fn add_files(&self, _files: &[DownloadSessionFile]) -> Result<(), AppError> {
        // TODO: Implement this method
        Err(AppError::Internal(anyhow::anyhow!("Not implemented")))
    }

    async fn get_file(&self, _session_id: &str, _file_index: u32) -> Result<DownloadSessionFile, AppError> {
        // TODO: Implement this method
        Err(AppError::Internal(anyhow::anyhow!("Not implemented")))
    }

    async fn consume_session(&self, _session_id: &str) -> Result<(), AppError> {
        // TODO: Implement this method
        Err(AppError::Internal(anyhow::anyhow!("Not implemented")))
    }

    async fn claim_file(&self, _session_id: &str, _file_index: u32) -> Result<DownloadSessionFile, AppError> {
        // TODO: Implement this method
        Err(AppError::Internal(anyhow::anyhow!("Not implemented")))
    }

}



