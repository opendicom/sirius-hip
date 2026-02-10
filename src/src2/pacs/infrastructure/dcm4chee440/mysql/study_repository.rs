use async_trait::async_trait;
use sqlx::{MySql, MySqlPool, QueryBuilder};

use crate::src2::errors::PacsError;
use crate::src2::pacs::read_models::QidoStudyRow;
use crate::src2::pacs::read_models::StudyTokenRow;
use crate::src2::pacs::infrastructure::mysql_sql_helpers::{dataset_sources, override_col, override_or_default, qualified_ident_expr};
use crate::src2::pacs::repositories::StudyRepository;
use crate::src2::pacs::repositories::study_repository::{
    QidoStudiesIncludeFields, QidoStudiesQuery, StudyTokenQuery,
};

pub struct Dcm4chee440MySqlStudyRepository {
    pool: MySqlPool,
}

impl Dcm4chee440MySqlStudyRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StudyRepository for Dcm4chee440MySqlStudyRepository {
    async fn fetch_study_token_rows(
        &self,
        query: StudyTokenQuery<'_>,
        include_filesystem: bool,
        include_wado: bool,
    ) -> Result<Vec<StudyTokenRow>, PacsError> {
        fn split_backslash(value: &str) -> Vec<&str> {
            value.split('\\').filter(|s| !s.is_empty()).collect()
        }

        fn sanitize_yyyymmdd_maybe(value: &str) -> String {
            value.replace('-', "")
        }

        let use_filesystem_expr = "(
            ABS(TIMESTAMPDIFF(SECOND, study.created_time, study.updated_time)) <= 600 AND 
            ABS(TIMESTAMPDIFF(SECOND, series.created_time, series.updated_time)) <= 600 AND 
            ABS(TIMESTAMPDIFF(SECOND, instance.created_time, instance.updated_time)) <= 600
        )";

        let mut qb = QueryBuilder::<MySql>::new("SELECT ");

        let overrides = query.metadata_overrides;
        let patient_name_select = override_or_default(
            overrides,
            "PatientName",
            "CONCAT_WS('^', person_name.family_name, person_name.given_name, person_name.middle_name)",
        );
        let patient_id_select = override_or_default(
            overrides,
            "PatientID",
            "(SELECT pid.pat_id FROM patient_id pid WHERE pid.patient_fk = patient.pk ORDER BY pid.pk LIMIT 1)",
        );
        let patient_sex_select = override_or_default(overrides, "PatientSex", "patient.pat_sex");
        let patient_birthdate_select = override_or_default(overrides, "PatientBirthDate", "patient.pat_birthdate");
        let accession_no_select = override_or_default(overrides, "AccessionNumber", "study.accession_no");
        let modalities_select = override_or_default(overrides, "ModalitiesInStudy", "study.mods_in_study");
        let study_description_select = override_or_default(overrides, "StudyDescription", "study.study_desc");

        // InstitutionName (0008,0080) at study level:
        // - Default comes from decoding the study dataset (`study_dicomattrs.attrs`).
        // - Optionally it can be overridden as a direct column value (non-dataset), e.g. `study.study_custom1`.
        let institution_name_select = override_col(overrides, "InstitutionName").unwrap_or_else(|| "NULL".to_string());

        let ds_sources = dataset_sources(overrides);

        // Dataset overrides (`dataset=true`) are delivered to the presenter by selecting up to 4
        // extra dataset blobs into the row read-model columns `ov_ds1..ov_ds4`.
        //
        // `dataset_sources(overrides)` returns the distinct set of override `source` strings
        // (each is `table.column`) and sorts them to make the mapping deterministic.
        // The positional slot mapping is:
        // - ov_ds1 -> ds_sources[0]
        // - ov_ds2 -> ds_sources[1]
        // - ov_ds3 -> ds_sources[2]
        // - ov_ds4 -> ds_sources[3]
        //
        // The OHIF presenter recomputes the same `ds_sources` list and uses it to map
        // a configured override `source` back to the correct `ov_dsN` bytes.

