use sqlx::FromRow;

/// Flat read-model optimized for StudyToken resolution.
/// This model is NOT part of the domain.
/// It exists purely for performance and query optimization.
#[derive(Debug, FromRow)]
pub struct StudyTokenRow {
    // Patient
    pub patient_name: Option<String>,
    pub patient_id: Option<String>,
    pub patient_sex: Option<String>,
    pub patient_birthdate: Option<String>,

    // Study
    pub study_date: Option<String>,
    pub study_time: Option<String>,
    pub study_description: Option<String>,
    pub accession_no: Option<String>,
    pub num_instances: Option<i32>,
    pub modalities: Option<String>,

    /// DICOM (0008,0080) InstitutionName at study level.
    ///
    /// If an override is configured with `keyword = "InstitutionName"` and `dataset = false`,
    /// repositories select it as a direct column value (non-dataset) into this column.
    pub institution_name: Option<String>,

    /// Study-level dataset blob used for default InstitutionName extraction.
    /// - dcm4chee 2.18.3: `study.study_attrs`
    /// - dcm4chee 4.4.0: `dicomattrs.attrs` joined via `study.dicomattrs_fk`
    pub study_attrs: Option<Vec<u8>>,

    pub study_instance_uid: String,
    pub series_instance_uid: String,
    pub sop_instance_uid: String,

    // Series
    pub series_no: Option<String>,
    pub series_description: Option<String>,
    pub modality: Option<String>,
    pub series_updated_time: Option<String>,

    // Instance
    pub instance_pk: Option<i32>,
    pub inst_no: Option<String>,
    pub sop_cuid: Option<String>,
    pub inst_attrs: Option<Vec<u8>>,

    // Additional dataset blobs used by metadata_overrides (dataset=true)
    pub ov_ds1: Option<Vec<u8>>,
    pub ov_ds2: Option<Vec<u8>>,
    pub ov_ds3: Option<Vec<u8>>,
    pub ov_ds4: Option<Vec<u8>>,

    // File reference (relative + filesystem id)
    pub relative_file_path: Option<String>,
    pub filesystem_fk: Option<i32>,

    /// Final resolution flag (precomputed in SQL)
    pub use_filesystem: bool,
}
