use std::vec;
use sqlx::{Error, MySqlPool, Row};
use futures::TryStreamExt;
use anyhow::{Result, Context};

use dicom_core::Tag;
use dicom_encoding::TransferSyntaxIndex;
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::TransferSyntaxRegistry;

use crate::{settings::Settings, 
            models::cornerstone::{Studies, Patient, Study, Serie, Instance},
            constants::{SOP_CLASS_SINGLEFRAME, SOP_CLASS_MULTIFRAME},
            api::study_token::params::StudyTokenParams, database::QueryBuilder};


pub async fn get_studies(pool: &MySqlPool, params: &StudyTokenParams, settings: &Settings) -> Result<Studies> {
    
    // -- Build SQL query 
    let mut query_builder = QueryBuilder::new();
    query_builder
        .select("
            patient.pk as pat_pk,
            patient.pat_id,
            patient.pat_name,
            CAST(patient.pat_birthdate as CHAR) as pat_birthdate,
            patient.pat_sex,
            study.pk as study_pk,
            study.study_iuid,
            CAST(DATE(study.study_datetime) as CHAR) as study_date,
            CAST(TIME(study.study_datetime) as CHAR) as study_time,
            study.accession_no,
            study.study_desc,
            study.mods_in_study,
            study.study_custom3 as physicians_reading,
            study.ref_physician,
            study.study_id,
            series.pk as serie_pk,
            series.series_iuid,
            series.series_no,
            series.station_name,
            series.series_desc,
            series.modality,
            series.perf_physician,
            series.num_instances,
            instance.pk as instance_pk,
            instance.sop_iuid,
            instance.inst_no,
            instance.sop_cuid")
        .from("
            study
            INNER JOIN patient ON patient.pk = study.patient_fk
            INNER JOIN series ON study.pk = series.study_fk
            INNER JOIN instance ON series.pk = instance.series_fk");

    params.add_query_conditions(&mut query_builder, settings);

    match &settings.dicomarchive.number_frames_field {
        Some(value) => query_builder.select(format!("instance.{},",value)),
        None => query_builder.select("instance.inst_attrs"),
    };

    // -- Execute SQL and map to model
    let mut patients: Box<Vec<Patient>> = Box::new(Vec::new());
    let mut rows = query_builder.build().fetch(pool);
    while let Some(row) = rows.try_next().await? {

        // -- Patient
        let row_pat_pk: i32 = row.try_get("pat_pk")?;
        let patient = match patients.iter_mut().find(|x|**x == row_pat_pk) {
            Some(entry) => entry,
            None => {
                patients.push(
                    Patient { 
                        pat_pk:  row_pat_pk,
                        pat_id:  row.try_get("pat_id")?,
                        pat_name:  row.try_get("pat_name")?,
                        pat_birthdate:  row.try_get("pat_birthdate")?,
                        pat_sex:  row.try_get("pat_sex")?,
                        studies: vec![],
                    });
                patients.last_mut().unwrap()
            }
        };

        // -- Study
        let row_study_pk: i32 = row.try_get("study_pk")?;
        let study = match patient.studies.iter_mut().find(|x|**x == row_study_pk) {
            Some(entry) => entry,
            None => {
                patient.studies.push(
                    Study {
                        study_pk:  row.try_get("study_pk")?,
                        study_iuid:  row.try_get("study_iuid")?,
                        study_date:  row.try_get("study_date")?,
                        study_time:  row.try_get("study_time")?,
                        accession_no:  row.try_get("accession_no")?,
                        study_desc:  row.try_get("study_desc")?,
                        mods_in_study:  row.try_get("mods_in_study")?,
                        institution:  match row.try_get::<Option<String>,&str>("institution") {
                            Ok(value) => Ok(value),
                            Err(Error::ColumnNotFound(_)) => Ok(None),
                            Err(err) =>  Err(err),
                        }?,
                        physicians_reading:  row.try_get("physicians_reading")?,
                        ref_physician:  row.try_get("ref_physician")?,
                        study_id:  row.try_get("study_id")?,
                        series: vec![],
                    });
                    patient.studies.last_mut().unwrap()
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
                        series_no:  row.try_get("series_no")?,
                        station_name:  row.try_get("station_name")?,
                        series_desc:  row.try_get("series_desc")?,
                        modality:  row.try_get("modality")?,
                        perf_physician:  row.try_get("perf_physician")?,
                        num_instances:  row.try_get("num_instances")?,
                        instances: vec![],
                    });
                    study.series.last_mut().unwrap()
            }
        };

        // -- Instance

        /*
        Number of Frames
        In relation to Cornerstone, the number of frames is also important, for Weasis this inforamtion is not relevant
        An instance can be no-frame, single-frame o multi-frame based.
        The number of frames is not available in non multiframe objects. Those shall have the value `0` if they are not-frame based and `1` if they are always single-frame.
        
        In the case of multi-frame SOP Classes:
        Number of frames may be stored in some database field in `blob` or `varchar` data type. By default dcm4chee-2.18.3 store this information in `instance.inst_attr` field in `blob` data type.
        If another field is specified in Sirius HIP configuration file (num_frame_field) is expected to have `varchar` data type

        So, we reserve the value `-1` to state that the info is not available at all in the DB.
        As seen, some cases can be resolved before any database data processing, based on the SOP Class already obtained for series filters
        - If SOP Class correspond to a non-frame based object, the number of frames is forced to `0`
        - If corresponds to a single-frame object, the number of frames is forced to `1`
        - If corresponds to an enhanced SOP Class potentially containing multiframes.
        */

        // Get Series SOP Class
        let sop_cuid =  row.try_get("sop_cuid")?;

        let num_frames = match &settings.dicomarchive.number_frames_field {
            // Get number of frames from custom field (varchar) , set -1 if not found or error in data type conversion
            Some(field) => 
                row.try_get::<i32, &str>(field.as_ref()).unwrap_or(-1),    

            // Get number of frames from inst_attr field (blob)
            None => {
                match SOP_CLASS_SINGLEFRAME.iter().find(|&&x| x == sop_cuid ) {
                    // If SOP CLASS is single-frame
                    Some(_) => 1,

                    None => {
                        match SOP_CLASS_MULTIFRAME.iter().find(|&&x| x == sop_cuid ) {
                            // If SOP CLASS is multi-frame
                            Some(_) => {
                                
                                let buf: Vec<u8> = row.try_get("inst_attrs")?;

                                // Explicit Little Endian (Transfer syntax encoding we suppose dcm4chee use to store data in ins_attrs database field)
                                let ts = TransferSyntaxRegistry.get("1.2.840.10008.1.2.1").unwrap(); 
                                let obj = InMemDicomObject::read_dataset_with_ts(buf.as_slice(), ts)
                                    .context("Failed to read inst_attrs value from database")?;
                                
                                match obj.element_opt(Tag(0x0028, 0x0008))
                                    .context("Failed to get Dicom Tag (0028,0008) from inst_attrs)")? 
                                    
                                {
                                    Some(elem) => {
                                        log::debug!("{:?}",elem);
                                        elem.to_int()
                                            .context("Failed to convert Dicom Tag (0028,0008) to int")?
                                    }

                                    // If not found Tag(0028,0008) asume there is only one frame
                                    None => 1,
                                }
                            },
                            // If SOP CLASS not-based frame
                            None => 0,
                        }
                    }
                }
            }
        };

        serie.instances.push(
            Instance {
                instance_pk:  row.try_get("instance_pk")?,
                sop_iuid:  row.try_get("sop_iuid")?,
                inst_no:  row.try_get("inst_no")?,
                sop_cuid,
                num_frames,
        });
    }

    Ok(Studies { inner: patients })


}