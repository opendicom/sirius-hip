use std::sync::Arc;

use crate::{features::study_token::entities::Study, pacs::{PacsRegistry, StudySearchCriteria}};

pub struct StudyService {
    registry: Arc<PacsRegistry>,
}


impl StudyService {
    pub fn new(registry: Arc<PacsRegistry>) -> Self {
        Self { registry }
    }

    pub async fn search(
        &self,
        criteria: StudySearchCriteria,
    )
    -> anyhow::Result<Vec<Study>>
    {
        self.registry
            .search_studies(&criteria)
            .await
    }
}