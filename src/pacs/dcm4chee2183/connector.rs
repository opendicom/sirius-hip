use std::sync::Arc;

use async_trait::async_trait;
use sqlx::MySqlPool;

use crate::{
    features::study_token::{entities::Study, StudySearchCriteria},
    pacs::{
        DicomObject,
        Instance,
        InstanceLocator,
        InstanceSearchCriteria,
        MetadataProvider,
        ObjectAccessContext,
        ObjectProvider,
        PacsConnector,
        Series,
        SeriesSearchCriteria,
    },
    shared::config::PacsKind,
};

use super::{metadata::Dcm4chee2183MysqlMetadataProvider, objects::Dcm4chee2183DicomWebObjectProvider};

pub struct Dcm4chee2183MySqlConnector {
    id: String,

    metadata_provider:
        Arc<dyn MetadataProvider>,

    object_provider:
        Arc<dyn ObjectProvider>,
}

impl Dcm4chee2183MySqlConnector {
    pub fn new(id: String, pool: MySqlPool, wadouri: String) -> Self {
        let object_provider: Arc<dyn ObjectProvider> =
            Arc::new(Dcm4chee2183DicomWebObjectProvider::new(reqwest::Client::new(), wadouri));

        Self::new_with_object_provider(id, pool, object_provider)
    }

    pub fn new_with_object_provider(
        id: String,
        pool: MySqlPool,
        object_provider: Arc<dyn ObjectProvider>,
    ) -> Self {
        let metadata_provider: Arc<dyn MetadataProvider> =
            Arc::new(Dcm4chee2183MysqlMetadataProvider::new(pool));

        Self {
            id,
            metadata_provider,
            object_provider,
        }
    }

    pub fn with_providers(
        id: String,
        metadata_provider: Arc<dyn MetadataProvider>,
        object_provider: Arc<dyn ObjectProvider>,
    ) -> Self {
        Self {
            id,
            metadata_provider,
            object_provider,
        }
    }
}

#[async_trait]
impl PacsConnector for Dcm4chee2183MySqlConnector {
    fn id(&self) -> &str {
        &self.id
    }

    fn pacs_kind(&self) -> PacsKind {
        PacsKind::Dcm4chee2183
    }

    async fn search_studies(
        &self,
        criteria: &StudySearchCriteria,
    ) -> anyhow::Result<Vec<Study>> {
        self.metadata_provider.search_studies(criteria).await
    }

    async fn search_series(
        &self,
        criteria: &SeriesSearchCriteria,
    ) -> anyhow::Result<Vec<Series>> {
        self.metadata_provider.search_series(criteria).await
    }

    async fn search_instances(
        &self,
        criteria: &InstanceSearchCriteria,
    ) -> anyhow::Result<Vec<Instance>> {
        self.metadata_provider.search_instances(criteria).await
    }

    async fn retrieve_instance(
        &self,
        locator: &InstanceLocator,
    ) -> anyhow::Result<DicomObject> {
        self.object_provider.retrieve_instance(locator).await
    }

    fn build_access_link(
        &self,
        locator: &InstanceLocator,
        context: &ObjectAccessContext,
    ) -> anyhow::Result<String> {
        self.object_provider.build_access_link(locator, context)
    }
}