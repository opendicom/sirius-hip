use std::sync::Arc;
use thiserror::Error;

use crate::{bootstrap::{build_registry, pacs::PacsRegistryBuildError}, pacs::PacsRegistry, shared::config::AppSettings};


// -- Dependency Error implementations -------------------------------------------------------------------------------------- //

/// Errors that can occur during the building of application dependencies, such as the PACS registry.
#[derive(Debug, Error)]
pub enum DependencyBuildError {
    #[error("Failed to build PACS registry")]
    PacsRegistry(#[from] PacsRegistryBuildError),
}


// -- Dependency main implementations -------------------------------------------------------------------------------------- //

/// The collection of dependencies required by the application, such as the PACS registry and various services.
/// This struct is constructed during application startup and passed to various components that require access 
/// to these dependencies.
pub struct AppDependencies {
    //pub study_service: Arc<StudyService>,
    pub pacs_registry: Arc<PacsRegistry>,
}


/// Build the application dependencies based on the provided settings, such as constructing the PACS registry.
pub async fn build_dependencies(
    settings: &AppSettings,
) -> Result<AppDependencies, DependencyBuildError>
{
    let pacs_registry = build_registry(settings).await?;

    // let study_service =
    //     Arc::new(
    //         StudyService::new(
    //             pacs_registry.clone()
    //         )
    //     );

    Ok(
        AppDependencies {
            pacs_registry,
            //study_service,
        }
    )
}
