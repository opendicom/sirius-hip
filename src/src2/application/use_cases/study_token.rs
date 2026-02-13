use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde_json::json;
use sqlx::MySqlPool;
use async_trait::async_trait;
use anyhow::Context;

use std::sync::{Arc as StdArc, Mutex};
use dicom_encoding::TransferSyntaxIndex;
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use dicom_dictionary_std::tags as DicomTag;

use crate::api::study_token::params::StudyTokenParams;
use crate::auth::{self, AuthClaims};
use crate::settings::{JwtAuthMethod, Settings};
use crate::src2::errors::app_error::AppError;
use crate::src2::pacs::repositories::study_repository::{StudyRepository, StudyTokenQuery};
use crate::src2::application::models::{
    download_session::DownloadSession,
    download_session_file::DownloadSessionFile,
};

use crate::src2::application::repositories::download_session_repository::DownloadSessionRepository;



/// Access type for /studyToken request.
/// Determines how the client will consume the study
#[derive(Debug, Clone, Copy)]
pub enum AccessType {
    Zip,
    Weasis,
    Ohif,
    Cornerstone,
}

impl AccessType {
    fn from_param(access_type: &str) -> Option<Self> {
        match access_type {
            "dicom.zip" => Some(AccessType::Zip),
            "weasis.xml" => Some(AccessType::Weasis),
            "ohif" => Some(AccessType::Ohif),
            "cornerstone.json" => Some(AccessType::Cornerstone),
            _ => None,
        }
    }

}


// ========================================================================================= //
// region: === StudyTokenPlan - Output returned by /studyToken (varies by accessType)
// ========================================================================================= //

/// Output of /studyToken request rendering.
/// Varies by `accessType` parameter.
pub enum StudyTokenOutput {
    Json(serde_json::Value),
    Xml(String),
    Zip {
        filename: String,
        zip: crate::models::dicomzip::DicomStreamZip,
    },
}

/// Intermediate **request plan** for building a `/studyToken` response.
///
/// In this module we split the workflow in two phases:
///
/// 1) **Orchestration (use-case)**: validate auth, query PACS, optionally create a OneTime
///    download session, and derive all URLs/sources required by the response.
/// 2) **Presentation (presenters)**: turn the prepared data into the final output shape
///    (`JSON`, `XML`, or `ZIP`) depending on `accessType`.
///
/// We call the output of phase (1) a *Plan* because it is a complete, self-contained
/// **snapshot of everything needed to render** the response, without doing any more domain
/// decisions. Presenters should be able to operate only on this struct.
///
/// Notes:
/// - This is **per-request**, in-memory only (it is *not* persisted).
/// - It intentionally contains both raw data (`rows`) and derived data (`retrieve_urls`,
///   `zip_sources`) so rendering does not need to touch DB/auth.
/// - In `JwtAuthMethod::OneTime`, the plan carries a `session_id` and uses local `/files/{session}/{index}`
///   URLs so downloads can be enforced and tracked.
#[derive(Debug)]
struct StudyTokenPlan {
    params: StudyTokenParams,
    access_type: AccessType,
    rows: Vec<crate::src2::pacs::read_models::StudyTokenRow>,
    total_files: u32,
    session_id: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    base_url: String,
    // Retrieval URLs intended for viewer-style clients.
    // - In OneTime mode: local /files/{session}/{index} URLs (enforced)
    // - Otherwise: local /files/{session}/{index} URLs (proxy)
    retrieve_urls: Vec<String>,
    // Sources suitable for ZIP building (file:// or http(s) WADO).
    // In OneTime mode this matches what was persisted for the session.
    zip_sources: Vec<(String, String)>,
}


// endregion: === StudyTokenPlan =========================================================== //
// ========================================================================================= // 



// ========================================================================================= //
// region: === StudyTokenPresenter ========================================================= //
// ========================================================================================= // 


/// Presenter trait for /studyToken response rendering.
#[async_trait(?Send)]
trait StudyTokenPresenter {
    async fn render(&self, plan: StudyTokenPlan) -> Result<StudyTokenOutput, AppError>;
}

/// OHIF presenter implementation
/// Uses detailed JSON manifest format
/// Requires extensive DICOM metadata extraction
/// List of required metadata for OHIF viewer:
/// https://docs.ohif.org/faq/technical#what-are-the-list-of-required-metadata-for-the-ohif-viewer-to-work
struct OhifPresenter {
    pool: MySqlPool,
    settings: Arc<Settings>,
}
struct CornerstonePresenter;
struct WeasisPresenter;
struct ZipPresenter;


