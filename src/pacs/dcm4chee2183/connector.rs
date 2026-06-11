use async_trait::async_trait;

use crate::features::study_token::entities::Study;
use crate::shared::config::PacsKind;
use crate::pacs::{
    DicomObject, 
    Instance, 
    InstanceLocator, 
    InstanceSearchCriteria, 
    MetadataProvider, 
    ObjectAccessContext, 
    ObjectProvider, 
    PacsConnector, 
    PacsConnectorError,
    Series, 
    SeriesSearchCriteria, 
    StudySearchCriteria
};

pub struct Dcm4chee2183Connector {
    id: String,
    metadata_provider: Box<dyn MetadataProvider>,
    object_provider: Box<dyn ObjectProvider>,
}

impl Dcm4chee2183Connector {
    
    /// Creates a new instance of the DCM4CHEE v2183 connector with the specified providers.
    pub fn new(
        id: String,
        metadata_provider: Box<dyn MetadataProvider>,
        object_provider: Box<dyn ObjectProvider>,
    ) -> Self {
        Self {
            id,
            metadata_provider,
            object_provider,
        }
    }
}

#[async_trait]
impl PacsConnector for Dcm4chee2183Connector {
    fn id(&self) -> &str {
        &self.id
    }

    fn pacs_kind(&self) -> PacsKind {
        PacsKind::Dcm4chee2183
    }

    async fn require_dirty_triggers(&self) -> Result<(), PacsConnectorError> {
        self.metadata_provider.require_dirty_triggers().await
    }

    async fn search_studies(
        &self,
        criteria: &StudySearchCriteria,
    ) -> Result<Vec<Study>, PacsConnectorError> {

        self.metadata_provider.search_studies(criteria).await
    }

    async fn search_series(
        &self,
        criteria: &SeriesSearchCriteria,
    ) -> Result<Vec<Series>, PacsConnectorError> {

        self.metadata_provider.search_series(criteria).await
    }

    async fn search_instances(
        &self,
        criteria: &InstanceSearchCriteria,
    ) -> Result<Vec<Instance>, PacsConnectorError> {
        
        self.metadata_provider.search_instances(criteria).await
    }

    async fn retrieve_instance(
        &self,
        locator: &InstanceLocator,
    ) -> Result<DicomObject, PacsConnectorError> {
        
        self.object_provider.retrieve_instance(locator).await
    }

    fn build_access_link(
        &self,
        locator: &InstanceLocator,
        context: &ObjectAccessContext,
    ) -> Result<String, PacsConnectorError> {

        self.object_provider.build_access_link(locator, context)
    }
}