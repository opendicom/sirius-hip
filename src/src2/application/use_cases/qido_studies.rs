use std::collections::HashSet;

use dicom_core::value::PrimitiveValue;
use dicom_core::{DataElement, Tag, VR};
use dicom_json::DicomJson;
use dicom_object::InMemDicomObject;
use log::error;

use crate::api::qido::QidoStudiesParams;
use crate::constants::QIDO_STUDY_INCLUDEFIELD_DIC;
use crate::models::qido::Qido;
use crate::src2::errors::app_error::AppError;
use crate::src2::pacs::repositories::study_repository::{
    QidoStudiesIncludeFields, QidoStudiesQuery,
};
use crate::src2::state2::AppState2;

fn to_dicom_date(date: &str) -> String {
    date.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn to_dicom_time(time: &str) -> String {
    time.chars().filter(|c| c.is_ascii_digit()).collect()
}

pub async fn execute_qido_studies(
    params: QidoStudiesParams,
    state: &AppState2,
) -> Result<Qido, AppError> {
    // --------------------------------------------------------------
    // 1. VALIDATE `includefield` PARAMETERS
    // --------------------------------------------------------------

    let mut validated_include_fields: HashSet<&'static str> = HashSet::new();
    if let Some(fields) = &params.includefield {
        validated_include_fields.reserve(fields.len());
        for field in fields {
            if let Some(tag) = QIDO_STUDY_INCLUDEFIELD_DIC.get(field.as_str()) {
                validated_include_fields.insert(*tag);
            } else {
                error!("Invalid includefield parameter: {field}");
                return Err(AppError::BadRequest);
            }
        }
    }

    let include = QidoStudiesIncludeFields {
        includefield_00080062: validated_include_fields.contains("00080062"),
        includefield_00081030: validated_include_fields.contains("00081030"),
        includefield_00100021: validated_include_fields.contains("00100021"),
    };

    let limit = params.limit.unwrap_or(state.settings.max_default);

    // --------------------------------------------------------------
    // 2. BUILD REPOSITORY QUERY
    // --------------------------------------------------------------

    let repo_query = QidoStudiesQuery {
        metadata_overrides: state.settings.dicomarchive.metadata_overrides.as_deref(),
        patient_id: params.patient_id.as_deref(),
        patient_name: params.patient_name.as_deref(),
        referring_physician_name: params.referring_physician_name.as_deref(),
        accession_no: params.accession_no.as_deref(),
        modalities_in_study: params.modalities_in_study.as_deref(),
        study_iuid: params.study_iuid.as_deref(),
        study_id: params.study_id.as_deref(),
        study_date: params.study_date.as_deref(),
        study_time: params.study_time.as_deref(),
        limit,
        offset: params.offset,
    };

    // --------------------------------------------------------------
    // 3. FETCH STUDY ROWS FROM REPOSITORY
    // --------------------------------------------------------------
    let rows = state
        .pacs
        .study_repo
        .fetch_qido_studies_rows(repo_query, include)
        .await
        .map_err(AppError::Pacs)?;

    // --------------------------------------------------------------
    // 4. CONVERT ROWS TO DICOM JSON
    // --------------------------------------------------------------

    let mut qido = Qido::new();

    for row in rows {
        let mut dicomobj = InMemDicomObject::new_empty();

        // Specific Character Set
        dicomobj.put_element(DataElement::new(
            Tag(0x0008, 0x0005),
            VR::CS,
            PrimitiveValue::from("ISO_IR 100"),
        ));

        let study_date: String = row.study_date;
        let study_time: String = row.study_time;

        dicomobj.put_element(DataElement::new(
            Tag(0x0008, 0x0020),
            VR::DA,
            PrimitiveValue::from(to_dicom_date(&study_date)),
        ));

        dicomobj.put_element(DataElement::new(
            Tag(0x0008, 0x0030),
            VR::TM,
            PrimitiveValue::from(to_dicom_time(&study_time)),
        ));

        let accession_no: Option<String> = row.accession_no;
        dicomobj.put_element(DataElement::new(
            Tag(0x0008, 0x0050),
            VR::SH,
            PrimitiveValue::from(accession_no.unwrap_or_default()),
        ));

        dicomobj.put_element(DataElement::new(
            Tag(0x0008, 0x0056),
            VR::CS,
            PrimitiveValue::from("ONLINE"),
        ));

        let mods: Option<String> = row.mods_in_study;
        dicomobj.put_element(DataElement::new(
            Tag(0x0008, 0x0061),
            VR::CS,
            PrimitiveValue::from(mods.unwrap_or_default()),
        ));

        let ref_phys: Option<String> = row.ref_physician;
        dicomobj.put_element(DataElement::new(
            Tag(0x0008, 0x0090),
            VR::PN,
            PrimitiveValue::from(ref_phys.unwrap_or_default()),
        ));

        // Retrieve URL (empty, consistent with existing implementation)
        dicomobj.put_element(DataElement::new(
            Tag(0x0008, 0x1190),
            VR::UR,
            PrimitiveValue::from(""),
        ));

        let pat_name: Option<String> = row.pat_name;
        dicomobj.put_element(DataElement::new(
            Tag(0x0010, 0x0010),
            VR::PN,
            PrimitiveValue::from(pat_name.unwrap_or_default()),
        ));

        let pat_id: Option<String> = row.pat_id;
        dicomobj.put_element(DataElement::new(
            Tag(0x0010, 0x0020),
            VR::LO,
            PrimitiveValue::from(pat_id.unwrap_or_default()),
        ));

        let pat_birthdate: Option<String> = row.pat_birthdate;
        dicomobj.put_element(DataElement::new(
            Tag(0x0010, 0x0030),
            VR::DA,
            PrimitiveValue::from(to_dicom_date(&pat_birthdate.unwrap_or_default())),
        ));

        let pat_sex: Option<String> = row.pat_sex;
        dicomobj.put_element(DataElement::new(
            Tag(0x0010, 0x0040),
            VR::CS,
            PrimitiveValue::from(pat_sex.unwrap_or_default()),
        ));

        let study_iuid: String = row.study_iuid;
        dicomobj.put_element(DataElement::new(
            Tag(0x0020, 0x000D),
            VR::UI,
            PrimitiveValue::from(study_iuid),
        ));

        let study_id: Option<String> = row.study_id;
        dicomobj.put_element(DataElement::new(
            Tag(0x0020, 0x0010),
            VR::SH,
            PrimitiveValue::from(study_id.unwrap_or_default()),
        ));

        let num_series: i64 = row.num_series;
        dicomobj.put_element(DataElement::new(
            Tag(0x0020, 0x1206),
            VR::IS,
            PrimitiveValue::from(num_series),
        ));

        let num_instances: i64 = row.num_instances;
        dicomobj.put_element(DataElement::new(
            Tag(0x0020, 0x1208),
            VR::IS,
            PrimitiveValue::from(num_instances),
        ));

        // includefield extras (only those requested)
        if validated_include_fields.contains("00080062") {
            let v: Option<String> = row.includefield_00080062;
            dicomobj.put_element(DataElement::new(
                Tag(0x0008, 0x0062),
                VR::CS,
                PrimitiveValue::from(v.unwrap_or_default()),
            ));
        }
        if validated_include_fields.contains("00081030") {
            let v: Option<String> = row.includefield_00081030;
            dicomobj.put_element(DataElement::new(
                Tag(0x0008, 0x1030),
                VR::LO,
                PrimitiveValue::from(v.unwrap_or_default()),
            ));
        }
        if validated_include_fields.contains("00100021") {
            let v: Option<String> = row.includefield_00100021;
            dicomobj.put_element(DataElement::new(
                Tag(0x0010, 0x0021),
                VR::LO,
                PrimitiveValue::from(v.unwrap_or_default()),
            ));
        }

        qido.add_dicom_json(DicomJson::from(dicomobj));
    }

    Ok(qido)
}