// -------------------------------------------------------------------------------- //
// OHIF - StudyTokenPresenter implementation
// -------------------------------------------------------------------------------- //
#[async_trait(?Send)]
impl StudyTokenPresenter for OhifPresenter {
    async fn render(&self, plan: StudyTokenPlan) -> Result<StudyTokenOutput, AppError> {
        use crate::models::ohif::{Instance, InstanceMetadata, Serie, Studies, Study};
        use dicom_core::Tag;
        use dicom_core::DataElement;
        use crate::settings::MetadataOverride;
        use crate::src2::pacs::infrastructure::mysql_sql_helpers::dataset_sources;

        // Precompute URL constant pieces for this request.
        let wado_fallback = if plan.base_url.is_empty() {
            "/wado".to_string()
        } else {
            format!("{}/wado", plan.base_url)
        };

        let wado_base = plan
            .params
            .proxy_uri
            .as_deref()
            .or(self.settings.dicomarchive.manifest_base_url.as_deref())
            .unwrap_or(wado_fallback.as_str());

        let session_q = plan
            .params
            .session
            .as_ref()
            .map_or(String::new(), |val| format!("&session={val}"));

        let custodian_q = self
            .settings
            .dicomarchive
            .custodianoid
            .as_ref()
            .map_or(String::new(), |x| format!("&custodianOID={x}"));

        let arc_q = self
            .settings
            .dicomarchive
            .pacsoid
            .as_ref()
            .map_or(String::new(), |x| format!("&arcId={x}"));

        let token_q = plan
            .params
            .token
            .as_ref()
            .map_or(String::new(), |val| format!("&token={val}"));
        let transfer_syntax = self.settings.dicomarchive.transfer_syntax.clone();

        let ts = TransferSyntaxRegistry
            .get("1.2.840.10008.1.2.1")
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Missing Explicit Little Endian transfer syntax")))?;

        let overrides: Option<&[MetadataOverride]> = self.settings.dicomarchive.metadata_overrides.as_deref();
        // `dataset_sources()` returns the distinct set of `source` values where `dataset=true`.
        // The list is sorted to make slot assignment deterministic across runs.
        //
        // IMPORTANT: the SQL repositories select these sources into a fixed number of columns
        // in the row read-model: `ov_ds1..ov_ds4`.
        // The mapping is positional:
        // - ov_ds1 -> ds_sources[0]
        // - ov_ds2 -> ds_sources[1]
        // - ov_ds3 -> ds_sources[2]
        // - ov_ds4 -> ds_sources[3]
        //
        // If you add a new dataset source, it must be within the supported limit (currently 4).
        let ds_sources = dataset_sources(overrides);

        // Build by single pass over ordered rows: study -> series -> instances.
        let mut out: Vec<Study> = Vec::new();
        let mut current_study_iuid: Option<&str> = None;
        let mut current_series_iuid: Option<&str> = None;
        let mut current_study: Option<Study> = None;
        let mut current_series: Option<Serie> = None;

        // -------------------------------------------------------------- //

        // Iterate over rows and build hierarchy.
        for (row_idx, row) in plan.rows.iter().enumerate() {
            let study_changed = current_study_iuid != Some(row.study_instance_uid.as_str());
            let series_changed = study_changed || current_series_iuid != Some(row.series_instance_uid.as_str());

            // Handle study change.
            if study_changed {
                if let Some(serie) = current_series.take() {
                    if let Some(study) = current_study.as_mut() {
                        study.series.push(serie);
                    }
                }
                if let Some(study) = current_study.take() {
                    out.push(study);
                }

                let patient_age = row
                    .patient_birthdate
                    .clone()
                    .map(crate::database::helpers::calculate_age)
                    .transpose()
                    .map_err(|e| AppError::Internal(e.into()))?
                    .map(|age| age.to_string());

                // Study-level InstitutionName (0008,0080):
                // 1) If a direct column value (non-dataset) override is configured (dataset=false),
                //    repositories select it into `row.institution_name`.
                // 2) Otherwise, try to decode it from the study-level dataset blob `row.study_attrs`.
                let institution_name: Option<String> = row
                    .institution_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        let bytes = row.study_attrs.as_deref()?;
                        // Best-effort decode: missing/invalid blobs should not break OHIF rendering.
                        let dcm = InMemDicomObject::read_dataset_with_ts(bytes, ts).ok()?;
                        let el = dcm.element_opt(Tag(0x0008, 0x0080)).ok()??;
                        el.to_str().ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
                    });

                current_study = Some(Study {
                    study_pk: 0,
                    study_iuid: row.study_instance_uid.clone(),
                    study_date: row
                        .study_date
                        .as_deref()
                        .unwrap_or("")
                        .replace('-', ""),
                    study_time: row.study_time.clone().unwrap_or_default(),
                    study_description: row.study_description.clone(),
                    patient_name: row.patient_name.clone().unwrap_or_default(),
                    patient_id: row.patient_id.clone().unwrap_or_default(),
                    accession_no: row.accession_no.clone(),
                    patient_age,
                    patient_sex: row.patient_sex.clone(),
                    num_instances: row.num_instances.unwrap_or(0),
                    modalities: row.modalities.clone().unwrap_or_default(),
                    institution_name,
                    series: Vec::new(),
                });

                current_study_iuid = Some(row.study_instance_uid.as_str());
                current_series_iuid = None;
            }

            // Handle series change.
            if series_changed {
                if let Some(serie) = current_series.take() {
                    if let Some(study) = current_study.as_mut() {
                        study.series.push(serie);
                    }
                }

                let series_no = row
                    .series_no
                    .as_deref()
                    .unwrap_or("0")
                    .parse::<i32>()
                    .context("Failed to parse series_no to integer")
                    .map_err(|e| AppError::Internal(e.into()))?;

                current_series = Some(Serie {
                    serie_pk: 0,
                    series_iuid: row.series_instance_uid.clone(),
                    series_no,
                    modality: row.modality.clone().unwrap_or_default(),
                    series_description: row.series_description.clone(),
                    instances: Vec::new(),
                });

                current_series_iuid = Some(row.series_instance_uid.as_str());
            }

