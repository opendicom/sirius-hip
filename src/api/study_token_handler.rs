use actix_web::HttpRequest;
use actix_web::{web, HttpResponse};
use crate::application::use_cases::{execute_study_token, StudyTokenOutput};
use crate::errors::app_error::AppError;
use crate::state2::AppState2;
use super::extract_token_from_headers;

use serde::Deserialize;


// ========================================================================================= // 
// region: HTTP QUERY PARAMETERS - /studyToken                                               //
// ========================================================================================= // 

/// Defines the expected query parameters for the /studyToken endpoint, which generates a token 
/// that can be used to access study data in various formats. The parameters include options for
/// filtering studies, series, and instances, as well as operation-level parameters for authentication
/// and response formatting.
#[derive(Deserialize, Debug)]
pub struct StudyTokenParams {
    // Operation level
    pub token: Option<String>,              // For auth* interaction with external services       
    
    pub session: Option<String>,            // For auth* interaction with external services                    
    
    pub institution: Option<String>,        // For auth* interaction with external services
    
    #[serde(rename = "proxyURI")]
    pub proxy_uri: Option<String>,          // For interaction with external services
    #[serde(rename = "accessType")]
    pub access_type: String,                // Type of response expected
    pub max: Option<u64>,                   // Limit number of records to response

    // Patient level
    #[serde(rename = "PatientID")]
    pub patient_id: Option<String>,         // | Equal match | Patient id (0010,0020)   
    #[serde(rename = "patient")]
    pub patient_fullname: Option<String>,   // | REGEX match  | Patient name(0010,0010)  

    // Study level
    #[serde(rename = "StudyInstanceUID")]
    pub study_instance_uid: Option<String>, // | Equal match | List of Studies instance UID to search, \(back slash) separated 
    #[serde(rename = "AccessionNumber")]
    pub accession_number: Option<String>,   // | Equal match | Accession Number         
    #[serde(rename = "StudyID")]
    pub study_id: Option<String>,           // | Like match  | Study ID (0020,0010)     
    #[serde(rename = "StudyDate")]
    pub study_date: Option<String>,         // | Equal match | Study Date (0008,0020)   Four formats: AAA-MM-DD or AAA-MM-DD| or |AAA-MM-DD or AAA-MM-DD|AAA-MM-DD 
    #[serde(rename = "ModalityInStudy")]
    pub modality_in_study: Option<String>,  //  | Like match | Modality a study must contain
    #[serde(rename = "cuidsInStudy")]
    pub cuids_in_study: Option<String>,     // | Equal match | SOP Class OID in Study  

    // Series level
    #[serde(rename = "SeriesInstanceUID")]
    pub series_instance_uid: Option<String>,    // | Equal match | List of series iuid \(back slash) separated 
    #[serde(rename = "SeriesNumber")]
    pub series_number: Option<String>,          // | Equal match | 
    #[serde(rename = "SeriesDescription")]
    pub series_description: Option<String>,     // | Like match | 
    #[serde(rename = "Modality")]
    pub modality: Option<String>,               // | Equal match |
    #[serde(rename = "ModalityOff")]
    pub modality_off: Option<String>,           // | Equal match | List \(back slash) separated 
    #[serde(rename = "SOPClass")]
    pub sop_class: Option<String>,              // | Equal match |
    #[serde(rename = "SOPClassOff")]
    pub sop_class_off: Option<String>,          // | Equal match | (exclude Instance with that soap class)
}

// endregion: HTTP QUERY PARAMETERS - /studyToken                                            //
// ========================================================================================= // 




// ========================================================================================= // 
// region: HTTP HANDLER - /studyToken                                                        //
// ========================================================================================= // 

/// Handles HTTP requests for the study token endpoint, which generates a token 
/// that can be used to access study data in various formats 
/// - Weasis manifest
/// - OHIF manifest
/// - DICOM ZIP
pub async fn study_token_handler(
    state: web::Data<AppState2>,
    req: HttpRequest,
    params: web::Query<StudyTokenParams>,
) -> Result<HttpResponse, AppError> {

    // --------------------------------------------------------------
    // 1. GET SERVER BASE URL
    // --------------------------------------------------------------
    let conn = req.connection_info();
    let server_base_url = format!("{}://{}",
        conn.scheme(),
        conn.host(),
    );


    // --------------------------------------------------------------
    // 2. EXTRACT TOKEN (headers first, then query param)
    // --------------------------------------------------------------
    let mut params = params.into_inner();
    if let Some(token) = extract_token_from_headers(&req)? {
        params.token = Some(token);
    }
    
    
    // --------------------------------------------------------------
    // 3. PROCESS REQUEST
    // --------------------------------------------------------------
    let output = execute_study_token(
        state.pacs.study_repo.clone(),
        state.download_session_repo.clone(),
        state.tmp_pool.clone(),
        state.settings.clone(),
        params,
        &server_base_url,
    )
    .await?;


    // --------------------------------------------------------------
    // 4. BUILD RESPONSE
    // --------------------------------------------------------------
    Ok(match output {
        StudyTokenOutput::Json(value) => HttpResponse::Ok().json(value),
        StudyTokenOutput::Xml(xml) => HttpResponse::Ok()
            .content_type("application/xml; charset=utf-8")
            .body(xml),
        StudyTokenOutput::Zip { filename, mut zip } => {
            zip.set_http_client(state.http_client.clone());
            HttpResponse::Ok()
            .content_type("application/zip")
            .append_header((
                "Content-Disposition",
                format!("attachment; filename=\"{}\"", filename),
            ))
            .streaming(zip.build())
        }
    })
}

// endregion: HTTP HANDLER - /studyToken                                                     //
// ========================================================================================= // 