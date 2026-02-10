use async_trait::async_trait;
use sqlx::PgPool;

use crate::src2::errors::PacsError;
use crate::src2::pacs::read_models::StudyTokenRow;
use crate::src2::pacs::read_models::QidoStudyRow;
use crate::src2::pacs::repositories::StudyRepository;
use crate::src2::pacs::repositories::study_repository::{
    QidoStudiesIncludeFields, QidoStudiesQuery, StudyTokenQuery,
};

pub struct Dcm4chee440PostgresStudyRepository {
    _pool: PgPool,
}

impl Dcm4chee440PostgresStudyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { _pool: pool }
    }
}

#[async_trait]
impl StudyRepository for Dcm4chee440PostgresStudyRepository {
    async fn fetch_study_token_rows(
        &self,
        _query: StudyTokenQuery<'_>,
        _include_filesystem: bool,
        _include_wado: bool,
    ) -> Result<Vec<StudyTokenRow>, PacsError> {
        Err(PacsError::UnsupportedDatabase("postgres".to_string()))
    }

    async fn fetch_qido_studies_rows(
        &self,
        _query: QidoStudiesQuery<'_>,
        _include: QidoStudiesIncludeFields,
    ) -> Result<Vec<QidoStudyRow>, PacsError> {
        Err(PacsError::UnsupportedDatabase("postgres".to_string()))
    }
}
