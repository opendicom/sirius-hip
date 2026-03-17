use std::sync::Arc;

use sqlx::MySqlPool;
use reqwest;

use crate::{
    application::repositories::DownloadSessionRepository, 
    pacs::repositories::StudyRepository
};
use crate::settings::Settings;


pub struct AppState2 {
    pub download_session_repo: Arc<dyn DownloadSessionRepository>,
    pub pacs: PacsState2,
    pub settings: Arc<Settings>,
    pub tmp_pool: MySqlPool,
    pub http_client: reqwest::Client,
}


pub struct PacsState2 {
    pub study_repo: Arc<dyn StudyRepository>,
}