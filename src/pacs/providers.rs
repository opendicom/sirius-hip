use async_trait::async_trait;

use crate::{
    features::study_token::{entities::Study},
    pacs::{
        DicomObject,
        Instance,
        InstanceLocator,
        InstanceSearchCriteria,
        StudySearchCriteria,
        ObjectAccessContext,
        PacsConnectorError,
        Series,
        SeriesSearchCriteria,
    },
};


#[async_trait]
pub trait MetadataProvider: Send + Sync {

    async fn require_dirty_triggers(&self) -> Result<(), PacsConnectorError>;

    async fn search_studies(&self, criteria: &StudySearchCriteria) -> Result<Vec<Study>, PacsConnectorError>;

    async fn search_series(&self, criteria: &SeriesSearchCriteria) -> Result<Vec<Series>, PacsConnectorError>;

    async fn search_instances(&self, criteria: &InstanceSearchCriteria) -> Result<Vec<Instance>, PacsConnectorError>;
}


#[async_trait]
pub trait ObjectProvider: Send + Sync {
    async fn retrieve_instance(&self, locator: &InstanceLocator) -> Result<DicomObject, PacsConnectorError>;

    fn build_access_link(
        &self,
        locator: &InstanceLocator,
        context: &ObjectAccessContext,
    ) -> Result<String, PacsConnectorError>;
}