        if include_wado {
            qb.push(format!(
                "{} AS patient_name,
                 {} AS patient_id,
                 {} AS patient_sex,
                 {} AS patient_birthdate,
                 study.study_date AS study_date,
                 study.study_time AS study_time,
                 {} AS study_description,
                 {} AS accession_no,
                 study.num_instances1 AS num_instances,
                 {} AS modalities,
                 {} AS institution_name,
                 study_dicomattrs.attrs AS study_attrs,",
                patient_name_select,
                patient_id_select,
                patient_sex_select,
                patient_birthdate_select,
                study_description_select,
                accession_no_select,
                modalities_select,
                institution_name_select,
            ));
        } else {
            qb.push(
                "NULL AS patient_name,
                 NULL AS patient_id,
                 NULL AS patient_sex,
                 NULL AS patient_birthdate,
                 NULL AS study_date,
                 NULL AS study_time,
                 NULL AS study_description,
                 NULL AS accession_no,
                 NULL AS num_instances,
                 NULL AS modalities,
                 NULL AS institution_name,
                 NULL AS study_attrs,",
            );
        }

        qb.push(
            "study.study_iuid AS study_instance_uid,
             series.series_iuid AS series_instance_uid,
             instance.sop_iuid AS sop_instance_uid,",
        );

        if include_wado {
            qb.push(
                "series.series_no AS series_no,
                 series.series_desc AS series_description,
                 series.modality AS modality,
                 CAST(DATE(series.updated_time) AS CHAR) AS series_updated_time,
                 CAST(instance.pk AS SIGNED) AS instance_pk,
                 instance.inst_no AS inst_no,
                 instance.sop_cuid AS sop_cuid,
                 dicomattrs.attrs AS inst_attrs,",
            );

            // Select dataset override blobs into fixed slots.
            // Note: `idx` is 0-based, while `slot` is 1..=4.
            for (idx, slot) in (1usize..=4).enumerate() {
                let expr = ds_sources
                    .get(idx)
                    .and_then(|s| qualified_ident_expr(s))
                    .unwrap_or_else(|| "NULL".to_string());
                qb.push(format!("{} AS ov_ds{},", expr, slot));
            }
        } else {
            qb.push(
                "NULL AS series_no,
                 NULL AS series_description,
                 NULL AS modality,
                 NULL AS series_updated_time,
                 NULL AS instance_pk,
                 NULL AS inst_no,
                 NULL AS sop_cuid,
                 NULL AS inst_attrs,
                 NULL AS ov_ds1,
                 NULL AS ov_ds2,
                 NULL AS ov_ds3,
                 NULL AS ov_ds4,",
            );
        }

        if include_filesystem {
            qb.push("IF(");
            qb.push(use_filesystem_expr);
            qb.push(", file_ref.filepath, NULL) AS relative_file_path,");

            qb.push("IF(");
            qb.push(use_filesystem_expr);
            qb.push(", CAST(file_ref.filesystem_fk AS SIGNED), NULL) AS filesystem_fk,");
        } else {
            qb.push("NULL AS relative_file_path, NULL AS filesystem_fk,");
        }

        qb.push("CASE WHEN ");
        qb.push(use_filesystem_expr);
        qb.push(" THEN 1 ELSE 0 END AS use_filesystem ");

        qb.push(
            "FROM `study`
             INNER JOIN `patient` ON patient.pk = study.patient_fk
             LEFT JOIN `person_name` ON person_name.pk = patient.pat_name_fk
             INNER JOIN `series` ON series.study_fk = study.pk
             INNER JOIN `instance` ON instance.series_fk = series.pk
             INNER JOIN `file_ref` ON file_ref.instance_fk = instance.pk
             LEFT JOIN `dicomattrs` ON dicomattrs.pk = instance.dicomattrs_fk
             LEFT JOIN `dicomattrs` study_dicomattrs ON study_dicomattrs.pk = study.dicomattrs_fk
             WHERE 1=1",
        );

        // ------------------------------------------------------------
        // Dynamic filters based on StudyTokenQuery
        // ------------------------------------------------------------

        // Institution filter
        if let Some(institution) = query.institution {
            qb.push(" AND series.institution = ").push_bind(institution);
        }

        // Patient filters
        if let Some(patient_id) = query.patient_id {
            if let Some(col) = override_col(overrides, "PatientID") {
                qb.push(" AND ").push(col).push(" = ").push_bind(patient_id);
            } else {
                qb.push(" AND EXISTS (SELECT 1 FROM patient_id pid WHERE pid.patient_fk = patient.pk AND pid.pat_id = ")
                    .push_bind(patient_id)
                    .push(")");
            }
        }
        if let Some(patient_regex) = query.patient_fullname {
            let patient_name_where = override_or_default(
                overrides,
                "PatientName",
                "CONCAT_WS(' ', person_name.family_name, person_name.given_name, person_name.middle_name)",
            );
            qb.push(" AND ").push(patient_name_where).push(" REGEXP ").push_bind(patient_regex);
        }

