use crate::{bootstrap::dependencies::AppDependencies, features::study_token::StudyService, shared::config::AppSettings};


/// Application state shared through `actix_web::web::Data<AppState>`.
///
/// Example usage from a handler:
///
/// ```rust,ignore
/// use actix_web::web::Data;
///
/// async fn handler(state: Data<AppState>) {
///     // Borrow fields from shared state (no move).
///     let settings = &state.settings;
///     let study_service = &state.study_service;
///
///     // Clone only what you need to own locally.
///     let pacs = settings.pacs.clone();
///
///     // Use the service by reference.
///     let _ = study_service;
///     let _ = pacs;
/// }
/// ```
///
/// Avoid moving fields out of `state` (for example `let settings = state.settings;`),
/// because `Data<AppState>` is shared across concurrent requests.
pub struct AppState {
    pub settings: AppSettings,
    pub study_service: StudyService,
}

impl AppState {
    
    pub fn from_parts(
        settings: AppSettings,
        dependencies: AppDependencies,
    ) -> Self
    {
        let study_service = StudyService::new(dependencies.pacs_registry);

        Self {
            settings,
            study_service,
        }
    }
}