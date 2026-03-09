/// A minimal DICOMWeb QIDO-RS implementation that does not cover all features of the standard.
/// 
/// The official DICOMWeb QIDO-RS specification can be found here:  
/// https://dicom.nema.org/medical/dicom/2018a/output/chtml/part18/sect_6.7.html#table_6.7.1-1
/// 
/// The standard defines a set of attributes that can be used to filter queries at the  
/// **Patient**, **Study**, **Series**, and **Instance** levels.
/// 
/// ### Study-Level DICOM Attribute Filters
/// | Keyword                | Tag       |
/// |------------------------|-----------|
/// | StudyDate              | 0008,0020 |
/// | StudyTime              | 0008,0030 |
/// | AccessionNumber        | 0008,0050 |
/// | ModalitiesInStudy      | 0008,0061 |
/// | ReferringPhysicianName | 0008,0090 |
/// | PatientName            | 0010,0010 |
/// | PatientID              | 0010,0020 |
/// | StudyInstanceUID       | 0020,000D |
/// | StudyID                | 0020,0010 |
/// 
/// This list is implemented in the [`QidoStudiesParams`] type.
/// 
/// ---
/// 
/// The specification also defines the attributes that must/can be returned in the response:  
/// https://dicom.nema.org/medical/dicom/2018a/output/chtml/part18/sect_6.7.html#table_6.7.1-2
/// 
/// ### Required Response Attributes
/// | Attribute Name                        | Tag       |
/// |------------------------------------- -|-----------|
/// | Specific Character Set                | 0008,0005 |
/// | Study Date                            | 0008,0020 |
/// | Study Time                            | 0008,0030 |
/// | Accession Number                      | 0008,0050 |
/// | Instance Availability                 | 0008,0056 |
/// | Modalities in Study                   | 0008,0061 |
/// | Referring Physician’s Name            | 0008,0090 |
/// | Timezone Offset From UTC              | 0008,0201 |
/// | Retrieve URL                          | 0008,1190 |
/// | Patient’s Name                        | 0010,0010 |
/// | Patient ID                            | 0010,0020 |
/// | Patient’s Birth Date                  | 0010,0030 |
/// | Patient’s Sex                         | 0010,0040 |
/// | Study Instance UID                    | 0020,000D |
/// | Study ID                              | 0020,0010 |
/// | Number of Study Related Series        | 0020,1206 |
/// | Number of Study Related Instances     | 0020,1208 |
/// 
/// In addition:
/// - Any other Study-level attributes passed as `{attributeID}` query keys (if supported)  
///   must be included as matching or return attributes.  
/// - Any attributes explicitly requested via the `"includefield"` query key must be returned.  
/// - If `"includefield=all"` is specified, all available Study-level attributes must be returned.  
/// 
/// This response attribute list is implemented in the [`qido::database`] module.
 

use std::collections::HashSet;

use actix_web::{HttpResponse, Responder, web::Data};
use serde::Deserialize;
use serde_querystring_actix::QueryString;
use sqlx::MySqlPool;

use crate::{constants::QIDO_STUDY_INCLUDEFIELD_DIC, error::HttpError, models, settings::Settings};

// ----------------------------------------------------------------------------------------------- //
// -- End Points --------------------------------------------------------------------------------------- //
pub async fn studies(params: QueryString<QidoStudiesParams>, pool: Data<MySqlPool>, settings: Data<Settings>)-> Result<HttpResponse, HttpError> {
    
    let params = params.into_inner();

    // // -- JWT Authorization ------------------------------------------------------------------------- // 
    // If JWT auth is enabled, validate the token.
    //     if let Some(token) = &params.token {
    //         validate_token(token, &settings).context("Autorization error")?;
    //     } else {
    //         return Err(HttpError::new_http_err(actix_web::error::ErrorUnauthorized("Token parameter was not found in the url")));
    //     }
    // }


    // -- Validate includefield dicom attributes
    let mut validated_include_fields = HashSet::new();
    
    if let Some(fields) = &params.includefield {
        validated_include_fields.reserve(fields.len());

        for field in fields {
            if let Some(attr) = QIDO_STUDY_INCLUDEFIELD_DIC.get(field.as_str()) {
                validated_include_fields.insert(*attr);
            } else {

                return Err(HttpError::new_http_err(actix_web::error::ErrorBadRequest(format!(
                    "Dicom attribute {field} is not able to be included in the response."
                ))));
            }
        }
    }
        


    let response = models::qido::get_studies(params, validated_include_fields, &pool, &settings).await?;
    Ok(HttpResponse::Ok().json(response))

}

pub async fn series() -> impl Responder {
    HttpResponse::NotImplemented()
}
pub async fn instances() -> impl Responder {
    HttpResponse::NotImplemented()
}

// ----------------------------------------------------------------------------------------------- //
// -- Search parameters --------------------------------------------------------------------------------------- //

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

