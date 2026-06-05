use std::sync::Arc;

use crate::{bootstrap::dependencies::AppDependencies, features::study_token::StudyService, shared::config::AppSettings};


#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<AppSettings>,
    pub study_service: Arc<StudyService>,
}

impl AppState {
    
    pub fn from_parts(
        settings: AppSettings,
        dependencies: AppDependencies,
    ) -> Self
    {
        let study_service = Arc::new(StudyService::new(dependencies.pacs_registry));

        Self {
            settings: Arc::new(settings),
            study_service,
        }
    }
}