use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use serde_querystring_actix::QueryString;

use crate::errors::app_error::AppError;
use crate::application::use_cases::execute_qido_studies;
use crate::state2::AppState2;
use super::extract_token_from_headers;


// ========================================================================================= // 
// region: HTTP QUERY PARAMETERS - /qido/studies                                             //
// ========================================================================================= // 

/// Defines the expected query parameters for the /qido/studies endpoint, which returns study metadata
/// in DICOM JSON format based on query parameters and `includefield options. The parameters include 
/// standard DICOM query parameters for filtering studies, as well as options for including additional 
/// DICOM attributes in the response, fuzzy matching, pagination, and JWT authentication.
#[derive(Deserialize, Debug)]
pub struct QidoStudiesParams{
    #[serde(alias="StudyDate", alias="00080020")]
    pub study_date: Option<String>,

    #[serde(alias="StudyTime", alias="00080030")]
    pub study_time: Option<String>,

    #[serde(alias="AccessionNumber", alias="00080050")]
    pub accession_no: Option<String>,

    #[serde(alias="ModalitiesInStudy", alias="00080061")]
    pub modalities_in_study: Option<String>,

    #[serde(alias="ReferringPhysicianName", alias="00080090")]
    pub referring_physician_name: Option<String>,

    #[serde(alias="PatientName", alias="00100010")]
    pub patient_name: Option<String>,

    #[serde(alias="PatientID", alias="00100020")]
    pub patient_id: Option<String>,

    #[serde(alias="StudyInstanceUID", alias="0020000D")]
    pub study_iuid: Option<String>,

    #[serde(alias="StudyID", alias="00200010")]
    pub study_id: Option<String>,

    pub includefield: Option<Vec<String>>,

    pub fuzzymatching:Option<bool>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,

    pub token: Option<String>,
}

// endregion: HTTP QUERY PARAMETERS - /qido/studies                                          //
// ========================================================================================= //





// ========================================================================================= // 
// region: HTTP HANDLER - /qido/studies                                                      //
// ========================================================================================= // 

/// Handles HTTP requests for the QIDO studies endpoint (/qido/studies), which returns study metadata 
/// in DICOM JSON format based on query parameters and `includefield options.
pub async fn qido_studies_handler(
    req: HttpRequest,
    params: QueryString<QidoStudiesParams>,
    state: web::Data<AppState2>,
) -> Result<HttpResponse, AppError> {
    let mut params = params.into_inner();
    if let Some(token) = extract_token_from_headers(&req)? {
        params.token = Some(token);
    }

    let qido = execute_qido_studies(
        params, 
        state.pacs.study_repo.clone(),
        state.download_session_repo.clone(),
        state.settings.clone()
    ).await?;
    
    Ok(HttpResponse::Ok().json(qido))
}

// endregion: HTTP HANDLER - /qido/studies                                                   //
// ========================================================================================= //