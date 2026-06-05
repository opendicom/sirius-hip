use actix_web::{
    error::ErrorInternalServerError,
    web::{Data, Json, Query},
};
use serde::{Deserialize, Serialize};

use crate::{
    bootstrap::state::AppState,
    features::study_token::{entities::Study, StudySearchCriteria},
};


#[derive(Debug, Deserialize)]
pub struct SearchStudiesQuery {
    pub patient_id: Option<String>,
    pub accession_number: Option<String>,
}


#[derive(Debug, Serialize)]
pub struct StudyResponse {
    pub study_uid: String,
    pub patient_id: String,
    pub patient_name: String,
    pub accession_number: Option<String>,
}

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


pub async fn search_studies(
    state: Data<AppState>,
    Query(query): Query<SearchStudiesQuery>,
)
-> Result<Json<Vec<StudyResponse>>, actix_web::Error>
{
    let criteria =
        StudySearchCriteria {

            patient_id:
                query.patient_id,

            accession_number:
                query.accession_number,
        };

    let studies =
        state
            .study_service
            .search(criteria)
            .await
            .map_err(ErrorInternalServerError)?;

    Ok(
        Json(
            studies
                .into_iter()
                .map(StudyResponse::from)
                .collect()
        )
    )
}