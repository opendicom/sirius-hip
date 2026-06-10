use serde::Deserialize;

/// Study token search criteria for `fetch_study_token_rows`. 
/// This struct defines a comprehensive set of optional filters that can be applied at various levels of the DICOM hierarchy (patient, study, series).
/// The repository implementation is expected to interpret these criteria and construct an appropriate query against the underlying data store.
#[derive(Debug, Deserialize)]
pub struct StudyTokenQuery {
    // --------------------------------------------------
    // Global filters
    // --------------------------------------------------
    
    /// Institution name or code (for auth/tenant filtering, not DICOM standard)
    pub institution: Option<String>,
    
    /// Maximum number of records to return (pagination/limit)
    pub max: Option<u64>,

    // --------------------------------------------------
    // Patient level (DICOM Patient Module)
    // --------------------------------------------------

    /// Patient ID (DICOM tag 0010,0020). Exact match.
    pub patient_id: Option<String>,
    
    /// Patient full name (DICOM tag 0010,0010). Regex match for flexible search.
    pub patient_fullname: Option<String>,

    // --------------------------------------------------
    // Study level (DICOM Study Module)
    // --------------------------------------------------
    
    /// Study Instance UID(s) (DICOM tag 0020,000D). One or more, separated by '\'.
    pub study_instance_uid: Option<String>,
    
    /// Accession Number (DICOM tag 0008,0050). Exact match.
    pub accession_number: Option<String>,
    
    /// Study ID (DICOM tag 0020,0010). Partial/LIKE match.
    pub study_id: Option<String>,
    
    /// Study Date (DICOM tag 0008,0020). Four formats supported:
    /// - "YYYY-MM-DD" (exact)
    /// - "YYYY-MM-DD|" (>= date)
    /// - "|YYYY-MM-DD" (<= date)
    /// - "YYYY-MM-DD|YYYY-MM-DD" (between)
    pub study_date: Option<String>,
    
    /// Modality in Study (DICOM tag 0008,0061). LIKE match, e.g. "CT", "MR".
    pub modality_in_study: Option<String>,
    
    /// SOP Class OIDs in Study (DICOM tag 0008,0016). One or more, separated by '\'.
    pub cuids_in_study: Option<String>,

    // --------------------------------------------------
    // Series level (DICOM Series Module)
    // --------------------------------------------------
    
    /// Series Instance UID(s) (DICOM tag 0020,000E). One or more, separated by '\'.
    pub series_instance_uid: Option<String>,
    
    /// Series Number (DICOM tag 0020,0011). Exact match.
    pub series_number: Option<String>,
    
    /// Series Description (DICOM tag 0008,103E). LIKE match.
    pub series_description: Option<String>,
    
    /// Series Modality (DICOM tag 0008,0060). Exact match.
    pub modality: Option<String>,
    
    /// Exclude modalities (DICOM tag 0008,0060). One or more, separated by '\'.
    pub modality_off: Option<String>,
    
    /// SOP Class UID (DICOM tag 0008,0016). Exact match.
    pub sop_class: Option<String>,
    
    /// Exclude SOP Class UID (DICOM tag 0008,0016). Exact match (negation).
    pub sop_class_off: Option<String>,
}
