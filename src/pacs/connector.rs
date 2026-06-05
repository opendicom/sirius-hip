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
    shared::config::PacsKind,
};

/// Contract implemented by every PACS integration supported by the system.
///
/// A `PacsConnector` represents a specific PACS compatibility implementation,
/// including its vendor, version, database engine, and communication
/// mechanism.
///
/// Examples:
/// - DCM4CHEE v2183 + MySQL
/// - DCM4CHEE v2183 + PostgreSQL
/// - DCM4CHEE v440 + MySQL
/// - DCM4CHEE v440 + PostgreSQL
///
/// Connectors encapsulate all infrastructure and compatibility concerns
/// required to interact with a particular PACS implementation. This may
/// include:
/// - SQL queries
/// - REST API calls
/// - Filesystem access
/// - Data normalization
/// - Version-specific behavior
/// - Vendor-specific mappings
///
/// Application services never interact with PACS implementations directly.
/// Instead, they communicate through the `PacsRegistry`, which delegates
/// requests to the appropriate connector(s).
///
/// The connector is responsible for translating PACS-specific data into
/// domain entities used by the application.
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
/// Implementations should be stateless whenever possible and safe to share
/// across threads, therefore the trait requires both `Send` and `Sync`.
#[async_trait]
pub trait PacsConnector: Send + Sync {

    /// Returns the configured PACS instance name.
    ///
    /// Example:
    /// - "hospital_a"
    /// - "radiology_center"
    fn id(&self) -> &str;

    /// Returns the PACS product type.
    ///
    /// Example:
    /// - "dcm4chee2183"
    /// - "dcm4chee440"
    /// - "orthanc110"
    fn pacs_kind(&self) -> PacsKind;

    /// Searches studies matching the specified criteria.
    ///
    /// The connector is responsible for translating the search criteria
    /// into the appropriate PACS-specific query mechanism and mapping
    /// the result into domain entities.
    async fn search_studies(
        &self,
        criteria: &StudySearchCriteria,
    ) -> anyhow::Result<Vec<Study>>;

    /// Searches series matching the specified criteria.
    async fn search_series(
        &self,
        criteria: &SeriesSearchCriteria,
    ) -> anyhow::Result<Vec<Series>>;

    /// Searches instances matching the specified criteria.
    async fn search_instances(
        &self,
        criteria: &InstanceSearchCriteria,
    ) -> anyhow::Result<Vec<Instance>>;

    /// Retrieves the DICOM bytes for a specific SOP instance.
    async fn retrieve_instance(
        &self,
        locator: &InstanceLocator,
    ) -> anyhow::Result<DicomObject>;

    /// Builds an access link for the provided SOP instance.
    fn build_access_link(
        &self,
        locator: &InstanceLocator,
        context: &ObjectAccessContext,
    ) -> anyhow::Result<String>;
}