use serde::Deserialize;

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