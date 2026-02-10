
// --------------------------------------------------------------------------- //
// -- WEASIS - dcm4chee2183 module 
// --------------------------------------------------------------------------- //

use std::vec;

use sqlx::{MySqlPool, Row};
use futures::TryStreamExt;
use anyhow::{Result, Ok};

use crate::{settings::Settings, 
            models::weasis::{Studies, Patient, Study, Serie, Instance},
            api::study_token::params::StudyTokenParams,
            database::QueryBuilder};


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
            study.ref_physician,
            study.study_id,
            series.pk as serie_pk,
            series.series_iuid,
            series.series_no,
            series.series_desc,
            series.modality,
            instance.pk as instance_pk,
            instance.sop_iuid,
            instance.inst_no")
        .from(" 
            study
            INNER JOIN patient ON patient.pk = study.patient_fk
            INNER JOIN series ON study.pk = series.study_fk
            INNER JOIN instance ON series.pk = instance.series_fk");
    
    params.add_query_conditions(&mut query_builder, settings);

    
    // -- Fetch query and map to model
    let mut rows = query_builder.build().fetch(pool);
        
    let mut patients: Box<Vec<Patient>> = Box::new(Vec::new());
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