        // StudyInstanceUID: one-or-more separated by '\\'
        if let Some(study_uids) = query.study_instance_uid {
            let values = split_backslash(study_uids);
            if !values.is_empty() {
                qb.push(" AND study.study_iuid IN (");
                let mut separated = qb.separated(", ");
                for v in values {
                    separated.push_bind(v);
                }
                separated.push_unseparated(")");
            }
        }

        // Accession number
        if let Some(accession) = query.accession_number {
            if let Some(col) = override_col(overrides, "AccessionNumber") {
                qb.push(" AND ").push(col).push(" = ").push_bind(accession);
            } else {
                qb.push(" AND study.accession_no = ").push_bind(accession);
            }
        }

        // Study ID (LIKE)
        if let Some(study_id) = query.study_id {
            qb.push(" AND study.study_id LIKE ")
                .push_bind(format!("%{}%", study_id));
        }

        // Study date filter on s.study_date (string YYYYMMDD in dcm4chee 4.4)
        if let Some(study_date) = query.study_date {
            let parts = study_date.split('|').collect::<Vec<_>>();
            match parts.as_slice() {
                // "YYYY-MM-DD" (no pipe present)
                [single] if !study_date.contains('|') => {
                    qb.push(" AND study.study_date = ")
                        .push_bind(sanitize_yyyymmdd_maybe(single));
                }
                // "YYYY-MM-DD|" (>=)
                [start, ""] => {
                    qb.push(" AND study.study_date >= ")
                        .push_bind(sanitize_yyyymmdd_maybe(start));
                }
                // "|YYYY-MM-DD" (<=)
                ["", end] => {
                    qb.push(" AND study.study_date <= ")
                        .push_bind(sanitize_yyyymmdd_maybe(end));
                }
                // "YYYY-MM-DD|YYYY-MM-DD" (between)
                [start, end] => {
                    qb.push(" AND study.study_date BETWEEN ")
                        .push_bind(sanitize_yyyymmdd_maybe(start))
                        .push(" AND ")
                        .push_bind(sanitize_yyyymmdd_maybe(end));
                }
                _ => {
                    // Malformed; ignore at DB level.
                }
            }
        }

        // ModalityInStudy (study.mods_in_study contains values separated by '\\')
        if let Some(mod_in_study) = query.modality_in_study {
            let mods_col = override_or_default(overrides, "ModalitiesInStudy", "study.mods_in_study");
            qb.push(" AND INSTR(CONCAT(CHAR(92), IFNULL(")
                .push(mods_col)
                .push(", ''), CHAR(92)), CONCAT(CHAR(92), ")
                .push_bind(mod_in_study)
                .push(", CHAR(92))) > 0");
        }

        // CUIDsInStudy (study.cuids_in_study contains values separated by '\\')
        if let Some(cuids) = query.cuids_in_study {
            let values = split_backslash(cuids);
            if !values.is_empty() {
                qb.push(" AND (");
                let mut or_sep = qb.separated(" OR ");
                for v in values {
                    or_sep
                        .push(" INSTR(CONCAT(CHAR(92), IFNULL(study.cuids_in_study, ''), CHAR(92)), CONCAT(CHAR(92), ")
                        .push_bind(v)
                        .push(", CHAR(92))) > 0");
                }
                or_sep.push_unseparated(")");
            }
        }

        // Series filters
        if let Some(series_uids) = query.series_instance_uid {
            let values = split_backslash(series_uids);
            if !values.is_empty() {
                qb.push(" AND series.series_iuid IN (");
                let mut separated = qb.separated(", ");
                for v in values {
                    separated.push_bind(v);
                }
                separated.push_unseparated(")");
            }
        }
        if let Some(series_number) = query.series_number {
            qb.push(" AND series.series_no = ").push_bind(series_number);
        }
        if let Some(series_desc) = query.series_description {
            qb.push(" AND series.series_desc LIKE ")
                .push_bind(format!("%{}%", series_desc));
        }
        if let Some(modality) = query.modality {
            qb.push(" AND series.modality = ").push_bind(modality);
        }
        if let Some(modality_off) = query.modality_off {
            let values = split_backslash(modality_off);
            if !values.is_empty() {
                qb.push(" AND series.modality NOT IN (");
                let mut separated = qb.separated(", ");
                for v in values {
                    separated.push_bind(v);
                }
                separated.push_unseparated(")");
            }
        }