            // Instance URL: use the already-prepared per-row retrieval URL from the plan.
            // This avoids building a HashMap<String,String> just for lookup.
            let url = if let Some(retrieve_url) = plan.retrieve_urls.get(row_idx) {
                format!("dicomweb:{retrieve_url}")
            } else {
                // Defensive fallback (should not happen): build a WADO URL.
                let mut url = String::with_capacity(256);
                url.push_str("dicomweb:");
                url.push_str(wado_base);
                url.push_str("?requestType=WADO&studyUID=");
                url.push_str(&row.study_instance_uid);
                url.push_str("&seriesUID=");
                url.push_str(&row.series_instance_uid);
                url.push_str("&objectUID=");
                url.push_str(&row.sop_instance_uid);
                url.push_str("&transferSyntax=");
                url.push_str(&transfer_syntax);
                url.push_str("&contentType=application/dicom");
                url.push_str(&session_q);
                url.push_str(&custodian_q);
                url.push_str(&arc_q);
                url.push_str(&token_q);
                url
            };

            let instance_pk = row
                .instance_pk
                .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Missing instance_pk for OHIF")))?;

            let inst_no = row
                .inst_no
                .as_deref()
                .unwrap_or("0")
                .parse::<i32>()
                .context("Failed to parse Instance number to integer")
                .map_err(|e| AppError::Internal(e.into()))?;

            let sop_cuid = row.sop_cuid.clone().unwrap_or_default();
            
            let inst_attrs = row
                .inst_attrs
                .as_deref()
                .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Missing inst_attrs for OHIF")))?;

            // Decode instance.inst_attrs used for Ohif-required metadata, applying overrides if needed.
            let inst_attrs_dcm = InMemDicomObject::read_dataset_with_ts(inst_attrs, ts)
                .context("Failed to read inst_attrs value from database")
                .map_err(|e| AppError::Internal(e.into()))?;

            // Per-row override datasets (selected by SQL into ov_ds1..ov_ds4) follow ds_sources order.
            // We build a map of `source` -> bytes for easy lookup when applying overrides below.
            //
            // Why a map?
            // - `metadata_overrides` points to datasets using a string key (`source = "table.column"`).
            // - The row model stores the actual bytes in positional fields (ov_ds1..ov_ds4).
            // - This map bridges both worlds so later code can do: `bytes = map[source]`.
            //
            // Why references (`&str`, `&[u8]`) instead of owned values?
            // - Avoid cloning `source` strings and dataset bytes on every row.
            // - The references are valid for the duration of this loop iteration because:
            //   - keys reference `ds_sources` items
            //   - values reference `row.ov_dsN` buffers
            let mut ds_bytes_by_source: HashMap<&str, &[u8]> = HashMap::new();
            if let Some(src) = ds_sources.get(0) {
                if let Some(b) = row.ov_ds1.as_deref() {
                    ds_bytes_by_source.insert(src.as_str(), b);
                }
            }
            if let Some(src) = ds_sources.get(1) {
                if let Some(b) = row.ov_ds2.as_deref() {
                    ds_bytes_by_source.insert(src.as_str(), b);
                }
            }
            if let Some(src) = ds_sources.get(2) {
                if let Some(b) = row.ov_ds3.as_deref() {
                    ds_bytes_by_source.insert(src.as_str(), b);
                }
            }
            if let Some(src) = ds_sources.get(3) {
                if let Some(b) = row.ov_ds4.as_deref() {
                    ds_bytes_by_source.insert(src.as_str(), b);
                }
            }

            // Decode any override datasets needed for OHIF-required tags.
            //
            // We cache decoded datasets by `source` so if multiple keywords point to the same
            // dataset blob we only parse it once per row.
            let mut decoded_by_source: HashMap<String, InMemDicomObject> = HashMap::new();

            // Helper to get the override source for a given DICOM keyword, if it exists and is a dataset override.
            let dataset_source_for_keyword = |keyword: &str| -> Option<&str> {
                let list = overrides?;
                let ov = list.iter().find(|ov| ov.keyword == keyword)?;
                if ov.dataset {
                    Some(ov.source.as_str())
                } else {
                    None
                }
            };

            // Only decode datasets for the keywords OHIF needs in this code path.
            // (We can extend this list later if other tags become override-driven.)
            for keyword in [
                "Columns",
                "Rows",
                "PhotometricInterpretation",
                "BitsAllocated",
                "PlanarConfiguration",
            ] {
                if let Some(source) = dataset_source_for_keyword(keyword) {
                    if decoded_by_source.contains_key(source) {
                        continue;
                    }

                    // If the configured override dataset is NULL for this row, we skip decoding.
                    // Later, reads will fall back to `inst_attrs_dcm`.
                    if let Some(bytes) = ds_bytes_by_source.get(source).copied() {
                        let dcm = InMemDicomObject::read_dataset_with_ts(bytes, ts)
                            .context(format!("Failed to read override dataset '{source}' for keyword '{keyword}'"))
                            .map_err(|e| AppError::Internal(e.into()))?;
                        decoded_by_source.insert(source.to_string(), dcm);
                    }
                }
            }

