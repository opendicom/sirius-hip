use std::sync::Arc;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use sqlx::MySqlPool;
use async_trait::async_trait;
use anyhow::Context;
use sha2::{Digest, Sha256};
use dicom_encoding::TransferSyntaxIndex;
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use std::path::{Component, Path, PathBuf};

use crate::api::study_token::params::StudyTokenParams;
use crate::auth::{self, AuthClaims};
use crate::settings::{JwtAuthMethod, Settings};
use crate::src2::errors::app_error::AppError;
use crate::src2::pacs::repositories::study_repository::{StudyRepository, StudyTokenSearchCriteria};
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
}

impl AccessType {
    fn from_param(access_type: &str) -> Option<Self> {
        match access_type {
            "dicom.zip" => Some(AccessType::Zip),
            "weasis.xml" => Some(AccessType::Weasis),
            "ohif" => Some(AccessType::Ohif),
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
        zip: crate::src2::dicomzip::DicomStreamZip,
    },
}

/// Minimal, typed representation of a WADO-URI request.
///
/// We keep this as structured data in the plan to:
/// - avoid allocating long WADO URLs for every instance up-front;
/// - enable late-bound rendering (only build the string when needed);
/// - reuse the same data for filesystem fallback.
#[derive(Debug, Clone)]
struct WadoRef {
    study_uid: String,
    series_uid: String,
    instance_uid: String,
}

impl WadoRef {
    fn from_row(row: &crate::src2::pacs::read_models::StudyTokenRow) -> Self {
        Self {
            study_uid: row.study_instance_uid.clone(),
            series_uid: row.series_instance_uid.clone(),
            instance_uid: row.sop_instance_uid.clone(),
        }
    }

