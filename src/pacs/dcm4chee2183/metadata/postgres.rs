use async_trait::async_trait;
use sqlx::PgPool;

use crate::features::study_token::entities::Study;
use crate::pacs::{
    Instance, 
    InstanceSearchCriteria, 
    MetadataProvider, 
    Series, 
    SeriesSearchCriteria, 
    StudySearchCriteria,
};


#[allow(dead_code)]
#[derive(Clone)]
pub struct Dcm4chee2183PostgresMetadataProvider {
	_pool: PgPool,
}

#[allow(dead_code)]
impl Dcm4chee2183PostgresMetadataProvider {
	pub fn new(pool: PgPool) -> Self {
		Self { _pool: pool }
	}
}


#[async_trait]
impl MetadataProvider for Dcm4chee2183PostgresMetadataProvider {
	
	async fn search_studies(&self, _criteria: &StudySearchCriteria) -> anyhow::Result<Vec<Study>> {
		anyhow::bail!("dcm4chee2183 + postgres metadata provider is not implemented yet")
	}

	async fn search_series(&self, _criteria: &SeriesSearchCriteria) -> anyhow::Result<Vec<Series>> {
		anyhow::bail!("dcm4chee2183 + postgres metadata provider is not implemented yet")
	}

	async fn search_instances(&self, _criteria: &InstanceSearchCriteria) -> anyhow::Result<Vec<Instance>> {
		anyhow::bail!("dcm4chee2183 + postgres metadata provider is not implemented yet")
	}
}