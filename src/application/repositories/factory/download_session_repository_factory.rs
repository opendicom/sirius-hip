use std::sync::Arc;


use crate::application::infrastructure::mongodb::MongoDownloadSessionRepository;
use crate::application::infrastructure::mysql::MySqlDownloadSessionRepository;
use crate::application::infrastructure::mysql::download_session_repository::CleanupConfig;
use crate::errors::AppError;
use sqlx::MySqlPool;
use mongodb::Database;
use crate::application::repositories::DownloadSessionRepository;

pub struct DownloadSessionRepositoryFactory;

impl DownloadSessionRepositoryFactory {
    pub async fn from_mysql_pool(pool: MySqlPool, cleanup_cfg: CleanupConfig) -> Result<Arc<dyn DownloadSessionRepository>, AppError> {
        Ok(Arc::new(MySqlDownloadSessionRepository::new_with_config(pool, cleanup_cfg).await?))
    }

    pub fn from_mongodb(db: Database) -> Arc<dyn DownloadSessionRepository> {
        Arc::new(MongoDownloadSessionRepository::new(db))
    }
}