            // Helper to get the appropriate DICOM attributes for a given keyword, applying dataset override if configured.
            //
            // Decision logic:
            // - If `metadata_overrides` says `keyword` is dataset-based, and we decoded that source,
            //   use the override dataset.
            // - Otherwise, fall back to the normal per-instance dataset (`inst_attrs`).
            let dicomattrs_for_keyword = |keyword: &str| -> &InMemDicomObject {
                if let Some(source) = dataset_source_for_keyword(keyword) {
                    if let Some(dcm) = decoded_by_source.get(source) {
                        return dcm;
                    }
                }
                &inst_attrs_dcm
            };

            fn u16_from_element(
                el: &DataElement<InMemDicomObject, Vec<u8>>,
                tag: Tag,
            ) -> Result<u16, AppError> {
                el.to_int()
                    .context(format!("Failed to parse DicomTag {tag} value to u16"))
                    .map_err(|e| AppError::Internal(e.into()))
            }

            fn string_from_element(
                el: &DataElement<InMemDicomObject, Vec<u8>>,
                tag: Tag,
            ) -> Result<String, AppError> {
                el.to_str()
                    .context(format!("Failed to parse DicomTag {tag} value to String"))
                    .map_err(|e| AppError::Internal(e.into()))
                    .map(|s| s.to_string())
            }

            // Fast-path: pull from inst_attrs (db_dicomattrs) without async fallback.
            // If something is missing, we will fall back to reading from the DICOM file on disk.
            // This avoids unnecessary file I/O in the common case.
            let mut columns: Option<u16> = dicomattrs_for_keyword("Columns")
                .element_opt(DicomTag::COLUMNS)
                .map_err(|e| AppError::Internal(e.into()))?
                .map(|el| u16_from_element(el, DicomTag::COLUMNS))
                .transpose()?;

            let mut rows_tag: Option<u16> = dicomattrs_for_keyword("Rows")
                .element_opt(DicomTag::ROWS)
                .map_err(|e| AppError::Internal(e.into()))?
                .map(|el| u16_from_element(el, DicomTag::ROWS))
                .transpose()?;

            let mut photometric_interpretation: Option<String> = dicomattrs_for_keyword("PhotometricInterpretation")
                .element_opt(DicomTag::PHOTOMETRIC_INTERPRETATION)
                .map_err(|e| AppError::Internal(e.into()))?
                .map(|el| string_from_element(el, DicomTag::PHOTOMETRIC_INTERPRETATION))
                .transpose()?;

            let mut bits_allocated: Option<u16> = dicomattrs_for_keyword("BitsAllocated")
                .element_opt(DicomTag::BITS_ALLOCATED)
                .map_err(|e| AppError::Internal(e.into()))?
                .map(|el| u16_from_element(el, DicomTag::BITS_ALLOCATED))
                .transpose()?;

            let mut planar_configuration: Option<u16> = if let Some(pi) = photometric_interpretation.as_deref() {
                if pi != "MONOCHROME1" && pi != "MONOCHROME2" {
                    dicomattrs_for_keyword("PlanarConfiguration")
                        .element_opt(DicomTag::PLANAR_CONFIGURATION)
                        .map_err(|e| AppError::Internal(e.into()))?
                        .map(|el| u16_from_element(el, DicomTag::PLANAR_CONFIGURATION))
                        .transpose()?
                } else {
                    None
                }
            } else {
                None
            };

