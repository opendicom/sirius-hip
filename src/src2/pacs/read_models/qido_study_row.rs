use sqlx::FromRow;

/// Flat read-model for QIDO-RS /studies responses.
///
/// This is study-level (one row per study), optimized for QIDO.
#[derive(Debug, FromRow)]
pub struct QidoStudyRow {
    // Patient
    pub pat_name: Option<String>,
    pub pat_id: Option<String>,
    pub pat_sex: Option<String>,
    pub pat_birthdate: Option<String>,

    // Study
    pub study_date: String,
    pub study_time: String,
    pub accession_no: Option<String>,
    pub mods_in_study: Option<String>,
    pub study_iuid: String,
    pub study_id: Option<String>,
    pub study_desc: Option<String>,
    pub ref_physician: Option<String>,
    pub num_instances: i64,
    pub num_series: i64,

    // Optional includefield extras
    pub includefield_00080062: Option<String>,
    pub includefield_00081030: Option<String>,
    pub includefield_00100021: Option<String>,
}
