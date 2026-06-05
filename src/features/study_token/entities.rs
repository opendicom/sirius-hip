#[derive(Debug, Clone)]
pub struct Study {

    pub study_uid: String,

    pub patient_id: String,

    pub patient_name: String,

    pub accession_number: Option<String>,
}