            // Slow-path: if something critical is missing from inst_attrs, fall back to the a helper
            // which can read from the DICOM file on disk.
            if columns.is_none()
                || rows_tag.is_none()
                || photometric_interpretation.is_none()
                || bits_allocated.is_none()
                || (planar_configuration.is_none()
                    && photometric_interpretation
                        .as_deref()
                        .map(|pi| pi != "MONOCHROME1" && pi != "MONOCHROME2")
                        .unwrap_or(false))
            {
                // Get absolute filepath.
                let abs_filepath = match (row.filesystem_fk, row.relative_file_path.as_deref()) {
                    (Some(fs_id), Some(rel)) => self
                        .settings
                        .dicomarchive
                        .get_fs_path_by_id(fs_id)
                        .map(|base| {
                            format!(
                                "{}/{}",
                                base.trim_end_matches('/'),
                                rel.trim_start_matches('/')
                            )
                        })
                        .unwrap_or_default(),
                    _ => String::new(),
                };

                // Prepare for file reading.
                let filepath: StdArc<Mutex<String>> = StdArc::new(Mutex::new(abs_filepath));
                let mut file_dicomattrs = InMemDicomObject::new_empty();

                if columns.is_none() {
                    let tag = DicomTag::COLUMNS;
                    columns = crate::database::helpers::get_dicom_element(
                        tag,
                        filepath.clone(),
                        dicomattrs_for_keyword("Columns"),
                        &mut file_dicomattrs,
                        instance_pk,
                        &self.pool,
                        &self.settings,
                    )
                    .await
                    .map_err(|e| AppError::Internal(e.into()))?
                    .map(|el| u16_from_element(el, tag))
                    .transpose()?;
                }

                if rows_tag.is_none() {
                    let tag = DicomTag::ROWS;
                    rows_tag = crate::database::helpers::get_dicom_element(
                        tag,
                        filepath.clone(),
                        dicomattrs_for_keyword("Rows"),
                        &mut file_dicomattrs,
                        instance_pk,
                        &self.pool,
                        &self.settings,
                    )
                    .await
                    .map_err(|e| AppError::Internal(e.into()))?
                    .map(|el| u16_from_element(el, tag))
                    .transpose()?;
                }

                if photometric_interpretation.is_none() {
                    let tag = DicomTag::PHOTOMETRIC_INTERPRETATION;
                    photometric_interpretation = crate::database::helpers::get_dicom_element(
                        tag,
                        filepath.clone(),
                        dicomattrs_for_keyword("PhotometricInterpretation"),
                        &mut file_dicomattrs,
                        instance_pk,
                        &self.pool,
                        &self.settings,
                    )
                    .await
                    .map_err(|e| AppError::Internal(e.into()))?
                    .map(|el| string_from_element(el, tag))
                    .transpose()?;
                }

                if bits_allocated.is_none() {
                    let tag = DicomTag::BITS_ALLOCATED;
                    bits_allocated = crate::database::helpers::get_dicom_element(
                        tag,
                        filepath.clone(),
                        dicomattrs_for_keyword("BitsAllocated"),
                        &mut file_dicomattrs,
                        instance_pk,
                        &self.pool,
                        &self.settings,
                    )
                    .await
                    .map_err(|e| AppError::Internal(e.into()))?
                    .map(|el| u16_from_element(el, tag))
                    .transpose()?;
                }

                let needs_planar = photometric_interpretation
                    .as_deref()
                    .map(|pi| pi != "MONOCHROME1" && pi != "MONOCHROME2")
                    .unwrap_or(false);

                if needs_planar && planar_configuration.is_none() {
                    let tag = DicomTag::PLANAR_CONFIGURATION;
                    planar_configuration = crate::database::helpers::get_dicom_element(
                        tag,
                        filepath.clone(),
                        dicomattrs_for_keyword("PlanarConfiguration"),
                        &mut file_dicomattrs,
                        instance_pk,
                        &self.pool,
                        &self.settings,
                    )
                    .await
                    .map_err(|e| AppError::Internal(e.into()))?
                    .map(|el| u16_from_element(el, tag))
                    .transpose()?;
                }
            }

            let series_date = row
                .series_updated_time
                .as_deref()
                .unwrap_or("")
                .replace('-', "");
            let series_modality = current_series
                .as_ref()
                .map(|s| s.modality.clone())
                .unwrap_or_default();

            if let Some(serie) = current_series.as_mut() {
                serie.instances.push(Instance {
                    instance_pk,
                    metadata: InstanceMetadata {
                        instance_no: inst_no,
                        instance_sop_cuid: sop_cuid,
                        series_modality,
                        instance_sop_iuid: row.sop_instance_uid.clone(),
                        series_iuid: row.series_instance_uid.clone(),
                        study_iuid: row.study_instance_uid.clone(),
                        series_date,
                        columns,
                        rows: rows_tag,
                        photometric_interpretation,
                        bits_allocated,
                        pixel_representation: None,
                        samples_per_pixel: None,
                        pixel_spacing: None,
                        bits_stored: None,
                        high_bit: None,
                        image_orientation_patient: None,
                        image_position_patient: None,
                        frame_of_reference_uid: None,
                        image_type: None,
                        window_center: None,
                        window_width: None,
                        planar_configuration,
                        rescale_intercept: None,
                        rescale_slope: None,
                        number_of_frames: None,
                        frame_time: None,
                    },
                    url,
                });
            }
        }

        if let Some(serie) = current_series.take() {
            if let Some(study) = current_study.as_mut() {
                study.series.push(serie);
            }
        }
        if let Some(study) = current_study.take() {
            out.push(study);
        }

        let manifest = Studies {
            studies: Box::new(out),
        };

        let value = serde_json::to_value(manifest).map_err(|e| AppError::Internal(e.into()))?;
        Ok(StudyTokenOutput::Json(value))
    }
}


// -------------------------------------------------------------------------------- //
// CORNERSTONE -  StudyTokenPresenter implementation
// -------------------------------------------------------------------------------- //
#[async_trait(?Send)]
impl StudyTokenPresenter for CornerstonePresenter {
    async fn render(&self, plan: StudyTokenPlan) -> Result<StudyTokenOutput, AppError> {
        Ok(StudyTokenOutput::Json(json!({
            "accessType": "cornerstone.json",
            "sessionId": plan.session_id,
            "expiresAt": plan.expires_at.map(|d| d.to_rfc3339()),
            "totalFiles": plan.total_files,
            "urls": plan.retrieve_urls,
        })))
    }
}


