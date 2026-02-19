use std::{vec, sync::{Arc, Mutex}};

use dicom_encoding::TransferSyntaxIndex;
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;
use dicom_dictionary_std::tags as DicomTag;
use dicom_core::Tag;

use sqlx::{MySqlPool, Row};
use futures::TryStreamExt;
use anyhow::{Result, Context};
use core::result::Result::Ok;

use crate::{settings::Settings, 
            models::ohif::{Studies, Study, Serie, Instance, InstanceMetadata},
            api::study_token::params::StudyTokenParams,
            database::QueryBuilder};

use crate::database::helpers::{get_dicom_element,calculate_age};
use crate::src2::pacs::infrastructure::mysql_sql_helpers::override_col;

pub async fn get_studies(pool: &MySqlPool, params: &StudyTokenParams, settings: &Settings, server_base_url: String) -> Result<Studies> {
    
    // -- Build SQL query 

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
            patient.pat_name,
            patient.pat_id,
            patient.pat_sex,
            CAST(patient.pat_birthdate as CHAR) as pat_birthdate,

            study.pk as study_pk,
            study.study_iuid,
            CAST(DATE(study.study_datetime) as CHAR) as study_date,
            CAST(TIME(study.study_datetime) as CHAR) as study_time,
            study.accession_no,
            study.num_instances,
            study.mods_in_study,

            study.study_attrs as study_attrs")
        .select(format!("{institution_name_expr} as institution_name"))
        .select("

            series.pk as serie_pk,
            series.series_iuid,
            series.series_no,
            series.series_desc,
            series.modality,
            CAST(DATE(series.updated_time) as CHAR) as series_updated_time,

            instance.pk as instance_pk,
            instance.inst_no,
            instance.sop_cuid,
            instance.sop_iuid,
            instance.inst_attrs")
        .from(" 
            study
            INNER JOIN patient ON patient.pk = study.patient_fk
            INNER JOIN series ON study.pk = series.study_fk
            INNER JOIN instance ON series.pk = instance.series_fk");

    params.add_query_conditions(&mut query_builder, settings);
    
    // -- Execute SQL and map to model
    let mut studies: Box<Vec<Study>> = Box::new(Vec::new());
    let mut rows = query_builder.build().fetch(pool);
    while let Some(row) = rows.try_next().await? {

        // -- Study
        let row_study_pk: i32 = row.try_get("study_pk")?;
        let study = match studies.iter_mut().find(|x|**x == row_study_pk) {
            Some(entry) => entry,
            None => {

                let patient_age = row.try_get::<Option<String>, &str>("pat_birthdate")?
                    .map(|bdate| calculate_age(bdate))
                    .transpose()?
                    .map(|age| age.to_string());

                let institution_name: Option<String> = row
                    .try_get::<Option<String>, &str>("institution_name")?
                    .and_then(|s| {
                        let trimmed = s.trim().to_string();
                        (!trimmed.is_empty()).then_some(trimmed)
                    })
                    .or_else(|| {
                        let bytes: Option<Vec<u8>> = row.try_get("study_attrs").ok()?;
                        let bytes = bytes?;

                        // Explicit Little Endian (Transfer syntax encoding we suppose dcm4chee uses in study_attrs)
                        let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.2.1").unwrap();

                        // Best-effort decode: if the blob is missing/invalid, skip the field.
                        let dcm = InMemDicomObject::read_dataset_with_ts(bytes.as_slice(), ts).ok()?;
                        let el = dcm.element_opt(Tag(0x0008, 0x0080)).ok()??;
                        el.to_str()
                            .ok()
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                    });

                studies.push(
                    Study {
                        study_pk:  row_study_pk,
                        study_iuid:  row.try_get("study_iuid")?,
                        study_date:  row.try_get::<&str,&str>("study_date")?
                                        .replace('-',"")
                                        .to_string(),
                        study_time:  row.try_get("study_time")?,
                        study_description: row.try_get("study_description")?,
                        patient_name: row.try_get("pat_name")?,
                        patient_id: row.try_get("pat_id")?,
                        accession_no:  row.try_get("accession_no")?,
                        patient_age,
                        patient_sex: row.try_get("pat_sex")?,
                        num_instances: row.try_get("num_instances")?,
                        modalities: row.try_get("mods_in_study")?,
                        institution_name,
                        series: vec![], 
                    });
                    studies.last_mut().unwrap()
            }
        };

        // -- Series
        let row_serie_pk: i32 = row.try_get("serie_pk")?;
        let serie = match study.series.iter_mut().find(|x|**x == row_serie_pk) {
            Some(entry) => entry,
            None => {
                study.series.push(
                    Serie {
                        serie_pk:  row_serie_pk,
                        series_iuid:  row.try_get("series_iuid")?,
                        series_no:   row.try_get::<&str,&str>("series_no")?
                                        .parse::<i32>()
                                        .context("Failed to parse series_no to integer")?,
                        modality:  row.try_get("modality")?,
                        series_description: row.try_get("series_desc")?,
                        instances: vec![],
                    });
                    study.series.last_mut().unwrap()
            }
        };

        // -- Instance
        
        let instance_sop_iuid: String = row.try_get("sop_iuid")?;
        let instance_pk: i32 = row.try_get("instance_pk")?;

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
            let buf: Vec<u8> = row.try_get("inst_attrs")?;

            // Explicit Little Endian (Transfer syntax encoding we suppose dcm4chee use to store data in ins_attrs database field)
            let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.2.1").unwrap(); 
            
            InMemDicomObject::read_dataset_with_ts(buf.as_slice(), ts)
                .context("Failed to read inst_attrs value from database")?
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

        // ---------------------------------------------------------------------------------------------------------------------------------------
        // OPTIONAL TAGS -------------------------------------------------------------------------------------------------------------------------
        /*
        let tag = DicomTag::PIXEL_REPRESENTATION;   // (0028,0103) US
        let pixel_representation: Option<u16> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value| value.to_int())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to u16"))?;

        let tag = DicomTag::SAMPLES_PER_PIXEL;      // (0028,0002) US
        let samples_per_pixel: Option<u16> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value| value.to_int())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to u16"))?;
        
        let tag = DicomTag::PIXEL_SPACING;          // (0028,0030) DS
        let pixel_spacing: Option<Vec<f64>> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value|value.to_multi_float64())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to Vec<f64>"))?;

        let tag = DicomTag::BITS_STORED;            // (0028,0101) US
        let bits_stored: Option<u16> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value| value.to_int())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to u16"))?;

        let tag = DicomTag::HIGH_BIT;               // (0028,0102) US
        let high_bit: Option<u16> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value| value.to_int())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to u16"))?;

        let tag = DicomTag::IMAGE_ORIENTATION_PATIENT; // (0020,0037) DS
        let image_orientation_patient: Option<Vec<f64>> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value| value.to_multi_float64())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to Vec<f64>"))?;

        let tag = DicomTag::IMAGE_POSITION_PATIENT; // (0020,0032) DS
        let image_position_patient: Option<Vec<f64>> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value| value.to_multi_float64())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to Vec<f64>"))?;
        
        let tag = DicomTag::FRAME_OF_REFERENCE_UID; // (0020,0052) UI
        let frame_of_reference_uid: Option<String> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value|value.to_str())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to String"))?
            .map(|s|s.to_string());

        let tag = DicomTag::IMAGE_TYPE;             // (0008,0008) CS
        let image_type: Option<Vec<String>> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value|value.to_multi_str())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to Vec<String>"))?
            .map(|s|s.to_vec());

        let tag = DicomTag::WINDOW_CENTER;          // (0028,1050) DS
        let window_center: Option<f64> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value| value.to_float64())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to f64"))?;

        let tag = DicomTag::WINDOW_WIDTH; // (0028,1051) DS
        let window_width: Option<f64> = get_dicom_element(tag, filepath.clone(), &db_dicomattrs, &mut file_dicomattrs, instance_pk,pool, settings)
            .await?
            .map(|value| value.to_float64())
            .transpose()
            .context(format!("Failed to parse DicomTag {tag} value to f64"))?;
        */

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
                    instance_no: row.try_get::<String,&str>("inst_no")?
                                    .parse::<i32>()
                                    .context("Failed to parse Instance number to integer")?,
                    instance_sop_cuid: row.try_get("sop_cuid")?,
                    series_modality: serie.modality.clone(),
                    instance_sop_iuid,
                    series_iuid: serie.series_iuid.clone(),
                    study_iuid: study.study_iuid.clone(),
                    series_date: row.try_get::<&str,&str>("series_updated_time")?
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

