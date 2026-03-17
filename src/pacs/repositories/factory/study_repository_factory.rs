use std::sync::Arc;

use sqlx::mysql::MySqlPoolOptions;
use sqlx::postgres::PgPoolOptions;

use chrono::NaiveDateTime;

use crate::settings::{DBVersion, Settings};
use crate::errors::PacsError;
use crate::pacs::repositories::StudyRepository;
use crate::pacs::infrastructure::dcm4chee2183::{
    mysql::Dcm4chee2183MySqlStudyRepository,
    postgres::Dcm4chee2183PostgresStudyRepository,
};
use crate::pacs::infrastructure::dcm4chee440::{
    mysql::Dcm4chee440MySqlStudyRepository,
    postgres::Dcm4chee440PostgresStudyRepository,
};

pub struct StudyRepositoryFactory;

impl StudyRepositoryFactory {
    pub async fn create(
        settings: &Settings,
    ) -> Result<Arc<dyn StudyRepository>, PacsError> {

        let archive = &settings.dicomarchive;
        let db_url = archive.database_url.as_str();

        match archive.version {
            DBVersion::dcm4chee2183 => {
                Self::create_2183(db_url, archive.database_max_connections, archive.filesystem_cutoff_date).await
            }

            DBVersion::dcm4chee440 => {
                Self::create_440(db_url, archive.database_max_connections, archive.filesystem_cutoff_date).await
            }
        }
    }

    async fn create_2183(
        database_url: &str,
        max_connections: u32,
        filesystem_cutoff_date: Option<NaiveDateTime>,
    ) -> Result<Arc<dyn StudyRepository>, PacsError> {

        if database_url.starts_with("mysql://") {
            let pool = MySqlPoolOptions::new()
                .max_connections(max_connections)
                .connect(database_url)
                .await?;
            let repo_impl = Dcm4chee2183MySqlStudyRepository::new(pool, filesystem_cutoff_date).await?;
            let repo: Arc<dyn StudyRepository> = Arc::new(repo_impl);
            Ok(repo)
        } else if database_url.starts_with("postgres://") {
            let pool = PgPoolOptions::new()
                .max_connections(max_connections)
                .connect(database_url)
                .await?;
            let repo: Arc<dyn StudyRepository> = Arc::new(
                Dcm4chee2183PostgresStudyRepository::new(pool),
            );
            Ok(repo)
        } else {
            Err(PacsError::UnsupportedDatabase(database_url.into()))
        }
    }

    async fn create_440(
        database_url: &str,
        max_connections: u32,
        filesystem_cutoff_date: Option<NaiveDateTime>,
    ) -> Result<Arc<dyn StudyRepository>, PacsError> {

        if database_url.starts_with("mysql://") {
            let pool = MySqlPoolOptions::new()
                .max_connections(max_connections)
                .connect(database_url)
                .await?;
            let repo_impl = Dcm4chee440MySqlStudyRepository::new(pool, filesystem_cutoff_date).await?;
            let repo: Arc<dyn StudyRepository> = Arc::new(repo_impl);
            Ok(repo)
        } else if database_url.starts_with("postgres://") {
            let pool = PgPoolOptions::new()
                .max_connections(max_connections)
                .connect(database_url)
                .await?;
            let repo: Arc<dyn StudyRepository> = Arc::new(
                Dcm4chee440PostgresStudyRepository::new(pool),
            );
            Ok(repo)
        } else {
            Err(PacsError::UnsupportedDatabase(database_url.into()))
        }
    }
}
