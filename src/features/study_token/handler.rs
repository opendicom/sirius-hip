use actix_web::{
    error::ErrorInternalServerError,
    web::{Data, Json, Query},
};
use serde::{Deserialize, Serialize};

use crate::{
    bootstrap::state::AppState,
    features::study_token::{criteria::StudyTokenQuery, entities::Study}, pacs::StudySearchCriteria,
};


// ----------------------------------------------------------------------------------------------------- //
// -- API Request/Response Models 
// ----------------------------------------------------------------------------------------------------- //

/// Represents the query parameters for searching studies in the API endpoint.
#[derive(Debug, Deserialize)]
pub struct SearchStudiesQuery {
    pub patient_id: Option<String>,
    pub accession_number: Option<String>,
}


/// Represents the response format for a study in the API endpoint.
#[derive(Debug, Serialize)]
pub struct StudyResponse {
    pub study_uid: String,
    pub patient_id: String,
    pub patient_name: String,
    pub accession_number: Option<String>,
}

/// Converts a `Study` entity from the service layer into a `StudyResponse` for the API response.
impl From<Study> for StudyResponse {
    fn from(value: Study) -> Self {
        Self {
            study_uid: value.study_uid,
            patient_id: value.patient_id,
            patient_name: value.patient_name,
            accession_number: value.accession_number,
        }
    }
}


// ----------------------------------------------------------------------------------------------------- //
// -- Handler 
// ----------------------------------------------------------------------------------------------------- //

/// Handler for the API endpoint to search for studies based on query parameters.
pub async fn search_studies(
    state: Data<AppState>,
    Query(query): Query<StudyTokenQuery>,
)
-> Result<Json<Vec<StudyResponse>>, actix_web::Error>
{
    // Map StudyTokenQuery to StudySearchCriteria for the service layer
    let criteria =
        StudySearchCriteria {

            patient_id: query.patient_id,
            accession_number: query.accession_number,
        };
    
    // Call the service layer to perform the search
    let studies =
        state
            .study_service
            .search(criteria)
            .await
            .map_err(ErrorInternalServerError)?;

    // Map the service layer results to the API response format
    Ok(
        Json(
            studies
                .into_iter()
                .map(StudyResponse::from)
                .collect()
        )
    )
}