    fn to_url(&self, settings: &Settings) -> String {
        build_wado_url_from_uids(
            settings,
            &self.study_uid,
            &self.series_uid,
            &self.instance_uid,
        )
    }
}

/// Typed ZIP entry source planning.
///
/// This is intentionally **not** a URL string.
/// Filesystem paths are represented as `PathBuf` (cheap, no parsing).
/// WADO sources are represented as `WadoRef` so we can build URLs lazily.
#[derive(Debug, Clone)]
enum ZipSourcePlan {
    /// Prefer filesystem reads, but if the file is missing at streaming time,
    /// fallback to WADO.
    FilesystemPreferred {
        path: PathBuf,
        wado_fallback: WadoRef,
    },
    /// Always use WADO.
    Wado(WadoRef),
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
    session_id: Option<String>,
    base_url: String,
    // Retrieval URLs intended for viewer-style clients.
    // - In OneTime mode: local /files/{session}/{index} URLs (enforced)
    // - Otherwise: local /files/{session}/{index} URLs (proxy)
    retrieve_urls: Vec<String>,
    // Sources suitable for ZIP building (file:// or http(s) WADO).
    // In OneTime mode this matches what was persisted for the session.
    zip_sources: Vec<ZipSourcePlan>,
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
    settings: Arc<Settings>,
}
struct WeasisPresenter {
    settings: Arc<Settings>,
}
struct ZipPresenter {
    settings: Arc<Settings>,
}


// -------------------------------------------------------------------------------- //
// OHIF - StudyTokenPresenter implementation
// -------------------------------------------------------------------------------- //
#[async_trait(?Send)]
impl StudyTokenPresenter for OhifPresenter {
    async fn render(&self, plan: StudyTokenPlan) -> Result<StudyTokenOutput, AppError> {
        use crate::models::ohif::{Instance, InstanceMetadata, Serie, Studies, Study};

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

        // ------------------------------------------------------------------
        // Fast DICOM tag extraction
        // ------------------------------------------------------------------
        // During /studyToken we intentionally do *no filesystem I/O*.
        // If a tag is missing from `inst_attrs`, we return it as None/null.

        #[derive(Default, Clone)]
        struct PixelMeta {
            columns: Option<u16>,
            rows: Option<u16>,
            photometric_interpretation: Option<String>,
            bits_allocated: Option<u16>,
            planar_configuration: Option<u16>,
        }

        fn read_u16_le(buf: &[u8], off: usize) -> Option<u16> {
            let b0 = *buf.get(off)?;
            let b1 = *buf.get(off + 1)?;
            Some(u16::from_le_bytes([b0, b1]))
        }

        fn read_u32_le(buf: &[u8], off: usize) -> Option<u32> {
            let b0 = *buf.get(off)?;
            let b1 = *buf.get(off + 1)?;
            let b2 = *buf.get(off + 2)?;
            let b3 = *buf.get(off + 3)?;
            Some(u32::from_le_bytes([b0, b1, b2, b3]))
        }

        fn trim_dicom_str(bytes: &[u8]) -> String {
            let s = String::from_utf8_lossy(bytes);
            s.trim_matches(|c: char| c == '\u{0}' || c.is_ascii_whitespace())
                .to_string()
        }

        fn parse_first_u16_from_ascii(bytes: &[u8]) -> Option<u16> {
            let s = String::from_utf8_lossy(bytes);
            let first = s.split('\\').next()?.trim();
            first.parse::<u16>().ok()
        }

        fn extract_pixel_meta_explicit_le(dataset: &[u8]) -> PixelMeta {
            // Tags we care about:
            // - Columns (0028,0011) US
            // - Rows (0028,0010) US
            // - PhotometricInterpretation (0028,0004) CS
            // - BitsAllocated (0028,0100) US
            // - PlanarConfiguration (0028,0006) US
            let mut out = PixelMeta::default();

            // VRs with 32-bit length and 2 reserved bytes.
            fn is_long_vr(vr: &[u8; 2]) -> bool {
                matches!(vr, b"OB" | b"OD" | b"OF" | b"OL" | b"OW" | b"SQ" | b"UC" | b"UR" | b"UT" | b"UN")
            }

            let mut i = 0usize;
            while i + 8 <= dataset.len() {
                let group = match read_u16_le(dataset, i) {
                    Some(v) => v,
                    None => break,
                };
                let element = match read_u16_le(dataset, i + 2) {
                    Some(v) => v,
                    None => break,
                };

                // Item / delimiter handling.
                if group == 0xFFFE {
                    let item_len = match read_u32_le(dataset, i + 4) {
                        Some(v) => v,
                        None => break,
                    };
                    i += 8;
                    if item_len == 0xFFFF_FFFF {
                        break;
                    }
                    i = i.saturating_add(item_len as usize);
                    continue;
                }

                let vr = [dataset[i + 4], dataset[i + 5]];
                let (header_len, value_len) = if is_long_vr(&vr) {
                    if i + 12 > dataset.len() {
                        break;
                    }
                    let vlen = match read_u32_le(dataset, i + 8) {
                        Some(v) => v,
                        None => break,
                    };
                    (12usize, vlen)
                } else {
                    if i + 8 > dataset.len() {
                        break;
                    }
                    let vlen = match read_u16_le(dataset, i + 6) {
                        Some(v) => v as u32,
                        None => break,
                    };
                    (8usize, vlen)
                };

                if value_len == 0xFFFF_FFFF {
                    break;
                }

                let value_start = i + header_len;
                let value_end = value_start.saturating_add(value_len as usize);
                if value_end > dataset.len() {
                    break;
                }
                let value = &dataset[value_start..value_end];

                match (group, element) {
                    (0x0028, 0x0011) => {
                        // Columns
                        out.columns = out.columns.or_else(|| {
                            if &vr == b"US" || &vr == b"SS" {
                                read_u16_le(value, 0)
                            } else {
                                parse_first_u16_from_ascii(value)
                            }
                        });
                    }
                    (0x0028, 0x0010) => {
                        // Rows
                        out.rows = out.rows.or_else(|| {
                            if &vr == b"US" || &vr == b"SS" {
                                read_u16_le(value, 0)
                            } else {
                                parse_first_u16_from_ascii(value)
                            }
                        });
                    }
                    (0x0028, 0x0004) => {
                        // PhotometricInterpretation
                        if out.photometric_interpretation.is_none() {
                            let s = trim_dicom_str(value);
                            if !s.is_empty() {
                                out.photometric_interpretation = Some(s);
                            }
                        }
                    }
                    (0x0028, 0x0100) => {
                        // BitsAllocated
                        out.bits_allocated = out.bits_allocated.or_else(|| {
                            if &vr == b"US" || &vr == b"SS" {
                                read_u16_le(value, 0)
                            } else {
                                parse_first_u16_from_ascii(value)
                            }
                        });
                    }
                    (0x0028, 0x0006) => {
                        // PlanarConfiguration
                        out.planar_configuration = out.planar_configuration.or_else(|| {
                            if &vr == b"US" || &vr == b"SS" {
                                read_u16_le(value, 0)
                            } else {
                                parse_first_u16_from_ascii(value)
                            }
                        });
                    }
                    _ => {}
                }

                if out.columns.is_some()
                    && out.rows.is_some()
                    && out.photometric_interpretation.is_some()
                    && out.bits_allocated.is_some()
                    && out.planar_configuration.is_some()
                {
                    break;
                }

                i = value_end;
            }

            out
        }

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
                // 1) If an override is configured, repositories may select it into `row.institution_name`.
                // 2) Otherwise, best-effort decode it from `row.study_attrs`, falling back to `row.inst_attrs`
                //    (first instance).
                let institution_name: Option<String> = row
                    .institution_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        // Best-effort decode: missing/invalid blobs should not break OHIF rendering.
                        let bytes = row
                            .study_attrs
                            .as_deref()
                            .or_else(|| row.inst_attrs.as_deref())?;
                        let dcm = InMemDicomObject::read_dataset_with_ts(bytes, ts).ok()?;
                        let el = dcm
                            .element_opt(dicom_core::Tag(0x0008, 0x0080))
                            .ok()??;
                        el.to_str()
                            .ok()
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
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

            // Pixel metadata tags are only meaningful for image storage SOP classes.
            // Skipping extraction for non-image instances avoids unnecessary dataset decoding and file I/O.
            let is_image_sop_class = crate::constants::SOP_CLASS_SINGLEFRAME
                .contains(&sop_cuid.as_str())
                || crate::constants::SOP_CLASS_MULTIFRAME.contains(&sop_cuid.as_str());

            let (columns, rows_tag, photometric_interpretation, bits_allocated, planar_configuration) =
                if is_image_sop_class {
                    let inst_attrs = row.inst_attrs.as_deref().unwrap_or(&[]);
                    let meta = extract_pixel_meta_explicit_le(inst_attrs);
                    (
                        meta.columns,
                        meta.rows,
                        meta.photometric_interpretation,
                        meta.bits_allocated,
                        meta.planar_configuration,
                    )
                } else {
                    (None, None, None, None, None)
                };

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
// WEASIS - StudyTokenPresenter implementation
// -------------------------------------------------------------------------------- //
#[async_trait(?Send)]
impl StudyTokenPresenter for WeasisPresenter {
    async fn render(&self, plan: StudyTokenPlan) -> Result<StudyTokenOutput, AppError> {
        fn push_attr(xml: &mut String, name: &str, value: &str) {
            xml.push(' ');
            xml.push_str(name);
            xml.push_str("=\"");
            for ch in value.chars() {
                match ch {
                    '&' => xml.push_str("&amp;"),
                    '<' => xml.push_str("&lt;"),
                    '>' => xml.push_str("&gt;"),
                    '"' => xml.push_str("&quot;"),
                    '\'' => xml.push_str("&apos;"),
                    _ => xml.push(ch),
                }
            }
            xml.push('"');
        }

        fn normalize_dicom_date(value: &str) -> std::borrow::Cow<'_, str> {
            if value.as_bytes().contains(&b'-') {
                let mut out = String::with_capacity(value.len());
                for ch in value.chars() {
                    if ch != '-' {
                        out.push(ch);
                    }
                }
                std::borrow::Cow::Owned(out)
            } else {
                std::borrow::Cow::Borrowed(value)
            }
        }

        let mut xml = String::with_capacity(512 + plan.rows.len() * 160);
        xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" ?>"#);
        xml.push_str(r#"<manifest xmlns="http://www.weasis.org/xsd/2.5" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">"#);

        xml.push_str("<arcQuery");

        // Common URL parameters for all authentication methods.
        // We always pass `session` for Weasis so the /wado endpoint can resolve
        // filesystem vs WADO source without touching PACS DB per-instance.
        let session = plan.session_id.as_deref().unwrap_or("");
        let ts = self.settings.dicomarchive.transfer_syntax.as_str();
        match self.settings.jwt_auth {
            JwtAuthMethod::Standard => {
                let token = plan.params.token.as_deref().unwrap_or("");
                push_attr(
                    &mut xml,
                    "additionnalParameters",
                    &format!("&transferSyntax={ts}&session={session}&token={token}"),
                );
            }

            JwtAuthMethod::OneTime => {
                let token = plan.params.token.as_deref().unwrap_or("");
                push_attr(
                    &mut xml,
                    "additionnalParameters",
                    &format!("&transferSyntax={ts}&session={session}&token={token}"),
                );
            }
        }
        push_attr(&mut xml, "arcId", "");
        push_attr(&mut xml, "baseUrl", &format!("{}/wado", plan.base_url));
        push_attr(&mut xml, "requireOnlySOPInstanceUID", "false");
        xml.push('>');

        let mut current_patient_id: Option<&str> = None;
        let mut current_patient_name: Option<&str> = None;
        let mut current_patient_sex: Option<&str> = None;
        let mut current_study_uid: Option<&str> = None;
        let mut current_series_uid: Option<&str> = None;

        let mut patient_open = false;
        let mut study_open = false;
        let mut series_open = false;

        for row in &plan.rows {
            let patient_id = row.patient_id.as_deref().unwrap_or("");
            let patient_name = row.patient_name.as_deref().unwrap_or("");
            let patient_sex = row.patient_sex.as_deref().unwrap_or("");
            let study_uid = row.study_instance_uid.as_str();
            let series_uid = row.series_instance_uid.as_str();

            let patient_changed = current_patient_id != Some(patient_id)
                || current_patient_name != Some(patient_name)
                || current_patient_sex != Some(patient_sex);
            let study_changed = patient_changed || current_study_uid != Some(study_uid);
            let series_changed = study_changed || current_series_uid != Some(series_uid);

            if series_changed && series_open {
                xml.push_str("</Series>");
                series_open = false;
            }
            if study_changed && study_open {
                xml.push_str("</Study>");
                study_open = false;
            }
            if patient_changed && patient_open {
                xml.push_str("</Patient>");
                patient_open = false;
            }

            if patient_changed {
                xml.push_str("<Patient");
                push_attr(&mut xml, "PatientID", patient_id);
                push_attr(&mut xml, "PatientName", patient_name);
                push_attr(&mut xml, "PatientSex", patient_sex);
                xml.push('>');
                patient_open = true;
                current_patient_id = Some(patient_id);
                current_patient_name = Some(patient_name);
                current_patient_sex = Some(patient_sex);
            }

            if study_changed {
                xml.push_str("<Study");
                push_attr(
                    &mut xml,
                    "AccessionNumber",
                    row.accession_no.as_deref().unwrap_or(""),
                );
                push_attr(&mut xml, "ReferringPhysicianName", "");
                let study_date = row.study_date.as_deref().unwrap_or("");
                let normalized_date = normalize_dicom_date(study_date);
                push_attr(&mut xml, "StudyDate", normalized_date.as_ref());
                push_attr(
                    &mut xml,
                    "StudyDescription",
                    row.study_description.as_deref().unwrap_or(""),
                );
                push_attr(&mut xml, "StudyID", "");
                push_attr(&mut xml, "StudyInstanceUID", study_uid);
                push_attr(
                    &mut xml,
                    "StudyTime",
                    row.study_time.as_deref().unwrap_or(""),
                );
                xml.push('>');
                study_open = true;
                current_study_uid = Some(study_uid);
            }

            if series_changed {
                xml.push_str("<Series");
                push_attr(
                    &mut xml,
                    "Modality",
                    row.modality.as_deref().unwrap_or(""),
                );
                push_attr(
                    &mut xml,
                    "SeriesDescription",
                    row.series_description.as_deref().unwrap_or(""),
                );
                push_attr(&mut xml, "SeriesInstanceUID", series_uid);
                push_attr(
                    &mut xml,
                    "SeriesNumber",
                    row.series_no.as_deref().unwrap_or(""),
                );
                xml.push('>');
                series_open = true;
                current_series_uid = Some(series_uid);
            }

            xml.push_str("<Instance");
            push_attr(
                &mut xml,
                "InstanceNumber",
                row.inst_no.as_deref().unwrap_or(""),
            );
            push_attr(&mut xml, "SOPInstanceUID", row.sop_instance_uid.as_str());
            xml.push_str("/>");
        }

        if series_open {
            xml.push_str("</Series>");
        }
        if study_open {
            xml.push_str("</Study>");
        }
        if patient_open {
            xml.push_str("</Patient>");
        }

        xml.push_str("</arcQuery>");
        xml.push_str("</manifest>");

        Ok(StudyTokenOutput::Xml(xml))
    }
}


// -------------------------------------------------------------------------------- //
// DICOM ZIP - StudyTokenPresenter implementation
// -------------------------------------------------------------------------------- //
#[async_trait(?Send)]
impl StudyTokenPresenter for ZipPresenter {
    async fn render(&self, plan: StudyTokenPlan) -> Result<StudyTokenOutput, AppError> {
        let mut zip = crate::src2::dicomzip::DicomStreamZip::new();

        // Name files deterministically. Keep it simple: 0001.dcm, 0002.dcm, ...
        for (idx, source) in plan.zip_sources.iter().enumerate() {
            let name = format!("{:04}.dcm", idx + 1);
            match source {
                ZipSourcePlan::Wado(w) => {
                    zip.add_http_entry(&name, w.to_url(self.settings.as_ref()));
                }
                ZipSourcePlan::FilesystemPreferred { path, wado_fallback } => {
                    zip.add_filesystem_entry_with_http_fallback(
                        &name,
                        path.clone(),
                        wado_fallback.to_url(self.settings.as_ref()),
                    );
                }
            }
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

fn build_wado_url_from_uids(
    settings: &Settings,
    study_uid: &str,
    series_uid: &str,
    instance_uid: &str,
) -> String {
    format!(
        "{}?requestType=WADO&studyUID={}&seriesUID={}&objectUID={}&contentType=application/dicom&transferSyntax={}",
        settings.dicomarchive.wadouri,
        study_uid,
        series_uid,
        instance_uid,
        settings.dicomarchive.transfer_syntax,
    )
}

/// Build absolute filesystem path for given instance row
/// Returns None if filesystem_fk or relative_file_path are missing
fn build_absolute_filesystem_path(
    settings: &Settings,
    row: &crate::src2::pacs::read_models::StudyTokenRow,
) -> Option<PathBuf> {
    let fs_id = row.filesystem_fk?;
    let rel = row.relative_file_path.as_deref()?;
    build_absolute_filesystem_path_by_id(settings, fs_id, rel)
}

fn build_absolute_filesystem_path_by_id(settings: &Settings, fs_id: i32, rel: &str) -> Option<PathBuf> {
    let base = settings.dicomarchive.get_fs_path_by_id(fs_id)?;

    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return None;
    }
    for comp in rel_path.components() {
        match comp {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    Some(Path::new(base).join(rel_path))
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
    _pacs_pool: MySqlPool,
    settings: Arc<Settings>,
    params: StudyTokenParams,
    server_base_url: &str,
) -> Result<StudyTokenOutput, AppError> {

        let access_type = AccessType::from_param(params.access_type.as_str()).ok_or_else(|| {
            AppError::bad_request(format!(
                "unsupported accessType: {} (supported: ohif, weasis.xml, dicom.zip)",
                params.access_type
            ))
        })?;


        // --------------------------------------------------------------
        // 1. VALIDATE JWT TOKEN
        // --------------------------------------------------------------
        let jwt_claims: AuthClaims = {
            let token = params
                .token
                .as_ref()
                .ok_or_else(|| AppError::unauthorized("missing token"))?;
            auth::validate_jwt_token(token, settings.as_ref())?
        };

        // Enforce strict one-time semantics for the /studyToken JWT.
        // This is intentionally done early to avoid expensive PACS queries for already-used tokens.
        if matches!(settings.jwt_auth, JwtAuthMethod::OneTime) {
            let token = params
                .token
                .as_deref()
                .ok_or_else(|| AppError::unauthorized("missing token"))?;
            if session_repo.is_one_time_token_used(token).await? {
                return Err(AppError::TokenAlreadyUsed);
            }
        }

        // --------------------------------------------------------------
        // 2. BUILD REPOSITORY SEARCH CRITERIA FROM REQUEST PARAMETERS
        // --------------------------------------------------------------

        // Separate search and control parameters
        // `token`` and `accessType are not part of search query
        // --------------------------------------------------------------
        let criteria = StudyTokenSearchCriteria {
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
        // Viewer manifests require additional patient/study/series/instance metadata.
        let include_ohif_metadata = matches!(access_type, AccessType::Ohif);
        let include_weasis_metadata = matches!(access_type, AccessType::Weasis);

        // Viewer-style clients (OHIF) need per-instance retrieval URLs.
        let needs_download_urls = matches!(
            access_type,
            AccessType::Ohif | AccessType::Weasis
        );

        // Only request filesystem references when we can actually use them.
        // - OneTime: we persist per-file refs for enforced downloads.
        // - ZIP: we may prefer file:// sources when available.
        // - Viewer-style (OHIF/Weasis): we embed filesystem refs into local
        //   download URLs/tokens so the /files endpoint can serve FS-first (with WADO fallback).
        let filesystem_configured = !settings.dicomarchive.filesystems.is_empty();
        let include_filesystem = filesystem_configured
            && (matches!(settings.jwt_auth, JwtAuthMethod::OneTime)
                || matches!(access_type, AccessType::Zip)
                || needs_download_urls);

        let rows = study_repo
            .fetch_study_token_rows(
                criteria,
                include_filesystem,
                // Request extra viewer metadata depending on access type.
                include_ohif_metadata,
                include_weasis_metadata,
            )
            .await
            .map_err(AppError::Pacs)?;

        // If the PACS indicates a row is eligible for filesystem retrieval, it MUST provide
        // a filesystem reference. Missing refs when `use_filesystem = true` indicates inconsistent
        // PACS data and should not silently fall back to WADO.
        if include_filesystem {
            for r in &rows {
                if r.use_filesystem {
                    let has_fs = r.filesystem_fk.is_some()
                        && matches!(r.relative_file_path.as_deref(), Some(p) if !p.is_empty());
                    if !has_fs {
                        return Err(AppError::MissingFilesystemReference {
                            study_uid: r.study_instance_uid.clone(),
                            series_uid: r.series_instance_uid.clone(),
                            sop_uid: r.sop_instance_uid.clone(),
                        });
                    }
                }
            }
        }

        // ------------------------------------------------------------
        // 4. CREATE DOWNLOAD SESSION
        // ------------------------------------------------------------
        // We always create a session for Weasis so the /wado proxy can resolve
        // filesystem vs WADO source via a single app-DB lookup per instance.
        // This avoids touching the PACS DB on the hot-path.
        let mut session_id: Option<String> = None;
        let mut persisted_files_for_zip: Option<Vec<DownloadSessionFile>> = None;

        let create_session = matches!(settings.jwt_auth, JwtAuthMethod::OneTime)
            || matches!(access_type, AccessType::Weasis | AccessType::Ohif);

        if create_session {
            let exp = jwt_claims.exp;
            let expires_at = DateTime::<Utc>::from_timestamp(jwt_claims.exp as i64, 0)
                .ok_or(AppError::Internal(anyhow::anyhow!("Invalid expiration timestamp")))?;

            // Bind the download session to the JWT token that created it.
            let token_hash = params
                .token
                .as_deref()
                .map(|t| Sha256::digest(t.as_bytes()).to_vec());

            let new_session_id = Uuid::new_v4().to_string();

            let session = DownloadSession::new(
                new_session_id.clone(),
                expires_at,
                rows.len() as u32,
                token_hash,
            );

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

            if matches!(settings.jwt_auth, JwtAuthMethod::OneTime) {
                let token = params
                    .token
                    .as_deref()
                    .ok_or_else(|| AppError::unauthorized("missing token"))?;

                session_repo
                    .create_session_with_files_and_claim_token(&session, &files, token, exp)
                    .await?;
            } else {
                session_repo.create_session_with_files(&session, &files).await?;
            }

            session_id = Some(new_session_id);

            // Only keep the per-file list in memory if we need it to build ZIP sources.
            if matches!(access_type, AccessType::Zip) {
                persisted_files_for_zip = Some(files);
            }
        }

        // ------------------------------------------------------------
        // 5. BUILD PLAN (domain) and RENDER (presentation)
        // ------------------------------------------------------------

        // Determine base URL for retrieval URLs and download tokens:
        // - If a proxy URI is configured, use it (it should be configured to point to this server or a compatible proxy).
        // - Otherwise, use the DICOM archive's manifest base URL if configured (it should be configured to point to this server or a compatible proxy).
        // - Otherwise, fall back to the provided server_base_url (which should be the public URL of this server).
        let base_url = params.proxy_uri.clone()
            .unwrap_or(settings.dicomarchive.manifest_base_url.clone()
            .unwrap_or(server_base_url.to_string()));


         
        // Build retrieval URLs according to access type and session mode.
        // Also build ZIP sources accordingly.
        let retrieve_urls = if matches!(access_type, AccessType::Zip) || matches!(access_type, AccessType::Weasis) {
            // ZIP and Weasis presenter doesn't need per-row retrieval URLs.
            Vec::new()
        } else if let Some(ref sid) = session_id {
            // OneTime/Standard/None: point to local /files/{session}/{index} URLs.
            // - OneTime is enforced by the /files endpoint
            // - Standard/None uses the same endpoint as a proxy
            let token_qs = if matches!(access_type, AccessType::Ohif) {
                Some(
                    params
                        .token
                        .as_deref()
                        .ok_or_else(|| AppError::unauthorized("missing token"))?,
                )
            } else {
                None
            };

            (0..rows.len())
                .map(|idx| {
                    if let Some(t) = token_qs {
                        format!("{}/files/{}/{}?token={}", base_url, sid, idx, t)
                    } else {
                        format!("{}/files/{}/{}", base_url, sid, idx)
                    }
                })
                .collect::<Vec<_>>()
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
                        let wado = WadoRef {
                            study_uid: f.study_uid.clone(),
                            series_uid: f.series_uid.clone(),
                            instance_uid: f.instance_uid.clone(),
                        };

                        // If the session indicates this file should be served via filesystem, prefer that but include WADO fallback.
                        let src = if f.use_wado {
                            ZipSourcePlan::Wado(wado)
                        } else {
                            match (f.filesystem_fk, f.relative_file_path.as_deref()) {
                                (Some(fs_id), Some(rel)) => {
                                    if let Some(path) = build_absolute_filesystem_path_by_id(
                                        settings.as_ref(),
                                        fs_id,
                                        rel,
                                    ) {
                                        ZipSourcePlan::FilesystemPreferred {
                                            path,
                                            wado_fallback: wado,
                                        }
                                    } else {
                                        ZipSourcePlan::Wado(wado)
                                    }
                                }
                                _ => ZipSourcePlan::Wado(wado),
                            }
                        };

                        src
                    })
                    .collect::<Vec<_>>()
            } else {
                // ZIP without session: choose best source per row.
                rows.iter()
                    .map(|r| {
                        let wado = WadoRef::from_row(r);

                        let src = if r.use_filesystem {
                            build_absolute_filesystem_path(settings.as_ref(), r)
                                .map(|p| ZipSourcePlan::FilesystemPreferred {
                                    path: p,
                                    wado_fallback: wado.clone(),
                                })
                                .unwrap_or_else(|| ZipSourcePlan::Wado(wado))
                        } else {
                            ZipSourcePlan::Wado(wado)
                        };

                        src
                    })
                    .collect::<Vec<_>>()
            }
        } else {
            // Non-ZIP presenters never use ZIP sources.
            Vec::new()
        };

        // ZIP responses do not need the full PACS rows after `zip_sources` are computed.
        // Drop them early to reduce peak memory on large studies.
        let rows = if matches!(access_type, AccessType::Zip) {
            Vec::new()
        } else {
            rows
        };

        let plan = StudyTokenPlan {
            params,
            access_type,
            rows,
            session_id: session_id,
            base_url: base_url,
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
            AccessType::Zip => {
                let presenter = ZipPresenter {
                    settings: settings.clone(),
                };
                presenter.render(plan).await?
            }
            AccessType::Weasis => {
                let presenter = WeasisPresenter {
                    settings: settings.clone(),
                };
                presenter.render(plan).await?
            }            
            AccessType::Ohif => {
                let presenter = OhifPresenter {
                    settings: settings.clone(),
                };
                presenter.render(plan).await?
            }
        };

        Ok(output)
}


// endregion: === Main use-case function =================================================== //
// ========================================================================================= // 