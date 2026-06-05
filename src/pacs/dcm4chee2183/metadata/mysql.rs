


use async_trait::async_trait;
use sqlx::{MySql, MySqlPool, QueryBuilder};

use crate::{
    features::study_token::{entities::Study, StudySearchCriteria},
    pacs::{
        Instance,
        InstanceSearchCriteria,
        MetadataProvider,
        Series,
        SeriesSearchCriteria,
    },
};


const SEARCH_LIMIT: i64 = 2000;


#[derive(Debug, sqlx::FromRow)]
struct StudyRow {
    study_uid: String,
    patient_id: Option<String>,
    patient_name: Option<String>,
    accession_number: Option<String>,
}


#[derive(Debug, sqlx::FromRow)]
struct SeriesRow {
    study_uid: String,
    series_uid: String,
    modality: Option<String>,
    description: Option<String>,
}


#[derive(Debug, sqlx::FromRow)]
struct InstanceRow {
    study_uid: String,
    series_uid: String,
    sop_instance_uid: String,
    sop_class_uid: Option<String>,
    instance_number: Option<String>,
    relative_file_path: Option<String>,
    filesystem_id: Option<i64>,
}


pub struct Dcm4chee2183MysqlMetadataProvider {
    pool: MySqlPool,
}

impl Dcm4chee2183MysqlMetadataProvider {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}


#[async_trait]
impl MetadataProvider for Dcm4chee2183MysqlMetadataProvider {
    async fn search_studies(&self, criteria: &StudySearchCriteria) -> anyhow::Result<Vec<Study>> {
        let mut qb = QueryBuilder::<MySql>::new(
            "SELECT \
                st.study_iuid AS study_uid, \
                pt.pat_id AS patient_id, \
                pt.pat_name AS patient_name, \
                st.accession_no AS accession_number \
             FROM study st \
             LEFT JOIN patient pt ON pt.pk = st.patient_fk \
             WHERE 1=1",
        );

        if let Some(patient_id) = criteria.patient_id.as_deref() {
            qb.push(" AND pt.pat_id = ").push_bind(patient_id);
        }

        if let Some(accession_number) = criteria.accession_number.as_deref() {
            qb.push(" AND st.accession_no = ").push_bind(accession_number);
        }

        qb.push(" ORDER BY st.study_datetime DESC LIMIT ").push_bind(SEARCH_LIMIT);

        let rows = qb
            .build_query_as::<StudyRow>()
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| Study {
                study_uid: row.study_uid,
                patient_id: row.patient_id.unwrap_or_default(),
                patient_name: row.patient_name.unwrap_or_default(),
                accession_number: row.accession_number,
            })
            .collect())
    }

    async fn search_series(&self, criteria: &SeriesSearchCriteria) -> anyhow::Result<Vec<Series>> {
        let mut qb = QueryBuilder::<MySql>::new(
            "SELECT \
                st.study_iuid AS study_uid, \
                se.series_iuid AS series_uid, \
                se.modality AS modality, \
                se.series_desc AS description \
             FROM series se \
             INNER JOIN study st ON st.pk = se.study_fk \
             LEFT JOIN patient pt ON pt.pk = st.patient_fk \
             WHERE 1=1",
        );

        if let Some(study_uid) = criteria.study_uid.as_deref() {
            qb.push(" AND st.study_iuid = ").push_bind(study_uid);
        }

        if let Some(patient_id) = criteria.patient_id.as_deref() {
            qb.push(" AND pt.pat_id = ").push_bind(patient_id);
        }

        if let Some(modality) = criteria.modality.as_deref() {
            qb.push(" AND se.modality = ").push_bind(modality);
        }

        qb.push(" ORDER BY st.study_datetime DESC, se.series_no ASC LIMIT ")
            .push_bind(SEARCH_LIMIT);

        let rows = qb
            .build_query_as::<SeriesRow>()
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| Series {
                study_uid: row.study_uid,
                series_uid: row.series_uid,
                modality: row.modality,
                description: row.description,
            })
            .collect())
    }

    async fn search_instances(
        &self,
        criteria: &InstanceSearchCriteria,
    ) -> anyhow::Result<Vec<Instance>> {
        let mut qb = QueryBuilder::<MySql>::new(
            "SELECT \
                st.study_iuid AS study_uid, \
                se.series_iuid AS series_uid, \
                ins.sop_iuid AS sop_instance_uid, \
                ins.sop_cuid AS sop_class_uid, \
                ins.inst_no AS instance_number, \
                fi.filepath AS relative_file_path, \
                CAST(fi.filesystem_fk AS SIGNED) AS filesystem_id \
             FROM instance ins \
             INNER JOIN series se ON se.pk = ins.series_fk \
             INNER JOIN study st ON st.pk = se.study_fk \
             LEFT JOIN files fi \
                ON fi.instance_fk = ins.pk \
                AND fi.pk = (SELECT MAX(f2.pk) FROM files f2 WHERE f2.instance_fk = ins.pk) \
             WHERE 1=1",
        );

        if let Some(study_uid) = criteria.study_uid.as_deref() {
            qb.push(" AND st.study_iuid = ").push_bind(study_uid);
        }

        if let Some(series_uid) = criteria.series_uid.as_deref() {
            qb.push(" AND se.series_iuid = ").push_bind(series_uid);
        }

        if let Some(sop_instance_uid) = criteria.sop_instance_uid.as_deref() {
            qb.push(" AND ins.sop_iuid = ").push_bind(sop_instance_uid);
        }

        qb.push(" ORDER BY st.study_datetime DESC, se.series_no ASC, ins.inst_no ASC LIMIT ")
            .push_bind(SEARCH_LIMIT);

        let rows = qb
            .build_query_as::<InstanceRow>()
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| Instance {
                study_uid: row.study_uid,
                series_uid: row.series_uid,
                sop_instance_uid: row.sop_instance_uid,
                sop_class_uid: row.sop_class_uid,
                instance_number: row.instance_number,
                relative_file_path: row.relative_file_path,
                filesystem_id: row
                    .filesystem_id
                    .and_then(|filesystem_id| i32::try_from(filesystem_id).ok()),
            })
            .collect())
    }

}