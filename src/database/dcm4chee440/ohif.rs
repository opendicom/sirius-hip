use std::{vec, sync::{Arc, Mutex}};

use dicom_dictionary_std::tags as DicomTag;
use dicom_encoding::{TransferSyntaxIndex};
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use dicom_core::Tag;

use sqlx::{MySqlPool, Row};
use futures::TryStreamExt;
use anyhow::{Ok, Context};

use crate::{settings::Settings, 
    models::ohif::{Studies, Study, Serie, Instance, InstanceMetadata},
    api::study_token::params::StudyTokenParams,
    database::{helpers::{calculate_age,get_dicom_element}, QueryBuilder}};

use crate::src2::pacs::infrastructure::mysql_sql_helpers::override_col;


// --------------------------------------------------------------------- //
// -- OHIF main function
// --------------------------------------------------------------------- //

/// Fetch studies from database and return model::ohif::Studies
pub async fn get_studies(pool: &MySqlPool, params: &StudyTokenParams, settings: &Settings, server_base_url: String) -> anyhow::Result<Studies> {
    
    // -- Build query -------------------------------------------------------------------------------
    let mut query_builder = QueryBuilder::new();

    // Study-level InstitutionName (0008,0080) override:
    // If configured, select it as a direct column value, e.g. `study.study_custom1`.
    let institution_name_expr = override_col(
        settings.dicomarchive.metadata_overrides.as_deref(),
        "InstitutionName",
    )
    .unwrap_or_else(|| "NULL".to_string());

    query_builder
        .select("
            study.pk as study_pk,
            study.study_iuid,
            study.study_date,
            study.study_time,
            study.accession_no,
            study.study_desc,
            ref_phys.given_name as study_ref_phys_gname,
            ref_phys.family_name as study_ref_phys_fname,
            ref_phys.middle_name as study_ref_phys_mname,
            study.study_id,
            study.num_instances1 as num_instances,
            study.mods_in_study,

            study_dicomattrs.attrs as study_attrs")
        .select(format!("{institution_name_expr} as institution_name"))
        .select("

            series.pk as serie_pk,
            series.series_iuid,
            series.series_no,
            series.modality,
            series.series_desc,
            CAST(DATE(series.updated_time) as CHAR) as series_updated_time,

            instance.pk as instance_pk,
            instance.sop_iuid,
            instance.sop_cuid,
            instance.inst_no,
            inst_dicomattrs.attrs as instance_attrs,

            patient.pk as patient_pk,
            P.family_name as patient_fname,
            P.given_name as patient_gname,
            P.middle_name as patient_mname,
            patient.pat_birthdate as patient_bdate,
            patient.pat_sex as patient_sex")
        .from("
            study
            INNER JOIN series ON study.pk = series.study_fk
            INNER JOIN instance ON series.pk = instance.series_fk
            INNER JOIN patient ON patient.pk = study.patient_fk
            INNER JOIN person_name AS P ON P.pk = patient.pat_name_fk
            INNER JOIN dicomattrs AS inst_dicomattrs ON inst_dicomattrs.pk = instance.dicomattrs_fk
            LEFT JOIN dicomattrs AS study_dicomattrs ON study_dicomattrs.pk = study.dicomattrs_fk
            LEFT JOIN person_name AS ref_phys ON ref_phys.pk = study.ref_phys_name_fk");

    // If search for PatientID
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
            return Ok(Studies { studies: Box::new(vec![]) });
        }
    } 

    if let Some(value) = &params.patient_fullname {
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

    if let Some(value) = &params.study_date {
        // AAAA-MM-DD|  (equal or newer than AAAA-MM-DD)
        if value.ends_with('|') {
            query_builder.condition("study.study_date >= ?", value.trim_end_matches('|').replace('-', ""));
        }
        
        // |AAAA-MM-DD  (equal or older than AAAA-MM-DD)
        else if value.starts_with('|') {
            query_builder.condition("study.study_date <= ?", value.trim_start_matches('|').replace('-', ""));
        }

        // AAAA-MM-DD|AAAA-DD-MM  (between)
        else if value.contains('|'){
            if let Some((start,end)) = value.split_once('|') {
                query_builder.condition_between("study.study_date BETWEEN ? AND ?", start.replace("-", ""), end.replace("-", ""));
            }
        }
        // AAAA-MM-DD (equal)
        else {
            query_builder.condition("study.study_date = ?",value);
        }
    }

    if let Some(field) = &settings.dicomarchive.institution_field {
        query_builder
            .condition_opt(format!("study.{field} = ?"), params.institution.as_ref());
    }

    query_builder
        .condition_list_opt("study.study_iuid IN ", params.study_instance_uid.as_ref(), '\\')
        .condition_opt("study.accession_no = ?", params.accession_number.as_ref())
        .condition_opt("study.study_id = ?", params.study_id.as_ref())
        .condition_opt("study.mods_in_study REGEXP ?", params.modality_in_study.as_ref())
        .condition_opt("study.cuids_in_study = ?", params.cuids_in_study.as_ref())
        .condition_opt("series.series_iuid = ?", params.series_instance_uid.as_ref())
        .condition_list_opt("series.series_iuid IN ", params.series_instance_uid.as_ref(), '\\')
        .condition_opt("series.modality = ?", params.modality.as_ref())
        .condition_opt("series.sop_cuid = ?", params.sop_class.as_ref())
        .condition_opt("series.sop_class != ?", params.sop_class_off.as_ref())
        .condition_list_opt("series.modality NOT IN ", params.modality_off.as_ref(), '\\')
        .order_by("study.pk")
        .limit(params.max.unwrap_or(settings.max_default));


    // -- Fetch from database ------------------------------------------------------------------- //
    let mut studies: Box<Vec<Study>> = Box::new(Vec::new());
    let mut rows = query_builder.build().fetch(pool);           
    while let Some(row) = rows.try_next().await? {
        let row_study_pk: i32 = row.try_get("study_pk").context("Failed get `study_pk`")?;
        let row_patient_pk: i32 = row.try_get("patient_pk").context("Failed to get `patient_pk`")?;
        let study = match studies.iter_mut().find(|x|**x == row_study_pk) {
            Some(entry) => entry,
            None => {

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
                        .map(|x|x.pat_id)
                        .context("Failed to get `pat_id`")?
                    }
                };

                let mut patient_name = String::new();
                if let Some(value) = row.try_get("patient_fname").context("Failed to get `patient_fname`")? {
                    patient_name.push_str(value);
                }
                if let Some(value) = row.try_get("patient_gname").context("Failed to get `patient_gname`")? {
                    patient_name.push('^');
                    patient_name.push_str(value);
                }
                if let Some(value) = row.try_get("patient_mname").context("Failed to get `patient_mname`")? {
                    patient_name.push(' ');
                    patient_name.push_str(value);
                }

                let patient_age = row.try_get::<Option<String>, &str>("patient_bdate")
                    .context("Failed to get `patient_bdate`")?
                    .map(|bdate| calculate_age(bdate))
                    .transpose()
                    .context("Failed to transpose `patient_bdate`")?
                    .map(|age| age.to_string());

                let institution_name: Option<String> = row
                    .try_get::<Option<String>, &str>("institution_name")
                    .ok()
                    .flatten()
                    .and_then(|s| {
                        let trimmed = s.trim().to_string();
                        (!trimmed.is_empty()).then_some(trimmed)
                    })
                    .or_else(|| {
                        let bytes: Option<Vec<u8>> = row.try_get("study_attrs").ok()?;
                        let bytes = bytes?;
                        let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.2.1").unwrap();
                        let dcm = InMemDicomObject::read_dataset_with_ts(bytes.as_slice(), ts).ok()?;
                        let el = dcm.element_opt(Tag(0x0008, 0x0080)).ok()??;
                        el.to_str().ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
                    });

                studies.push(
                    Study {
                        study_pk:  row_study_pk,
                        study_iuid:  row.try_get("study_iuid")
                                        .context("Failed to get `study_iuid`")?,

                        study_date:  row.try_get::<&str,&str>("study_date")
                                        .context("Failed to get `study_date`")?
                                        .replace('-',"")
                                        .to_string(),

                        study_time:  row.try_get("study_time")
                                        .context("Failed to get `study_time`")?,
                        study_description: row.try_get("study_desc").context("Failed to get `study_desc`")?,
                        patient_name,
                        patient_id,
                        accession_no:  row.try_get("accession_no").context("Failed to get `accession_no`")?,
                        patient_age,
                        patient_sex: row.try_get("patient_sex").context("Failed to get `patient_sex`")?,
                        num_instances: row.try_get("num_instances").context("Failed to get `num_instances`")?,
                        modalities: row.try_get("mods_in_study").context("Failed to get `mods_in_study`")?,
                        institution_name,
                        series: vec![], 
                    });
                    studies.last_mut().unwrap()
            } 
        };


        // -- Series
        let row_serie_pk: i32 = row.try_get("serie_pk").context("Failed to get `serie_pk`")?;
        let serie = match study.series.iter_mut().find(|x|**x == row_serie_pk) {
            Some(entry) => entry,
            None => {
                study.series.push(
                    Serie {
                        serie_pk:       row_serie_pk,
                        series_iuid:    row.try_get("series_iuid")
                                            .context("Failed to get `series_iuid`")?,

                        series_no:      row.try_get::<&str,&str>("series_no")
                                            .context("Failed to get `series_no`")?
                                            .parse::<i32>()
                                            .context("Failed to parse series_no to integer")?,

                        modality:       row.try_get("modality")
                                            .context("Failed to get `modality`")?,

                        series_description:     row.try_get("series_desc")
                                                    .context("Failed to get `series_desc`")?,
                        instances: vec![],
                    });
                    study.series.last_mut().unwrap()
            }
        };

        // -- Instance
        let instance_sop_iuid: String = row.try_get("sop_iuid").context("Failed to get `sop_iuid`")?;
        let instance_pk: i32 = row.try_get("instance_pk").context("Failed to get `instance_pk`")?;

        // Build URL
        let url= format!("dicomweb:{}?requestType=WADO&studyUID={}&seriesUID={}&objectUID={}&transferSyntax={}&contentType=application/dicom{}{}{}{}",
            params.proxy_uri.as_ref()
                .unwrap_or(settings.dicomarchive.manifest_base_url.as_ref()
                .unwrap_or(&format!("{}/wado",&server_base_url))),
            study.study_iuid, 
            serie.series_iuid, 
            instance_sop_iuid,
            settings.dicomarchive.transfer_syntax,
            params.session
                .as_ref()
                .map_or(String::new(),|val|format!("&session={val}")), 
            settings.dicomarchive.custodianoid
                .as_ref()
                .map_or(String::new(), |x|format!("&custodianOID={x}")),
            settings.dicomarchive.pacsoid
                .as_ref()
                .map_or(String::new(), |x|format!("&arcId={x}")), 
            params.token
                .as_ref()
                .map_or(String::new(),|val|format!("&token={val}")),
        ); 

        /*
        [OHIF DICOM Json Data Source documentation](https://docs.ohif.org/configuration/dataSources/dicom-json) describe the required json
        data to build the manifest.
        A quick visualization test for some commons studies (US,CR,MG,CT), show us that some of the Dicom Tags discribed there,
        are optional, so we decied to not search that tags to priorize the performance.
        If we detect that some of them are required for any reason, they will be added in a future release.

        Some Dicom Tags required to compose the OHIF DICOM Json manifest, can be retrieved from the database 
        (for examle table instance.inst_attrs in dcm4chee v2.18.3 if configured in dcm4chee-attribute-filer.xml) or 
        directly from the dicom file.
        We try to search each of these tags in the database, if any are not present we search the dicom file.
        */

        // Variables used if can't fetch all required dicom attributes from database
        let filepath: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));

        let mut file_dicomattrs = InMemDicomObject::new_empty();

        let db_dicomattrs = {
            let buf: Vec<u8> = row.try_get("instance_attrs").context("Failed to get `instance_attrs`")?;

            // Explicit Little Endian (Transfer syntax encoding we suppose dcm4chee use to store data in ins_attrs database field)
            let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.2.1").unwrap(); 
            
            InMemDicomObject::read_dataset_with_ts(buf.as_slice(), ts)
                .context("Failed to read dicomattrs.attrs value from database")?
        };
        
        let tag = DicomTag::COLUMNS;                // (0028,0011) US
        let columns: Option<u16> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value| value.to_int())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to u16"))?;  

        let tag = DicomTag::ROWS;                   // (0028,0010) US
        let rows: Option<u16> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value| value.to_int())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to u16"))?;
        
        let tag = DicomTag::PHOTOMETRIC_INTERPRETATION; // (0028,0004) CS 
        let photometric_interpretation: Option<String> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value|value.to_str())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to String"))?
            .map(|s|s.to_string());

        let tag = DicomTag::BITS_ALLOCATED;         // (0028,0100) US
        let bits_allocated: Option<u16> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value| value.to_int())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to u16"))?;

        // DicomTag::PLANAR_CONFIGURATION // (0028,0006)
        // Required only for color images. DicomTag PhotometricInterpretation (0029,0004) specifies the image type (monocrome or colored) 
        let planar_configuration: Option<u16> = 
        if let Some(value) = &photometric_interpretation {
            if value != "MONOCHROME1" && value != "MONOCHROME2" {
                let tag = DicomTag::PLANAR_CONFIGURATION;
                get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
                    .await?
                    .map(|value| value.to_int())
                    .transpose()
                    .context(format!("Failed to parse DicomTag {tag} value to u16"))?         
            } else {
                None
            }
        } else {
            None
        };

        serie.instances.push(
            Instance {
                instance_pk,
                metadata: InstanceMetadata {
                    instance_no: row.try_get::<String,&str>("inst_no")
                                    .context("Failed to get `inst_no`")?
                                    .parse::<i32>()
                                    .context("Failed to parse inst_no value to integer")?,

                    instance_sop_cuid: row.try_get("sop_cuid").context("Failed to get `sop_cuid`")?,

                    series_modality: serie.modality.clone(),
                    instance_sop_iuid,
                    series_iuid: serie.series_iuid.clone(),
                    study_iuid: study.study_iuid.clone(),
                    series_date: row.try_get::<&str,&str>("series_updated_time")
                                    .context("Failed to get `series_updated_time`")?
                                    .replace('-',"")
                                    .to_string(),
                    columns,
                    rows,
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
                    rescale_intercept: None,
                    rescale_slope: None,
                    planar_configuration,
                    number_of_frames: None,
                    frame_time: None,
                },
                url,
        });
    }
    
    Ok(Studies { studies })
       
}
