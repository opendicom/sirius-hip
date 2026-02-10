
use async_trait::async_trait;
use crate::src2::errors::PacsError;
use crate::src2::pacs::read_models::StudyTokenRow;
use crate::src2::pacs::read_models::QidoStudyRow;
use crate::settings::MetadataOverride;



// ===================================================
// StudyRepository Trait Definition
// ===================================================


#[async_trait]
pub trait StudyRepository: Send + Sync {

    /// Executes a single flat query to resolve a study token
    /// using precomputed hierarchy override flags.
    /// The query can include various filters at patient, study, series, and instance levels.
    /// Returns a vector of matching StudyTokenRow records.
    /// If no records match, returns an empty vector.
    /// The include_filesystem and include_wado flags
    /// control whether filesystem paths and WADO URLs
    /// are included in the returned rows.
    /// `include_wado` is reused to request extra OHIF metadata.
    async fn fetch_study_token_rows(
        &self,
        query: StudyTokenQuery<'_>,
        include_filesystem: bool,
        include_wado: bool,
    ) -> Result<Vec<StudyTokenRow>, PacsError>;

    /// Executes a study-level query suitable for QIDO-RS `/studies`.
    ///
    /// This must return one row per study (so `limit`/`offset` match QIDO semantics).
    async fn fetch_qido_studies_rows(
        &self,
        query: QidoStudiesQuery<'_>,
        include: QidoStudiesIncludeFields,
    ) -> Result<Vec<QidoStudyRow>, PacsError>;
}



// ===================================================
// StudyRepository Query Parameters Definition
// ===================================================


/// Query parameters for flexible study search (DICOM and business logic fields)
pub struct StudyTokenQuery<'a> {
    /// Optional list of metadata overrides.
    ///
    /// Each override describes:
    /// - what: `keyword`
    /// - where: `source` (must be `table.column`)
    /// - how: `dataset` (when true, `source` points to a DICOM dataset blob)
    ///
    /// NOTE: identifier validation is performed at startup (settings validation)
    /// and repository implementations may additionally enforce repository-specific rules.
    pub metadata_overrides: Option<&'a [MetadataOverride]>,
    
    // --------------------------------------------------
    // Global filters
    // --------------------------------------------------
    
    /// Institution name or code (for auth/tenant filtering, not DICOM standard)
    pub institution: Option<&'a str>,
    
    /// Maximum number of records to return (pagination/limit)
    pub max: Option<u64>,

    // --------------------------------------------------
    // Patient level (DICOM Patient Module)
    // --------------------------------------------------

    /// Patient ID (DICOM tag 0010,0020). Exact match.
    pub patient_id: Option<&'a str>,
    
    /// Patient full name (DICOM tag 0010,0010). Regex match for flexible search.
    pub patient_fullname: Option<&'a str>,

    // --------------------------------------------------
    // Study level (DICOM Study Module)
    // --------------------------------------------------
    
    /// Study Instance UID(s) (DICOM tag 0020,000D). One or more, separated by '\'.
    pub study_instance_uid: Option<&'a str>,
    
    /// Accession Number (DICOM tag 0008,0050). Exact match.
    pub accession_number: Option<&'a str>,
    
    /// Study ID (DICOM tag 0020,0010). Partial/LIKE match.
    pub study_id: Option<&'a str>,
    
    /// Study Date (DICOM tag 0008,0020). Four formats supported:
    /// - "YYYY-MM-DD" (exact)
    /// - "YYYY-MM-DD|" (>= date)
    /// - "|YYYY-MM-DD" (<= date)
    /// - "YYYY-MM-DD|YYYY-MM-DD" (between)
    pub study_date: Option<&'a str>,
    
    /// Modality in Study (DICOM tag 0008,0061). LIKE match, e.g. "CT", "MR".
    pub modality_in_study: Option<&'a str>,
    
    /// SOP Class OIDs in Study (DICOM tag 0008,0016). One or more, separated by '\'.
    pub cuids_in_study: Option<&'a str>,

    // --------------------------------------------------
    // Series level (DICOM Series Module)
    // --------------------------------------------------
    
    /// Series Instance UID(s) (DICOM tag 0020,000E). One or more, separated by '\'.
    pub series_instance_uid: Option<&'a str>,
    
    /// Series Number (DICOM tag 0020,0011). Exact match.
    pub series_number: Option<&'a str>,
    
    /// Series Description (DICOM tag 0008,103E). LIKE match.
    pub series_description: Option<&'a str>,
    
    /// Series Modality (DICOM tag 0008,0060). Exact match.
    pub modality: Option<&'a str>,
    
    /// Exclude modalities (DICOM tag 0008,0060). One or more, separated by '\'.
    pub modality_off: Option<&'a str>,
    
    /// SOP Class UID (DICOM tag 0008,0016). Exact match.
    pub sop_class: Option<&'a str>,
    
    /// Exclude SOP Class UID (DICOM tag 0008,0016). Exact match (negation).
    pub sop_class_off: Option<&'a str>,
}


// ===================================================
// QIDO /studies Query Parameters Definition
// ===================================================

pub struct QidoStudiesQuery<'a> {
    /// Optional list of metadata overrides.
    ///
    /// Each override describes:
    /// - what: `keyword`
    /// - where: `source` (must be `table.column`)
    /// - how: `dataset` (when true, `source` points to a DICOM dataset blob)
    ///
    pub metadata_overrides: Option<&'a [MetadataOverride]>,
    pub patient_id: Option<&'a str>,
    pub patient_name: Option<&'a str>,
    pub referring_physician_name: Option<&'a str>,
    pub accession_no: Option<&'a str>,
    pub modalities_in_study: Option<&'a str>,
    pub study_iuid: Option<&'a str>,
    pub study_id: Option<&'a str>,
    pub study_date: Option<&'a str>,
    pub study_time: Option<&'a str>,
    pub limit: u64,
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct QidoStudiesIncludeFields {
    pub includefield_00080062: bool,
    pub includefield_00081030: bool,
    pub includefield_00100021: bool,
}
