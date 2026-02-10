
// --------------------------------------------------------------------- //
// -- QIDO main function
// --------------------------------------------------------------------- //

// Query parameters are defined in the dicom standar where:
// https://dicom.nema.org/medical/dicom/2018a/output/chtml/part18/sect_6.7.html#table_6.7.1-1
//
// Returned attributes are defined in the dicom standar where:
// https://dicom.nema.org/medical/dicom/2018a/output/chtml/part18/sect_6.7.html#table_6.7.1-2


use anyhow::bail;
use dicom_object::InMemDicomObject;
use once_cell::sync::Lazy;
use sqlx::mysql::MySqlRow;
use sqlx::{MySqlPool, Row};
use futures::TryStreamExt;

use dicom_core::value::PrimitiveValue;
use dicom_core::{DataElement, Tag, VR};
use dicom_json::DicomJson;

use std::collections::{HashMap, HashSet};

use crate::{api::qido::QidoStudiesParams, database::QueryBuilder, models::qido::Qido, settings::Settings};


// Using dispatcher functions to improve performance over code like `if "PatientID" || "00100020" .. else {... }``

/// Handlers function definitions
type AttrHandler = (
    fn(&QidoStudiesParams, &mut QueryBuilder), 
    fn(&MySqlRow, &mut InMemDicomObject) -> anyhow::Result<()>
);

pub static ATTR_DISPATCHER: Lazy<HashMap<&'static str, AttrHandler>> = Lazy::new(|| {
    let mut m: HashMap<&'static str, AttrHandler> = HashMap::new();
    
    // SOPClassesInStudy 00080062
    m.insert("00080062", (handle_db_sop_classess_in_study, handle_dcm_sop_classess_in_study)); 

    // StudyDescription 00081030
    m.insert("00081030", (handle_db_study_description, handle_dcm_study_description));

    // IssuerOfPatientID 00100021
    m.insert("00100021", (handle_db_issuer_of_patient_id, handle_dcm_issuer_of_patient_id));

    m
});

// -------------------------------------------------------------------------
// -- Database attribute handlers
// -------------------------------------------------------------------------

/// Handler database for SOPClassesInStudy 00080062
fn handle_db_sop_classess_in_study(_params: &QidoStudiesParams, query_builder: &mut QueryBuilder) {
    query_builder.select("study.cuids_in_study as '00080062' ");
}

/// Handler database for StudyDescription 00081030
fn handle_db_study_description(_params: &QidoStudiesParams, query_builder: &mut QueryBuilder) {
    query_builder.select("study.study_desc as '00081030' ");
}

/// Handler database for IssuerOfPatientID 00100021
fn handle_db_issuer_of_patient_id(params: &QidoStudiesParams, query_builder: &mut QueryBuilder) {
    // Skip the patient_id table join if `patient_id` is Some, 
    // because the join has already been applied to filter by the patient_id
    if params.patient_id.is_none(){
        query_builder.from("INNER JOIN patient_id ON patient_id.patient_fk = patient.pk");
    } 

    query_builder.from("INNER JOIN issuer ON patient_id.issuer_fk = issuer.pk");
    query_builder.select("issuer.entity_id as '00100021' ");
}


// -------------------------------------------------------------------------
// -- Dicom attribute handlers
// -------------------------------------------------------------------------

/// Handler dicom for SOPClassesInStudy 00080062
fn handle_dcm_sop_classess_in_study(row: &MySqlRow, dcm: &mut InMemDicomObject) -> anyhow::Result<()> {
    dcm.put_element(DataElement::new(
        Tag(0x0008, 0x0062),
        VR::CS,
        PrimitiveValue::from(
            row.try_get::<&str,&str>("00080062")?)
    ));
    Ok(())
}

/// Handler dicom for StudyDescription 00081030
fn handle_dcm_study_description(row: &MySqlRow, dcm: &mut InMemDicomObject) -> anyhow::Result<()> {
    dcm.put_element(DataElement::new(
        Tag(0x0008, 0x1030),
        VR::LO,
        PrimitiveValue::from(
            row.try_get::<&str,&str>("00081030")?)
    ));
    Ok(())
}

/// Handler dicom for IssuerOfPatientID 00100021
fn handle_dcm_issuer_of_patient_id(row: &MySqlRow, dcm: &mut InMemDicomObject) -> anyhow::Result<()> {
    dcm.put_element(DataElement::new(
        Tag(0x0010, 0x0021),
        VR::LO,
        PrimitiveValue::from(
            row.try_get::<&str,&str>("00100021")?)
    ));
    Ok(())
}