        // Instance/SOP class filters
        if let Some(sop_class) = query.sop_class {
            qb.push(" AND instance.sop_cuid = ").push_bind(sop_class);
        }
        if let Some(sop_off) = query.sop_class_off {
            let values = split_backslash(sop_off);
            if values.len() <= 1 {
                if let Some(v) = values.first() {
                    qb.push(" AND instance.sop_cuid <> ").push_bind(*v);
                }
            } else {
                qb.push(" AND instance.sop_cuid NOT IN (");
                let mut separated = qb.separated(", ");
                for v in values {
                    separated.push_bind(v);
                }
                separated.push_unseparated(")");
            }
        }

        // Ordering/limit.
        qb.push(" ORDER BY study.study_iuid ASC, series.series_iuid ASC, CAST(instance.inst_no AS UNSIGNED) ASC, instance.sop_iuid ASC");
        if let Some(max) = query.max {
            qb.push(" LIMIT ").push_bind(max as u64);
        }

        let rows = qb
            .build_query_as::<StudyTokenRow>()
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
    }

    async fn fetch_qido_studies_rows(
        &self,
        query: QidoStudiesQuery<'_>,
        include: QidoStudiesIncludeFields,
    ) -> Result<Vec<QidoStudyRow>, PacsError> {
        fn digits_only(value: &str) -> String {
            value.chars().filter(|c| c.is_ascii_digit()).collect()
        }

        let overrides = query.metadata_overrides;
        let pat_name_expr = override_or_default(
            overrides,
            "PatientName",
            "CONCAT_WS('^', person_name.family_name, person_name.given_name, person_name.middle_name)",
        );
        let pat_id_expr = override_or_default(
            overrides,
            "PatientID",
            "(SELECT pid.pat_id FROM patient_id pid WHERE pid.patient_fk = patient.pk ORDER BY pid.pk LIMIT 1)",
        );
        let pat_birthdate_expr = override_or_default(overrides, "PatientBirthDate", "patient.pat_birthdate");
        let pat_sex_expr = override_or_default(overrides, "PatientSex", "patient.pat_sex");
        let study_desc_expr = override_or_default(overrides, "StudyDescription", "study.study_desc");
        let accession_expr = override_or_default(overrides, "AccessionNumber", "study.accession_no");
        let modalities_expr = override_or_default(overrides, "ModalitiesInStudy", "study.mods_in_study");
        let ref_phys_expr = override_or_default(
            overrides,
            "ReferringPhysicianName",
            "CONCAT_WS('^', refpn.family_name, refpn.given_name, refpn.middle_name)",
        );

        let mut qb = QueryBuilder::<MySql>::new("SELECT ");
        qb.push(format!(
            "study.study_date AS study_date,\
                study.study_time AS study_time,\
                {} AS accession_no,\
                {} AS mods_in_study,\
                study.study_iuid AS study_iuid,\
                study.study_id AS study_id,\
                {} AS study_desc,\
                {} AS ref_physician,\
                CAST(study.num_instances1 AS SIGNED) AS num_instances,\
                CAST(study.num_series1 AS SIGNED) AS num_series,\
                {} AS pat_name,\
                {} AS pat_id,\
                {} AS pat_birthdate,\
                {} AS pat_sex",
            accession_expr,
            modalities_expr,
            study_desc_expr,
            ref_phys_expr,
            pat_name_expr,
            pat_id_expr,
            pat_birthdate_expr,
            pat_sex_expr,
        ));

        if include.includefield_00080062 {
            let sop_classes_expr = override_or_default(overrides, "SOPClassesInStudy", "study.cuids_in_study");
            qb.push(", ").push(sop_classes_expr).push(" AS includefield_00080062");
        } else {
            qb.push(", NULL AS includefield_00080062");
        }
        if include.includefield_00081030 {
            qb.push(", ");
            qb.push(study_desc_expr);
            qb.push(" AS includefield_00081030");
        } else {
            qb.push(", NULL AS includefield_00081030");
        }
        if include.includefield_00100021 {
            let issuer_default = "(\
                SELECT iss.entity_id\
                FROM patient_id pid\
                LEFT JOIN issuer iss ON iss.pk = pid.issuer_fk\
                WHERE pid.patient_fk = patient.pk\
                ORDER BY pid.pk\
                LIMIT 1\
            )";
            let issuer_expr = override_or_default(overrides, "IssuerOfPatientID", issuer_default);
            qb.push(", ").push(issuer_expr).push(" AS includefield_00100021");
        } else {
            qb.push(", NULL AS includefield_00100021");
        }

        qb.push(
            " FROM study
               INNER JOIN patient ON patient.pk = study.patient_fk
             LEFT JOIN person_name ON person_name.pk = patient.pat_name_fk
               LEFT JOIN person_name refpn ON refpn.pk = study.ref_phys_name_fk
               WHERE 1=1",
        );

        if let Some(value) = query.patient_id {
            if let Some(col) = override_col(overrides, "PatientID") {
                qb.push(" AND ").push(col).push(" = ").push_bind(value);
            } else {
                qb.push(" AND EXISTS (SELECT 1 FROM patient_id pid WHERE pid.patient_fk = patient.pk AND pid.pat_id = ")
                    .push_bind(value)
                    .push(")");
            }
        }

        if let Some(value) = query.patient_name {
            let patient_name_where = override_or_default(
                overrides,
                "PatientName",
                "CONCAT_WS(' ', person_name.family_name, person_name.given_name, person_name.middle_name)",
            );
            qb.push(" AND ").push(patient_name_where).push(" REGEXP ").push_bind(value);
        }

        if let Some(value) = query.referring_physician_name {
            let ref_phys_where = override_or_default(
                overrides,
                "ReferringPhysicianName",
                "CONCAT_WS(' ', refpn.family_name, refpn.given_name, refpn.middle_name)",
            );
            qb.push(" AND ").push(ref_phys_where).push(" REGEXP ").push_bind(value);
        }

        if let Some(value) = query.accession_no {
            if let Some(col) = override_col(overrides, "AccessionNumber") {
                qb.push(" AND ").push(col).push(" = ").push_bind(value);
            } else {
                qb.push(" AND study.accession_no = ").push_bind(value);
            }
        }

        if let Some(value) = query.modalities_in_study {
            if let Some(col) = override_col(overrides, "ModalitiesInStudy") {
                qb.push(" AND ").push(col).push(" REGEXP ").push_bind(value);
            } else {
                qb.push(" AND study.mods_in_study REGEXP ").push_bind(value);
            }
        }

        if let Some(value) = query.study_id {
            qb.push(" AND study.study_id = ").push_bind(value);
        }

        if let Some(value) = query.study_iuid {
            let values: Vec<String> = value
                .split('\\')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
            if !values.is_empty() {
                qb.push(" AND study.study_iuid IN (");
                let mut separated = qb.separated(", ");
                for v in values {
                    separated.push_bind(v);
                }
                separated.push_unseparated(")");
            }
        }

        if let Some(value) = query.study_date {
            let raw = value.trim();
            if raw.ends_with('-') {
                let start = digits_only(raw.trim_end_matches('-'));
                if !start.is_empty() {
                    qb.push(" AND study.study_date >= ").push_bind(start);
                }
            } else if raw.starts_with('-') {
                let end = digits_only(raw.trim_start_matches('-'));
                if !end.is_empty() {
                    qb.push(" AND study.study_date <= ").push_bind(end);
                }
            } else if let Some((start, end)) = raw.split_once('-') {
                let start = digits_only(start);
                let end = digits_only(end);
                if !start.is_empty() && !end.is_empty() {
                    qb.push(" AND study.study_date BETWEEN ")
                        .push_bind(start)
                        .push(" AND ")
                        .push_bind(end);
                }
            } else {
                let exact = digits_only(raw);
                if !exact.is_empty() {
                    qb.push(" AND study.study_date = ").push_bind(exact);
                }
            }
        }

        if let Some(value) = query.study_time {
            let t = digits_only(value);
            if !t.is_empty() {
                qb.push(" AND study.study_time = ").push_bind(t);
            }
        }

        qb.push(" ORDER BY study.study_iuid ASC LIMIT ").push_bind(query.limit);
        if let Some(offset) = query.offset {
            qb.push(" OFFSET ").push_bind(offset);
        }

        let rows = qb
            .build_query_as::<QidoStudyRow>()
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
    }

}
