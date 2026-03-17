use sqlx::mysql::MySqlPoolOptions;
use mongodb::{Client, Database};
use crate::application::repositories::DownloadSessionRepository;
use crate::settings::Settings;
use crate::application::repositories::factory::DownloadSessionRepositoryFactory;
use crate::application::infrastructure::mysql::download_session_repository::CleanupConfig;

pub async fn init_download_session_repo(settings: &Settings) -> anyhow::Result<std::sync::Arc<dyn DownloadSessionRepository>> {
    let url = settings.app_database_url.as_str();

    if url.starts_with("mysql://") {
        let mut options = MySqlPoolOptions::new();
        options = options.max_connections(settings.app_database_max_connections);
        // Keep this aligned with PACS pool behavior: fail fast on unavailable DB.
        options = options.acquire_timeout(std::time::Duration::from_secs(6));

        let pool = options.connect(url).await?;
        let cleanup_cfg = CleanupConfig {
            session_batch: settings.onetime_cleanup.session_batch.max(1) as usize,
            max_batches: settings.onetime_cleanup.max_batches.max(1) as usize,
            token_delete_limit: settings.onetime_cleanup.token_delete_limit.max(1) as usize,
        };
        Ok(DownloadSessionRepositoryFactory::from_mysql_pool(pool, cleanup_cfg).await?)
    } else if url.starts_with("mongodb://") {
        let client = Client::with_uri_str(url).await?;
        let db_name = "sirius_hip_db";
        let db: Database = client.database(db_name);
        Ok(DownloadSessionRepositoryFactory::from_mongodb(db))
    } else {
        anyhow::bail!("Unsupported database URL: {}", url)
    }
}