use chrono::NaiveDateTime;
use sqlx::{MySqlPool, Row};
use futures::TryStreamExt;
use anyhow::{Result, Ok, anyhow};

use crate::{settings::Settings, 
            models::dicomzip::{Studies, Patient, Study, Serie, Instance}, 
            api::study_token::params::StudyTokenParams,
            database::QueryBuilder};

const UPDATED_DIFF: i64 = 180;

pub async fn get_studies(pool: &MySqlPool, params: &StudyTokenParams, settings: &Settings) -> Result<Studies> {
    
    // -- Build SQL query 
    let mut query_builder = QueryBuilder::new();
    query_builder
        .select("
            study.pk as study_pk,
            study.study_iuid,
            study.study_desc,
            study.created_time as study_created_time,
            study.updated_time as study_updated_time,

            series.pk as serie_pk,
            series.series_iuid,
            series.series_desc,
            series.created_time as series_created_time,
            series.updated_time as series_updated_time,

            instance.pk as instance_pk,
            instance.sop_iuid as instance_sop_iuid,
            instance.created_time as instance_created_time,
            instance.updated_time as instance_updated_time,

            patient.pk as patient_pk,
            P.family_name as patient_fname,
            P.given_name as patient_gname,
            P.middle_name as patient_mname,
            patient.created_time as patient_created_time,
            patient.updated_time as patient_updated_time,

            file_ref.filepath,
            file_ref.filesystem_fk")
    .from("
        study
        INNER JOIN series ON study.pk = series.study_fk
        INNER JOIN instance ON series.pk = instance.series_fk
        INNER JOIN patient ON patient.pk = study.patient_fk
        INNER JOIN person_name AS P ON P.pk = patient.pat_name_fk
        INNER JOIN file_ref ON instance.pk = file_ref.instance_fk");

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
            return Ok(Studies { inner: Box::new(vec![]) });
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
        .condition_opt("study.mods_in_study = ?", params.modality_in_study.as_ref())
        .condition_opt("study.cuids_in_study = ?", params.cuids_in_study.as_ref())
        .condition_opt("series.series_iuid = ?", params.series_instance_uid.as_ref())
        .condition_list_opt("series.series_iuid IN ", params.series_instance_uid.as_ref(), '\\')
        .condition_opt("series.modality = ?", params.modality.as_ref())
        .condition_opt("series.sop_cuid = ?", params.sop_class.as_ref())
        .condition_opt("series.sop_class != ?", params.sop_class_off.as_ref())
        .condition_list_opt("series.modality NOT IN ", params.modality_off.as_ref(), '\\')
        .order_by("study.pk")
        .limit(params.max.unwrap_or(settings.max_default));



    // -- Execute SQL and map to model
    let mut patients: Box<Vec<Patient>> = Box::new(Vec::new());
    let mut rows = query_builder.build().fetch(pool);
    while let Some(row) = rows.try_next().await? {
        let mut retrive_wado = false;

        // -- Patient
        let row_pat_pk: i32 = row.try_get("patient_pk")?;
        let pat_created_time: NaiveDateTime = row.try_get("patient_created_time")?;
        let pat_updated_time: NaiveDateTime = row.try_get("patient_updated_time")?;
        if (pat_updated_time - pat_created_time).num_seconds() > UPDATED_DIFF {
            retrive_wado = true
        }

        let patient = match patients.iter_mut().find(|x|**x == row_pat_pk) {
            Some(entry) => entry,
            None => {

                let pat_id = match &params.patient_id {
                    Some(value) => value.into(),
                    None => {
                        sqlx::query!("
                            SELECT patient_id.pat_id 
                            FROM patient_id 
                            WHERE patient_fk = ? LIMIT 1 ",
                            row_pat_pk
                        ).fetch_one(pool)
                        .await
                        .map(|x|x.pat_id)?
                    }
                };
        
                let mut pat_name = String::new();
                if let Some(value) = row.try_get("patient_fname")? {
                    pat_name.push_str(value);
                }
                if let Some(value) = row.try_get("patient_gname")? {
                    pat_name.push('^');
                    pat_name.push_str(value);
                }
                if let Some(value) = row.try_get("patient_mname")? {
                    pat_name.push(' ');
                    pat_name.push_str(value);
                }

                patients.push(
                    Patient { 
                        pat_pk:  row_pat_pk,
                        pat_id,
                        pat_name,
                        studies: vec![],
                    });
                patients.last_mut().unwrap()
            }
        };

        // -- Study
        let row_study_pk: i32 = row.try_get("study_pk")?;
        let study_created_time: NaiveDateTime = row.try_get("study_created_time")?;
        let study_updated_time: NaiveDateTime = row.try_get("study_updated_time")?;
        if (study_updated_time - study_created_time).num_seconds() > UPDATED_DIFF {
            retrive_wado = true
        }
        let study = match patient.studies.iter_mut().find(|x|**x == row_study_pk) {
            Some(entry) => entry,
            None => {
                patient.studies.push(
                    Study {
                        study_pk:  row.try_get("study_pk")?,
                        study_iuid:  row.try_get("study_iuid")?,
                        study_desc:  row.try_get("study_desc")?,
                        series: vec![],
                    });
                    patient.studies.last_mut().unwrap()
            }
        };

        // -- Series
        let row_serie_pk: i32 = row.try_get("serie_pk")?;
        let series_created_time: NaiveDateTime = row.try_get("series_created_time")?;
        let series_updated_time: NaiveDateTime = row.try_get("series_updated_time")?;
        if (series_updated_time - series_created_time).num_seconds() > UPDATED_DIFF {
            retrive_wado = true
        }
        let serie = match study.series.iter_mut().find(|x|**x == row_serie_pk) {
            Some(entry) => entry,
            None => {
                study.series.push(
                    Serie {
                        serie_pk:  row_serie_pk,
                        series_iuid:  row.try_get("series_iuid")?,
                        series_desc:  row.try_get("series_desc")?,
                        instances: vec![],
                    });
                    study.series.last_mut().unwrap()
            }
        };

        // -- Instance

        /* retrive_url
        Dcm4chee store dicom files received from the image acquisition equipment in it storages. They are configured in dcm4chee 
        with a directory path indicating where it's mounted in the filesystem of the dcm4chee server. 
        
        Another feature of dcm4chee is that when a data correction is made, it is only applied on the database, not in the dicom files. 
        So when delivering the files, it makes a merge between the database data and the original dicom files.

        For better performance Sirius HIP can access files directly from the filesystem or via wado if a data correction was made.
        To detect a data correction we check the updated and created timestamps in the database
        To access files directly from the storages Sirius HIP must be configured with the mappings of all the storages in dcm4chee.
        */
        
        let instance_created_time: NaiveDateTime = row.try_get("instance_created_time")?;
        let instance_updated_time: NaiveDateTime = row.try_get("instance_updated_time")?;
        let instance_sop_iuid = row.try_get("instance_sop_iuid")?;
        if (instance_updated_time - instance_created_time).num_seconds() > UPDATED_DIFF {
            retrive_wado = true
        }

        // Get image from wado
        let retrieve_url = if retrive_wado {
            format!("{}?requestType=WADO&studyUID={}&seriesUID={}&objectUID={}&contentType=application/dicom&transferSyntax={}",
                settings.dicomarchive.wadouri,
                study.study_iuid, 
                serie.series_iuid, 
                instance_sop_iuid, 
                settings.dicomarchive.transfer_syntax)

        // -- Get image from filesystem
        } else {
            let fs_id: i32 = row.try_get("filesystem_fk")?;
            let base = settings.dicomarchive.get_fs_path_by_id(fs_id)
                .ok_or(anyhow!("Not found mapping for dcm4chee filesystem id: `{}`",fs_id))?;

            format!("file://{}/{}",base,row.try_get::<String, &str>("filepath")?)
        };

        serie.instances.push(
            Instance {
                instance_pk:  row.try_get("instance_pk")?,
                sop_iuid:  instance_sop_iuid,
                retrieve_url,
        });
    }

 Ok(Studies { inner: patients })

}