// -------------------------------------------------------------------------------- //
// WEASIS - StudyTokenPresenter implementation
// -------------------------------------------------------------------------------- //
#[async_trait(?Send)]
impl StudyTokenPresenter for WeasisPresenter {
    async fn render(&self, plan: StudyTokenPlan) -> Result<StudyTokenOutput, AppError> {
        // Minimal XML payload (easy to evolve).
        // If you have a strict Weasis schema, replace this with a dedicated builder.
        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<weasisManifest>");
        if let Some(session_id) = &plan.session_id {
            xml.push_str("<sessionId>");
            xml.push_str(session_id);
            xml.push_str("</sessionId>");
        }
        xml.push_str("<totalFiles>");
        xml.push_str(&plan.total_files.to_string());
        xml.push_str("</totalFiles>");
        xml.push_str("<files>");
        fn escape_xml_attr(value: &str) -> String {
            value
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&apos;")
        }

        for url in &plan.retrieve_urls {
            xml.push_str("<file url=\"");
            xml.push_str(&escape_xml_attr(url));
            xml.push_str("\" />");
        }
        xml.push_str("</files>");
        xml.push_str("</weasisManifest>");

        Ok(StudyTokenOutput::Xml(xml))
    }
}


// -------------------------------------------------------------------------------- //
// DICOM ZIP - StudyTokenPresenter implementation
// -------------------------------------------------------------------------------- //
#[async_trait(?Send)]
impl StudyTokenPresenter for ZipPresenter {
    async fn render(&self, plan: StudyTokenPlan) -> Result<StudyTokenOutput, AppError> {
        let mut zip = crate::models::dicomzip::DicomStreamZip::new();

        // Name files deterministically. Keep it simple: 0001.dcm, 0002.dcm, ...
        for (idx, (_instance_uid, source_url)) in plan.zip_sources.iter().enumerate() {
            let name = format!("{:04}.dcm", idx + 1);
            zip.add_entry(&name, source_url);
        }

        Ok(StudyTokenOutput::Zip {
            filename: "dicom.zip".to_string(),
            zip,
        })
    }
}


// endregion: === StudyTokenPresenter ====================================================== //
// ========================================================================================= // 



// ========================================================================================= //
// region: === AccessType enum definition ================================================== //
// ========================================================================================= // 



// ========================================================================================= //
// region: === HELPERS FUNCTIONS =========================================================== //


/// Build direct WADO-URI for given instance row
/// Uses PACS WADO-URI base from settings
fn build_wado_url(settings: &Settings, row: &crate::src2::pacs::read_models::StudyTokenRow) -> String {
    // Use direct PACS WADO-URI (not the local /wado proxy) so redirects work even
    // when the client is not allowed to reach this server's /wado endpoint.
    // UIDs and transfer syntax are dot-separated and safe without extra encoding.
    format!(
        "{}?requestType=WADO&studyUID={}&seriesUID={}&objectUID={}&contentType=application/dicom&transferSyntax={}",
        settings.dicomarchive.wadouri,
        row.study_instance_uid,
        row.series_instance_uid,
        row.sop_instance_uid,
        settings.dicomarchive.transfer_syntax,
    )
}

/// Build absolute filesystem path for given instance row
/// Returns None if filesystem_fk or relative_file_path are missing
fn build_absolute_filesystem_path(settings: &Settings, row: &crate::src2::pacs::read_models::StudyTokenRow) -> Option<String> {
    let fs_id = row.filesystem_fk?;
    let rel = row.relative_file_path.as_deref()?;
    let base = settings.dicomarchive.get_fs_path_by_id(fs_id)?;
    Some(format!(
        "{}/{}",
        base.trim_end_matches('/'),
        rel.trim_start_matches('/')
    ))
}

// endregion: === HELPERS FUNCTIONS ======================================================== //
// ========================================================================================= // 



// ========================================================================================= // 
// region: === Main use-case function ====================================================== //
// ========================================================================================= // 


