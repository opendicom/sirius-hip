use std::sync::Arc;

use anyhow::anyhow;
use futures::future::try_join_all;

use crate::{
    features::study_token::entities::Study,
    pacs::{
        DicomObject,
        Instance,
        InstanceLocator,
        InstanceSearchCriteria,
        ObjectAccessContext,
        PacsConnector,
        Series,
        SeriesSearchCriteria, StudySearchCriteria,
    },
};

/// Central registry of all configured PACS connectors.
///
/// The registry acts as the entry point for PACS interactions within the
/// application. It maintains a collection of PACS-specific connector
/// implementations (e.g. DCM4CHEE v2 MySQL, DCM4CHEE v5 PostgreSQL,
/// Orthanc, Conquest) and provides a unified interface to access them.
///
/// Responsibilities:
/// - Store and manage all configured PACS connectors.
/// - Route requests to one or multiple PACS instances.
/// - Aggregate and merge results from multiple PACS when required.
/// - Hide PACS-specific implementation details from application services.
///
/// The registry does not contain PACS-specific business logic, SQL queries,
/// or infrastructure details. Those responsibilities belong to each
/// individual `PacsConnector` implementation.
///
/// Typical flow:
///
/// StudyService
///     ↓
/// PacsRegistry
///     ↓
/// PacsConnector
///     ↓
/// SQL / REST API / Filesystem
///
/// This allows application services to remain completely unaware of:
/// - PACS vendor
/// - PACS version
/// - Database engine
/// - Communication protocol
///
/// New PACS implementations can be added by registering additional
/// `PacsConnector` instances without modifying application services.
pub struct PacsRegistry {
    connectors: Vec<Arc<dyn PacsConnector>>,
}

impl PacsRegistry {

    /// Creates a new PACS registry with the given PACS connectors.
    pub fn new(connectors: Vec<Arc<dyn PacsConnector>>) -> Self {
        Self { connectors }
    }

    fn connector_by_id(&self, pacs_id: &str) -> anyhow::Result<Arc<dyn PacsConnector>> {
        self.connectors
            .iter()
            .find(|connector| connector.id() == pacs_id)
            .cloned()
            .ok_or_else(|| anyhow!("PACS backend `{pacs_id}` is not configured"))
    }
    
    /// Searches for studies across all connected PACS that match the given criteria.
    pub async fn search_studies(&self, criteria: &StudySearchCriteria) -> anyhow::Result<Vec<Study>> {

        let chunks = try_join_all(
            self.connectors
                .iter()
                .map(|connector| connector.search_studies(criteria)),
        )
        .await?;

        Ok(chunks.into_iter().flatten().collect())
    }

    /// Searches for series across all connected PACS that match the given criteria.
    pub async fn search_series(&self, criteria: &SeriesSearchCriteria) -> anyhow::Result<Vec<Series>> {

        let chunks = try_join_all(
            self.connectors
                .iter()
                .map(|connector| connector.search_series(criteria)),
        )
        .await?;

        Ok(chunks.into_iter().flatten().collect())
    }

    /// Searches for instances across all connected PACS that match the given criteria.
    pub async fn search_instances(
        &self,
        criteria: &InstanceSearchCriteria,
    ) -> anyhow::Result<Vec<Instance>> {
        let chunks = try_join_all(
            self.connectors
                .iter()
                .map(|connector| connector.search_instances(criteria)),
        )
        .await?;

        Ok(chunks.into_iter().flatten().collect())
    }

    /// Retrieves an instance from a specific PACS backend.
    pub async fn retrieve_instance(
        &self,
        pacs_id: &str,
        locator: &InstanceLocator,
    ) -> anyhow::Result<DicomObject> {
        
        let connector = self.connector_by_id(pacs_id)?;
        connector.retrieve_instance(locator).await
    }

    /// Builds an access link from a specific PACS backend.
    pub fn build_access_link(
        &self,
        pacs_id: &str,
        locator: &InstanceLocator,
        context: &ObjectAccessContext,
    ) -> anyhow::Result<String> {

        let connector = self.connector_by_id(pacs_id)?;
        connector.build_access_link(locator, context)
    }
}