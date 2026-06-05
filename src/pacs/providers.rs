use async_trait::async_trait;

use crate::{
    features::study_token::{entities::Study, StudySearchCriteria},
    pacs::{
        DicomObject,
        Instance,
        InstanceLocator,
        InstanceSearchCriteria,
        ObjectAccessContext,
        Series,
        SeriesSearchCriteria,
    },
};


#[async_trait]
pub trait MetadataProvider: Send + Sync {
    async fn search_studies(&self, criteria: &StudySearchCriteria) -> anyhow::Result<Vec<Study>>;

    async fn search_series(&self, criteria: &SeriesSearchCriteria) -> anyhow::Result<Vec<Series>>;

    async fn search_instances(&self, criteria: &InstanceSearchCriteria) -> anyhow::Result<Vec<Instance>>;
}


#[async_trait]
pub trait ObjectProvider: Send + Sync {
    async fn retrieve_instance(&self, locator: &InstanceLocator) -> anyhow::Result<DicomObject>;

    fn build_access_link(
        &self,
        locator: &InstanceLocator,
        context: &ObjectAccessContext,
    ) -> anyhow::Result<String>;
}