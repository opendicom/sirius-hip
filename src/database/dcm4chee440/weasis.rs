use std::vec;
use sqlx::{MySqlPool, Row};
use futures::TryStreamExt;
use anyhow::{Result as AHResult, Ok,};

use crate::{settings::Settings, 
            models::weasis::{Studies, Patient, Study, Serie, Instance},
            api::study_token::params::StudyTokenParams, 
            database::QueryBuilder};


pub async fn get_studies(pool: &MySqlPool, params: &StudyTokenParams, settings: &Settings) -> AHResult<Studies> {

        // -- Build query -------------------------------------------------------------------------------
        let mut query_builder = QueryBuilder::new();
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
    
                series.pk as serie_pk,
                series.series_iuid,
                series.series_no,
                series.series_desc,
                series.modality,
                CAST(DATE(series.updated_time) as CHAR) as series_updated_time,
    
                instance.pk as instance_pk,
                instance.sop_iuid,
                instance.sop_cuid,
                instance.inst_no,
    
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
    
    // -- Fetch from database ------------------------------------------------------------------- //   
    let mut patients: Box<Vec<Patient>> = Box::new(Vec::new());
    let mut rows = query_builder.build().fetch(pool);           
    while let Some(row) = rows.try_next().await? {

        let row_pat_pk:i32 = row.try_get("patient_pk")?;
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
                        pat_birthdate:  row.try_get("patient_bdate")?,
                        pat_sex:  row.try_get("patient_sex")?,
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

                let ref_phys_fname: Option<String> = row.try_get("study_ref_phys_fname")?;
                let ref_phys_gname: Option<String> = row.try_get("study_ref_phys_gname")?;
                let ref_phys_mname: Option<String> = row.try_get("study_ref_phys_mname")?;

                let study_ref_phys_name = if ref_phys_fname.is_some() || ref_phys_gname.is_some() || ref_phys_mname.is_some() {
                    let mut name = String::new();
                    if let Some(value) = row.try_get("study_ref_phys_fname")? {
                        name.push_str(value);
                    }
                    if let Some(value) = row.try_get("study_ref_phys_gname")? {
                        name.push('^');
                        name.push_str(value);
                    }
                    if let Some(value) = row.try_get("study_ref_phys_mname")? {
                        name.push(' ');
                        name.push_str(value);
                    }
                    Some(name)
                } else {
                    None
                };
                
                patient.studies.push(
                    Study {
                        study_pk:  row.try_get("study_pk")?,
                        study_iuid:  row.try_get("study_iuid")?,
                        study_date:  row.try_get("study_date")?,
                        study_time:  row.try_get("study_time")?,
                        accession_no:  row.try_get("accession_no")?,
                        study_desc:  row.try_get("study_desc")?,
                        ref_physician:  study_ref_phys_name,
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
                        series_desc:  row.try_get("series_desc")?,
                        modality:  row.try_get("modality")?,
                        instances: vec![],
                    });
                    study.series.last_mut().unwrap()
            }
        };

        serie.instances.push(
            Instance {
                instance_pk:  row.try_get("instance_pk")?,
                sop_iuid:  row.try_get("sop_iuid")?,
                inst_no:  row.try_get("inst_no")?,
        });
    }

    Ok(Studies { inner: patients })

}
