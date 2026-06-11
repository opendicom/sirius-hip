use async_trait::async_trait;
use sqlx::PgPool;

use crate::features::study_token::entities::Study;
use crate::pacs::{
    Instance, 
    InstanceSearchCriteria, 
    MetadataProvider, 
	PacsConnectorError,
    Series, 
    SeriesSearchCriteria, 
    StudySearchCriteria,
};


#[allow(dead_code)]
#[derive(Clone)]
pub struct Dcm4chee2183PostgresMetadataProvider {
	pacs_id: String,
	_pool: PgPool,
}

#[allow(dead_code)]
impl Dcm4chee2183PostgresMetadataProvider {
	pub fn new(pacs_id: String, pool: PgPool) -> Self {
		Self { pacs_id, _pool: pool }
	}
}


#[async_trait]
impl MetadataProvider for Dcm4chee2183PostgresMetadataProvider {

	async fn require_dirty_triggers(&self) -> Result<(), PacsConnectorError> {
		Err(PacsConnectorError::UnsupportedOperation {
			pacs_id: self.pacs_id.clone(),
			operation: "require_dirty_triggers",
			reason: "dcm4chee2183 + postgres metadata provider is not implemented yet",
		})
	}
	
	async fn search_studies(&self, _criteria: &StudySearchCriteria) -> Result<Vec<Study>, PacsConnectorError> {
		Err(PacsConnectorError::UnsupportedOperation {
			pacs_id: self.pacs_id.clone(),
			operation: "search_studies",
			reason: "dcm4chee2183 + postgres metadata provider is not implemented yet",
		})
	}

	async fn search_series(&self, _criteria: &SeriesSearchCriteria) -> Result<Vec<Series>, PacsConnectorError> {
		Err(PacsConnectorError::UnsupportedOperation {
			pacs_id: self.pacs_id.clone(),
			operation: "search_series",
			reason: "dcm4chee2183 + postgres metadata provider is not implemented yet",
		})
	}

	async fn search_instances(&self, _criteria: &InstanceSearchCriteria) -> Result<Vec<Instance>, PacsConnectorError> {
		Err(PacsConnectorError::UnsupportedOperation {
			pacs_id: self.pacs_id.clone(),
			operation: "search_instances",
			reason: "dcm4chee2183 + postgres metadata provider is not implemented yet",
		})
	}
}