#[derive(Debug, Clone, Default)]
pub struct SeriesSearchCriteria {
    pub study_uid: Option<String>,
    pub patient_id: Option<String>,
    pub modality: Option<String>,
}


#[derive(Debug, Clone, Default)]
pub struct InstanceSearchCriteria {
    pub study_uid: Option<String>,
    pub series_uid: Option<String>,
    pub sop_instance_uid: Option<String>,
}


#[derive(Debug, Clone)]
pub struct Series {
    pub study_uid: String,
    pub series_uid: String,
    pub modality: Option<String>,
    pub description: Option<String>,
}


#[derive(Debug, Clone)]
pub struct Instance {
    pub study_uid: String,
    pub series_uid: String,
    pub sop_instance_uid: String,
    pub sop_class_uid: Option<String>,
    pub instance_number: Option<String>,
    pub relative_file_path: Option<String>,
    pub filesystem_id: Option<i32>,
}


#[derive(Debug, Clone)]
pub struct InstanceLocator {
    pub study_uid: String,
    pub series_uid: String,
    pub sop_instance_uid: String,
    pub relative_file_path: Option<String>,
    pub filesystem_id: Option<i32>,
}

impl From<&Instance> for InstanceLocator {
    fn from(value: &Instance) -> Self {
        Self {
            study_uid: value.study_uid.clone(),
            series_uid: value.series_uid.clone(),
            sop_instance_uid: value.sop_instance_uid.clone(),
            relative_file_path: value.relative_file_path.clone(),
            filesystem_id: value.filesystem_id,
        }
    }
}


#[derive(Debug, Clone, Default)]
pub struct ObjectAccessContext {
    pub transfer_syntax: Option<String>,
    pub content_type: Option<String>,
}


#[derive(Debug, Clone)]
pub struct DicomObject {
    pub bytes: Vec<u8>,
    pub content_type: String,
}


#[derive(Debug, Clone, Default)]
pub struct StudySearchCriteria {
    pub patient_id: Option<String>,
    pub accession_number: Option<String>,
}