/// Fetch studies from database and return model::ohif::Studies
pub async fn get_studies(
    pool: &MySqlPool, 
    params: &QidoStudiesParams, 
    validated_include_fields: HashSet<&'static str>, 
    settings: &Settings
) -> anyhow::Result<Qido> {


   
    
    // -- Build query -------------------------------------------------------------------------------
    let mut query_builder = QueryBuilder::new();
    query_builder
        .select("

            study.study_date,
            study.study_time,
            study.accession_no,

            study.mods_in_study,
            ref_phys.given_name as study_ref_phys_gname,
            ref_phys.family_name as study_ref_phys_fname,
            ref_phys.middle_name as study_ref_phys_mname,


            P.family_name as patient_fname,
            P.given_name as patient_gname,
            P.middle_name as patient_mname,

            patient.pk as patient_pk,
            patient.pat_birthdate as patient_bdate,
            patient.pat_sex as patient_sex,
            study.study_iuid,
            study.study_id,
            study.num_series1 as num_series,
            study.num_instances1 as num_instances,
            dicomattrs.attrs"
        )
        .from("
            study
            INNER JOIN patient ON patient.pk = study.patient_fk
            INNER JOIN person_name AS P ON P.pk = patient.pat_name_fk
            INNER JOIN dicomattrs ON dicomattrs.pk = study.dicomattrs_fk
            LEFT JOIN person_name AS ref_phys ON ref_phys.pk = study.ref_phys_name_fk");

    // -- Search for PatientID
    if let Some(value) = &params.patient_id {

        // and exist in the database
        if let Some (row) =  sqlx::query!(
            r#"SELECT patient_id.pk FROM patient_id WHERE patient_id.pat_id = ?"#,
            value)
            .fetch_optional(pool)
            .await? 
        {
            query_builder
                .select("patient_id.pat_id as patient_id")
                .from("INNER JOIN patient_id ON patient_id.patient_fk = patient.pk")
                .condition("patient_id.pk = ?", row.pk);

        } else {
            return Ok(Qido::new());
        }
    } 

    // -- Search for Patient Name
    if let Some(value) = &params.patient_name {
        query_builder.condition_push("(");
        for val in value.split(&[' ','^','>']){
            query_builder.condition_push("
                P.family_name REGEXP ? OR 
                P.given_name  REGEXP ? OR 
                P.middle_name REGEXP ? ")
            .bind(val)
            .bind(val)
            .bind(val);

        }
        query_builder.condition_push(")");
    }

    // -- Search for Referring Physician Name
    if let Some(value) = &params.referring_physician_name {
        query_builder.condition_push("(");
        for val in value.split(&[' ','^','>']){
            query_builder.condition_push("
            ref_phys.family_name REGEXP ? OR 
            ref_phys.given_name  REGEXP ? OR 
            ref_phys.middle_name REGEXP ? ")
            .bind(val)
            .bind(val)
            .bind(val);

        }
        query_builder.condition_push(")");
    }

    // -- Search for Study Date
    // -- https://dicom.nema.org/dicom/2013/output/chtml/part04/sect_C.2.html#sect_C.2.2.2.5

    if let Some(value) = &params.study_date {
        // AAAAMMDD-  (equal or newer than AAAAMMDD)
        if value.ends_with('-') {
            query_builder.condition("study.study_date >= ?", value.trim_end_matches('|').replace('-', ""));
        }
        
        // -AAAAMMDD  (equal or older than AAAAMMDD)
        else if value.starts_with('-') {
            query_builder.condition("study.study_date <= ?", value.trim_start_matches('|').replace('-', ""));
        }

        // AAAAMMDD-AAAADDMM  (between)
        else if value.contains('-'){
            if let Some((start,end)) = value.split_once('-') {
                query_builder.condition_between("study.study_date BETWEEN ? AND ?", start, end);
            }
        }
        // AAAAMMDD (equal)
        else {
            query_builder.condition("study.study_date = ?",value);
        }
    }

    // TEST
    // -- Search for Study Time
    // -- https://dicom.nema.org/dicom/2013/output/chtml/part04/sect_C.2.html#sect_C.2.2.2.5
    if let Some(value) = &params.study_time {
        // HHMM-  (equal or newer than HH:MM)
        if value.ends_with('-') {
            query_builder.condition("study.study_date >= ?", value.trim_end_matches('|').replace('-', ""));
        }
        
        // -HHMM  (equal or older than HH:MM)
        else if value.starts_with('-') {
            query_builder.condition("study.study_date <= ?", value.trim_start_matches('|').replace('-', ""));
        }

        // HHMM-HHMM  (between)
        else if value.contains('-'){
            if let Some((start,end)) = value.split_once('-') {
                query_builder.condition_between("study.study_date BETWEEN ? AND ?", start, end);
            }
        }
        // HHMM (equal)
        else {
            query_builder.condition("study.study_date = ?",value);
        }
    }

    query_builder
        .condition_opt("study.accession_no = ?", params.accession_no.as_ref())
        .condition_opt("study.mods_in_study REGEXP ?", params.modalities_in_study.as_ref())
        .condition_list_opt("study.study_iuid IN ", params.study_iuid.as_ref(), '\\')
        .condition_opt("study.study_id = ?", params.study_id.as_ref())
        //offset???????
        .limit(params.limit.unwrap_or(settings.max_default));


    // Add includefield params
    for field in &validated_include_fields {
        if let Some(handler) = ATTR_DISPATCHER.get(field) {
            handler.0(&params, &mut query_builder);
        } else {
            bail!("No database handler defined for dicom attribute {}", field);
        }
    }


    // -- Fetch from database ------------------------------------------------------------------- //
    
    let mut qido = Qido::new();
    let mut rows = query_builder.build().fetch(pool);           
    while let Some(row) = rows.try_next().await? {

        // QIDO-RS STUDY Returned Attributes
        // https://dicom.nema.org/medical/dicom/2018a/output/chtml/part18/sect_6.7.html#table_6.7.1-2

        let mut dicomobj = InMemDicomObject::new_empty();

        // Specific Character Set
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0008, 0x0005),
                VR::CS,
                PrimitiveValue::from("ISO_IR 100"),
        ));
        
        // Study Date
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0008, 0x0020),
                VR::DA,
                PrimitiveValue::from(
                    row.try_get::<&str,&str>("study_date")?)
        ));

        // Study Time
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0008, 0x0030),
                VR::TM,
                PrimitiveValue::from(
                    row.try_get::<&str,&str>("study_time")?)
        ));

        // Accession Number
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0008, 0x0050),
                VR::SH,
                PrimitiveValue::from(
                    row.try_get::<&str,&str>("accession_no")?)
        ));

        // Instance Availability
        // TODO: Default value, check ONLINE/NEARLINE Storage....
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0008, 0x0056),
                VR::CS,
                PrimitiveValue::from("ONLINE")
        ));

        // Modalities in Study
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0008, 0x0061),
                VR::CS,
                PrimitiveValue::from(
                    row.try_get::<&str,&str>("mods_in_study")?)
        ));

        // Referring Physician's Name
        let mut ref_physician_name = String::new();
        if let Some(value) = row.try_get("study_ref_phys_fname")? {
            ref_physician_name.push_str(value);
        }
        if let Some(value) = row.try_get("study_ref_phys_gname")? {
            ref_physician_name.push('^');
            ref_physician_name.push_str(value);
        }
        if let Some(value) = row.try_get("study_ref_phys_mname")? {
            ref_physician_name.push(' ');
            ref_physician_name.push_str(value);
        }
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0008, 0x0090),
                VR::PN,
                PrimitiveValue::from(ref_physician_name)
        ));

        // Timezone Offset From UTC
        // May be absent if no value is available

        // Retrieve URL
        // Shall be empty if the resource cannot be retrieved via WADO-RS
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0008, 0x1190),
                VR::UR,
                PrimitiveValue::from("")
        ));

        // Patient Name
        let mut patient_name = String::new();
        if let Some(value) = row.try_get("patient_fname")? {
            patient_name.push_str(value);
        }
        if let Some(value) = row.try_get("patient_gname")? {
            patient_name.push('^');
            patient_name.push_str(value);
        }
        if let Some(value) = row.try_get("patient_mname")? {
            patient_name.push(' ');
            patient_name.push_str(value);
        }
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0010, 0x0010),
                VR::PN,
                PrimitiveValue::from(patient_name)
        ));

        // Patient ID
        let row_patient_pk: i32 = row.try_get("patient_pk")?;
        let patient_id = match &params.patient_id {
            Some(value) => value.into(),
            None => {
                sqlx::query!("
                    SELECT patient_id.pat_id 
                    FROM patient_id 
                    WHERE patient_fk = ? LIMIT 1 ",
                    row_patient_pk
                ).fetch_one(pool)
                .await
                .map(|x|x.pat_id)?
            }
        };

        dicomobj.put_element(
            DataElement::new(
                Tag(0x0010, 0x0020),
                VR::LO,
                PrimitiveValue::from(patient_id)
        ));

        // Patient's Birth Date
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0010, 0x0030),
                VR::DA,
                PrimitiveValue::from(
                    row.try_get::<&str,&str>("patient_bdate")?)
        ));

        // Patient's Sex
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0010, 0x0040),
                VR::CS,
                PrimitiveValue::from(
                    row.try_get::<&str,&str>("patient_sex")?)
        ));

        // Study Instance UID
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0020, 0x000D),
                VR::UI,
                PrimitiveValue::from(
                    row.try_get::<&str,&str>("study_iuid")?)
        ));

        // Study ID
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0020, 0x0010),
                VR::SH,
                PrimitiveValue::from(
                    row.try_get::<&str,&str>("study_id")?)
        ));

        // Number of Study Related Series
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0020, 0x1206),
                VR::IS,
                PrimitiveValue::from(
                    row.try_get::<i32,&str>("num_series")?)
        ));

        // Number of Study Related Instances
        dicomobj.put_element(
            DataElement::new(
                Tag(0x0020, 0x1208),
                VR::IS,
                PrimitiveValue::from(
                    row.try_get::<i32,&str>("num_instances")?)
        ));


        for field in &validated_include_fields {
            if let Some(handler) = ATTR_DISPATCHER.get(field) {
                handler.1(&row, &mut dicomobj)?;
            } else {
                bail!("No database handler defined for dicom attribute {}", field);
            }
        }

        qido.add_dicom_json(DicomJson::from(dicomobj));

    }

    Ok(qido)
}