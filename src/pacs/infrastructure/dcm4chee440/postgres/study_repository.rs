use async_trait::async_trait;
use sqlx::PgPool;

use crate::errors::PacsError;
use crate::pacs::read_models::StudyTokenRow;
use crate::pacs::read_models::QidoStudyRow;
use crate::pacs::repositories::StudyRepository;
use crate::pacs::repositories::study_repository::{
    QidoStudiesIncludeFields, QidoStudiesSearchCriteria, StudyTokenSearchCriteria,
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
        _criteria: StudyTokenSearchCriteria<'_>,
        _include_filesystem: bool,
        _include_ohif_metadata: bool,
        _include_weasis_metadata: bool,
    ) -> Result<Vec<StudyTokenRow>, PacsError> {
        Err(PacsError::UnsupportedDatabase("postgres".to_string()))
    }

    async fn fetch_qido_studies_rows(
        &self,
        _query: QidoStudiesSearchCriteria<'_>,
        _include: QidoStudiesIncludeFields,
    ) -> Result<Vec<QidoStudyRow>, PacsError> {
        Err(PacsError::UnsupportedDatabase("postgres".to_string()))
    }
}