/// Main use-case function for /studyToken
pub async fn execute_study_token(
    study_repo: Arc<dyn StudyRepository>,
    session_repo: Arc<dyn DownloadSessionRepository>,
    pacs_pool: MySqlPool,
    settings: Arc<Settings>,
    params: StudyTokenParams,
    server_base_url: &str,
) -> Result<StudyTokenOutput, AppError> {


        let access_type = AccessType::from_param(params.access_type.as_str())
            .ok_or_else(|| AppError::bad_request("missing required parameter"))?;


        // --------------------------------------------------------------
        // 1. VALIDATE JWT TOKEN
        // --------------------------------------------------------------
        let jwt_claims: Option<AuthClaims> = match settings.jwt_auth {
            JwtAuthMethod::None => None,
            JwtAuthMethod::Standard | JwtAuthMethod::OneTime => {
                let token = params
                    .token
                    .as_ref()
                    .ok_or_else(|| AppError::unauthorized("missing token"))?;
                Some(auth::validate_jwt_token(token, settings.as_ref())?)
            }
        };

        // Enforce strict one-time semantics for the /studyToken JWT.
        // This is intentionally done early to avoid expensive PACS queries for already-used tokens.
        if matches!(settings.jwt_auth, JwtAuthMethod::OneTime) {
            let token = params
                .token
                .as_deref()
                .ok_or_else(|| AppError::unauthorized("missing token"))?;
            let claims = jwt_claims
                .as_ref()
                .ok_or_else(|| AppError::unauthorized("invalid token"))?;
            session_repo.claim_one_time_token(token, claims.exp).await?;
        }

        // --------------------------------------------------------------
        // 2. BUILD DATABASE QUERY PARAMETERS FROM REQUEST
        // --------------------------------------------------------------
        // Separate search and control parameters
        // `token`` and `accessType are not part of search query
        // --------------------------------------------------------------
        let query = StudyTokenQuery {
            metadata_overrides: settings.dicomarchive.metadata_overrides.as_deref(),
            institution: params.institution.as_deref(),
            // If the client doesn't provide `max`, cap results using settings.
            // Treat `max=0` as "use default" to avoid accidentally returning 0 rows.
            max: params
                .max
                .filter(|m| *m > 0)
                .or(Some(settings.max_default)),
            patient_id: params.patient_id.as_deref(),
            patient_fullname: params.patient_fullname.as_deref(),
            study_instance_uid: params.study_instance_uid.as_deref(),
            accession_number: params.accession_number.as_deref(),
            study_id: params.study_id.as_deref(),
            study_date: params.study_date.as_deref(),
            modality_in_study: params.modality_in_study.as_deref(),
            cuids_in_study: params.cuids_in_study.as_deref(),
            series_instance_uid: params.series_instance_uid.as_deref(),
            series_number: params.series_number.as_deref(),
            series_description: params.series_description.as_deref(),
            modality: params.modality.as_deref(),
            modality_off: params.modality_off.as_deref(),
            sop_class: params.sop_class.as_deref(),
            sop_class_off: params.sop_class_off.as_deref(),
        };


        // ------------------------------------------------------------
        // 3. LOAD STUDY FROM PACS
        // ------------------------------------------------------------

        let include_ohif_metadata = matches!(access_type, AccessType::Ohif);

        let rows = study_repo
            .fetch_study_token_rows(
                query,
                // Always fetch filesystem references (relative path + filesystem_fk).
                // Whether a given instance should be served from disk vs via WADO is decided
                // per-row using PACS timestamps (`use_filesystem`), not by `accessType`.
                // These paths are also required to build OneTime sessions and ZIP sources
                // (file:// vs WADO) even when the response itself is not OHIF.
                true,
                // Reuse the 3rd flag to request extra OHIF metadata (patient/study/series/instance + inst_attrs).
                include_ohif_metadata,
            )
            .await
            .map_err(AppError::Pacs)?;
       

        // ------------------------------------------------------------
        // 4. CREATE DOWNLOAD SESSION (ONLY IF OneTime)
        // ------------------------------------------------------------

        let mut session_id: Option<String> = None;
        let mut session_expires_at: Option<DateTime<Utc>> = None;
        let mut persisted_files_for_zip: Option<Vec<DownloadSessionFile>> = None;

        if matches!(settings.jwt_auth, JwtAuthMethod::OneTime) {
            let claims = jwt_claims
                .as_ref()
                .ok_or_else(|| AppError::unauthorized("invalid token"))?;

            let new_session_id = Uuid::new_v4().to_string();
            let expires_at = DateTime::<Utc>::from_timestamp(claims.exp as i64, 0)
                .ok_or(AppError::Internal(anyhow::anyhow!("Invalid expiration timestamp")))?;

            let session = DownloadSession::new(new_session_id.clone(), expires_at, rows.len() as u32);

            let files = rows
                .iter()
                .enumerate()
                .map(|(idx, r)| DownloadSessionFile {
                    session_id: new_session_id.clone(),
                    file_index: idx as u32,
                    instance_uid: r.sop_instance_uid.clone(),
                    study_uid: r.study_instance_uid.clone(),
                    series_uid: r.series_instance_uid.clone(),
                    use_wado: !r.use_filesystem,
                    filesystem_fk: if r.use_filesystem { r.filesystem_fk } else { None },
                    relative_file_path: if r.use_filesystem { r.relative_file_path.clone() } else { None },
                })
                .collect::<Vec<_>>();

            session_repo.create_session(&session).await?;
            session_repo.add_files(&files).await?;

            session_id = Some(new_session_id);
            session_expires_at = Some(expires_at);

            // Only keep the per-file list in memory if we need it to build ZIP sources.
            if matches!(access_type, AccessType::Zip) {
                persisted_files_for_zip = Some(files);
            }
        }


        // ------------------------------------------------------------
        // 5. BUILD PLAN (domain) and RENDER (presentation)
        // ------------------------------------------------------------

        let total_files = rows.len() as u32;

        let base_url = params.proxy_uri.clone()
            .unwrap_or(settings.dicomarchive.manifest_base_url.clone()
            .unwrap_or(server_base_url.to_string()));

        // Viewer-style clients (OHIF/Weasis/Cornerstone) need per-instance URLs.
        let needs_download_urls = matches!(access_type, AccessType::Ohif | AccessType::Weasis | AccessType::Cornerstone);

        // Standard/None: build stateless signed download tokens.
        let token_urls = if session_id.is_none() && needs_download_urls {
            let now = Utc::now().timestamp().max(0) as usize;
            let mut exp = now + 15 * 60;
            if let Some(claims) = jwt_claims.as_ref() {
                exp = exp.min(claims.exp);
            }
            if exp <= now {
                return Err(AppError::unauthorized("unauthorized"));
            }

            let mut urls = Vec::with_capacity(rows.len());
            for r in &rows {
                let claims = auth::DownloadClaims {
                    aud: "sirius-hip-dl".to_string(),
                    exp,
                    study_uid: r.study_instance_uid.clone(),
                    series_uid: r.series_instance_uid.clone(),
                    sop_uid: r.sop_instance_uid.clone(),
                    filesystem_fk: r.filesystem_fk,
                    relative_file_path: r.relative_file_path.clone(),
                };
                let token = auth::encode_download_token(&claims, settings.as_ref())
                    .map_err(|_| AppError::unauthorized("unauthorized"))?;
                let url = format!("{}/files/{}", base_url, token);
                urls.push(url.clone());
            }
            Some(urls)
        } else {
            None
        };

        // Build retrieval URLs according to access type and session mode.
        // Also build ZIP sources accordingly.
        let retrieve_urls = if matches!(access_type, AccessType::Zip) {
            // ZIP presenter doesn't need per-row retrieval URLs.
            Vec::new()
        } else if let Some(ref sid) = session_id {
            // OneTime/Standard/None: point to local /files/{session}/{index} URLs.
            // - OneTime is enforced by the /files endpoint
            // - Standard/None uses the same endpoint as a proxy
            (0..rows.len())
                .map(|idx| format!("{}/files/{}/{}", base_url, sid, idx))
                .collect::<Vec<_>>()
        } else if let Some(urls) = token_urls {
            urls
        } else {
            // Non-ZIP responses without a session (should be rare).
            rows.iter().map(|r| build_wado_url(settings.as_ref(), r)).collect::<Vec<_>>()
        };

        let zip_sources = if matches!(access_type, AccessType::Zip) {
            if session_id.is_some() {
                // Session-backed: use what we persisted for the session.
                let persisted_files = persisted_files_for_zip.as_ref().ok_or_else(|| {
                    AppError::Internal(anyhow::anyhow!(
                        "Missing persisted file list for ZIP session"
                    ))
                })?;

                persisted_files
                    .iter()
                    .map(|f| {
                        let src = if f.use_wado {
                            format!(
                                "{}?requestType=WADO&studyUID={}&seriesUID={}&objectUID={}&contentType=application/dicom&transferSyntax={}",
                                settings.dicomarchive.wadouri,
                                f.study_uid,
                                f.series_uid,
                                f.instance_uid,
                                settings.dicomarchive.transfer_syntax,
                            )
                        } else {
                            match (f.filesystem_fk, f.relative_file_path.as_deref()) {
                                (Some(fs_id), Some(rel)) => {
                                    if let Some(base) = settings.dicomarchive.get_fs_path_by_id(fs_id) {
                                        format!(
                                            "file://{}/{}",
                                            base.trim_end_matches('/'),
                                            rel.trim_start_matches('/')
                                        )
                                    } else {
                                        format!(
                                            "{}?requestType=WADO&studyUID={}&seriesUID={}&objectUID={}&contentType=application/dicom&transferSyntax={}",
                                            settings.dicomarchive.wadouri,
                                            f.study_uid,
                                            f.series_uid,
                                            f.instance_uid,
                                            settings.dicomarchive.transfer_syntax,
                                        )
                                    }
                                }
                                _ => format!(
                                    "{}?requestType=WADO&studyUID={}&seriesUID={}&objectUID={}&contentType=application/dicom&transferSyntax={}",
                                    settings.dicomarchive.wadouri,
                                    f.study_uid,
                                    f.series_uid,
                                    f.instance_uid,
                                    settings.dicomarchive.transfer_syntax,
                                ),
                            }
                        };
                        (f.instance_uid.clone(), src)
                    })
                    .collect::<Vec<_>>()
            } else {
                // ZIP without session: choose best source per row.
                rows.iter()
                    .map(|r| {
                        let src = if r.use_filesystem {
                            build_absolute_filesystem_path(settings.as_ref(), r)
                                .map(|p| format!("file://{p}"))
                                .unwrap_or_else(|| build_wado_url(settings.as_ref(), r))
                        } else {
                            build_wado_url(settings.as_ref(), r)
                        };
                        (r.sop_instance_uid.clone(), src)
                    })
                    .collect::<Vec<_>>()
            }
        } else {
            // Non-ZIP presenters never use ZIP sources.
            Vec::new()
        };

        let plan = StudyTokenPlan {
            params,
            access_type,
            rows,
            total_files,
            session_id: session_id.clone(),
            expires_at: session_expires_at,
            base_url: base_url.clone(),
            retrieve_urls,
            zip_sources,
        };

        // If serving ZIP in OneTime mode, consume the session up-front.
        // This makes the "one-time" semantics strict even if the ZIP download is interrupted.
        if matches!(plan.access_type, AccessType::Zip) {
            if let Some(ref sid) = plan.session_id {
                session_repo.consume_session(sid).await?;
            }
        }

        let output = match plan.access_type {
            AccessType::Zip => ZipPresenter.render(plan).await?,
            AccessType::Weasis => WeasisPresenter.render(plan).await?,
            AccessType::Ohif => {
                let presenter = OhifPresenter {
                    pool: pacs_pool.clone(),
                    settings: settings.clone(),
                };
                presenter.render(plan).await?
            }
            AccessType::Cornerstone => CornerstonePresenter.render(plan).await?,
        };

        Ok(output)
}


// endregion: === Main use-case function =================================================== //
// ========================================================================================= // 