use async_trait::async_trait;
use sqlx::PgPool;

use crate::errors::PacsError;
use crate::pacs::read_models::QidoStudyRow;
use crate::pacs::read_models::StudyTokenRow;
use crate::pacs::repositories::StudyRepository;
use crate::pacs::repositories::study_repository::{
    QidoStudiesIncludeFields, QidoStudiesSearchCriteria, StudyTokenSearchCriteria,
};

pub struct Dcm4chee2183PostgresStudyRepository {
    _pool: PgPool,
}

impl Dcm4chee2183PostgresStudyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { _pool: pool }
    }
}

#[async_trait]
impl StudyRepository for Dcm4chee2183PostgresStudyRepository {
    async fn fetch_study_token_rows(
        &self,
        criteria: StudyTokenSearchCriteria<'_>,
        include_filesystem: bool,
        include_ohif_metadata: bool,
        include_weasis_metadata: bool,
    ) -> Result<Vec<StudyTokenRow>, PacsError> {
        let _ = (
            criteria,
            include_filesystem,
            include_ohif_metadata,
            include_weasis_metadata,
        );
        Err(PacsError::UnsupportedDatabase("postgres".to_string()))
    }

    async fn fetch_qido_studies_rows(
        &self,
        _criteria: QidoStudiesSearchCriteria<'_>,
        _include: QidoStudiesIncludeFields,
    ) -> Result<Vec<QidoStudyRow>, PacsError> {
        Err(PacsError::UnsupportedDatabase("postgres".to_string()))
    }
}
