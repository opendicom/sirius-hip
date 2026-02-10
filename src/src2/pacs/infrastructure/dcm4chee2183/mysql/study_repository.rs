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

pub struct Dcm4chee2183MySqlStudyRepository {
    pool: MySqlPool,
}

impl Dcm4chee2183MySqlStudyRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StudyRepository for Dcm4chee2183MySqlStudyRepository {

    async fn fetch_study_token_rows(
        &self,
        query: StudyTokenQuery<'_>,
        include_filesystem: bool,
        include_wado: bool,
    ) -> Result<Vec<StudyTokenRow>, PacsError> {
        fn split_backslash(value: &str) -> Vec<&str> {
            value.split('\\').filter(|s| !s.is_empty()).collect()
        }

        let use_filesystem_expr = "(
            ABS(TIMESTAMPDIFF(SECOND, study.created_time, study.updated_time)) <= 600 AND 
            ABS(TIMESTAMPDIFF(SECOND, series.created_time, series.updated_time)) <= 600 AND 
            ABS(TIMESTAMPDIFF(SECOND, instance.created_time, instance.updated_time)) <= 600
        )";

        let mut qb = QueryBuilder::<MySql>::new("SELECT ");

        let overrides = query.metadata_overrides;
        let patient_name_expr = override_or_default(overrides, "PatientName", "patient.pat_name");
        let patient_id_expr = override_or_default(overrides, "PatientID", "patient.pat_id");
        let patient_sex_expr = override_or_default(overrides, "PatientSex", "patient.pat_sex");
        let patient_birthdate_expr =
            override_or_default(overrides, "PatientBirthDate", "CAST(patient.pat_birthdate AS CHAR)");
        let accession_no_expr = override_or_default(overrides, "AccessionNumber", "study.accession_no");
        let modalities_expr = override_or_default(overrides, "ModalitiesInStudy", "study.mods_in_study");
        let study_description_expr = override_or_default(overrides, "StudyDescription", "study.study_desc");
        // InstitutionName (0008,0080) is special:
        // - Default comes from decoding `study.study_attrs`.
        // - Optionally it can be overridden as a direct column value (non-dataset), e.g. from `study.study_custom1`.
        let institution_name_expr = override_col(overrides, "InstitutionName").unwrap_or_else(|| "NULL".to_string());

        let ds_sources = dataset_sources(overrides);

        // Dataset overrides (`dataset=true`) work by selecting up to 4 extra dataset blobs
        // into the read-model columns `ov_ds1..ov_ds4`.
        //
        // The helper `dataset_sources(overrides)` returns the distinct set of override sources
        // (the `source = "table.column"` strings) and sorts them for deterministic ordering.
        // We rely on that stable ordering to assign each source to a fixed slot:
        // - ov_ds1 -> ds_sources[0]
        // - ov_ds2 -> ds_sources[1]
        // - ov_ds3 -> ds_sources[2]
        // - ov_ds4 -> ds_sources[3]
        //
        // The OHIF presenter later rebuilds the same `ds_sources` list and uses it to map
        // `source` -> `ov_dsN` bytes for per-row decoding.


        if include_wado {
            qb.push(
                format!(
                    "{} AS patient_name, 
                     {} AS patient_id, 
                     {} AS patient_sex, 
                     {} AS patient_birthdate, 
                    ",
                    patient_name_expr,
                    patient_id_expr,
                    patient_sex_expr,
                    patient_birthdate_expr,
                ),
            );

            qb.push(
                format!(
                    "CAST(DATE(study.study_datetime) AS CHAR) AS study_date, 
                     CAST(TIME(study.study_datetime) AS CHAR) AS study_time, 
                     {} AS study_description,
                     {} AS accession_no, 
                     study.num_instances AS num_instances, 
                     {} AS modalities,
                     {} AS institution_name,
                     study.study_attrs AS study_attrs,
                    ",
                    study_description_expr,
                    accession_no_expr,
                    modalities_expr,
                    institution_name_expr,
                ),
            );
        } else {
            qb.push(
                "NULL AS patient_name,
                 NULL AS patient_id,
                 NULL AS patient_sex,
                 NULL AS patient_birthdate,
                 CAST(DATE(study.study_datetime) AS CHAR) AS study_date,
                 CAST(TIME(study.study_datetime) AS CHAR) AS study_time,
                 NULL AS study_description,
                 study.accession_no AS accession_no,
                 study.num_instances AS num_instances,
                 study.mods_in_study AS modalities,
                 NULL AS institution_name,
                 NULL AS study_attrs,
            ",
            );
        }

        qb.push(
              "study.study_iuid AS study_instance_uid,
               series.series_iuid AS series_instance_uid,
               instance.sop_iuid AS sop_instance_uid, ",
        );

        if include_wado {
            qb.push(
                "CAST(series.series_no AS CHAR) AS series_no,
                 series.series_desc AS series_description,
                 series.modality AS modality,
                 CAST(DATE(series.updated_time) AS CHAR) AS series_updated_time,
                 instance.pk AS instance_pk,
                 CAST(instance.inst_no AS CHAR) AS inst_no,
                 instance.sop_cuid AS sop_cuid,
                 instance.inst_attrs AS inst_attrs,",
            );

            // Push dataset sources for metadata_overrides with dataset=true, if any.
            // We support up to 4 such overrides; if more are defined, the extras will be ignored with a warning.
            // The dataset blobs will be available in the StudyTokenRow as ov_ds1, ov_ds2, etc.
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
            qb.push(", files.filepath, NULL) AS relative_file_path,");

            qb.push("IF(");
            qb.push(use_filesystem_expr);
            qb.push(", files.filesystem_fk, NULL) AS filesystem_fk,");
        } else {
            qb.push("NULL AS relative_file_path, NULL AS filesystem_fk,");
        }

        qb.push("CASE WHEN ");
        qb.push(use_filesystem_expr);
        qb.push(" THEN 1 ELSE 0 END AS use_filesystem ");

        qb.push(
            "FROM `study`
             INNER JOIN `patient` ON patient.pk = study.patient_fk
             INNER JOIN `series` ON series.study_fk = study.pk
             INNER JOIN `instance` ON instance.series_fk = series.pk
             INNER JOIN `files` ON files.instance_fk = instance.pk
             WHERE 1=1",
        );

        // NOTE: `WHERE 1=1` is intentional.
        // It makes it safe to append any number of dynamic filters as `AND ...`
        // without having to special-case whether we're adding the first condition.

        // ------------------------------------------------------------
        // Dynamic filters based on StudyTokenQuery
        // ------------------------------------------------------------

        // Institution filter
        if let Some(institution) = query.institution {
            qb.push(" AND series.institution = ").push_bind(institution);
        }

        // Patient filters
        if let Some(patient_id) = query.patient_id {
            let patient_id_where = override_or_default(overrides, "PatientID", "patient.pat_id");
            qb.push(" AND ")
                .push(patient_id_where)
                .push(" = ")
                .push_bind(patient_id);
        }
        if let Some(patient_regex) = query.patient_fullname {
            // NOTE: columns are VARCHAR(..) BINARY, so matching is case-sensitive.
            let patient_name_where = override_or_default(overrides, "PatientName", "patient.pat_name");
            qb.push(" AND ")
                .push(patient_name_where)
                .push(" REGEXP ")
                .push_bind(patient_regex);
        }

        // StudyInstanceUID: one-or-more separated by '\'
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
            qb.push(" AND study.accession_no = ").push_bind(accession);
        }

        // Study ID (LIKE)
        if let Some(study_id) = query.study_id {
            qb.push(" AND study.study_id LIKE ").push_bind(format!("%{}%", study_id));
        }

        // Study date filter on DATE(study_datetime)
        if let Some(study_date) = query.study_date {
            let parts = study_date.split('|').collect::<Vec<_>>();
            match parts.as_slice() {
                // "YYYY-MM-DD" (no pipe present)
                [single] if !study_date.contains('|') => {
                    qb.push(" AND DATE(study.study_datetime) = ").push_bind(*single);
                }
                // "YYYY-MM-DD|" (>=)
                [start, ""] => {
                    qb.push(" AND DATE(study.study_datetime) >= ").push_bind(*start);
                }
                // "|YYYY-MM-DD" (<=)
                ["", end] => {
                    qb.push(" AND DATE(study.study_datetime) <= ").push_bind(*end);
                }
                // "YYYY-MM-DD|YYYY-MM-DD" (between)
                [start, end] => {
                    qb.push(" AND DATE(study.study_datetime) BETWEEN ")
                        .push_bind(*start)
                        .push(" AND ")
                        .push_bind(*end);
                }
                _ => {
                    // If malformed, ignore at DB level; validation should happen at API layer.
                }
            }
        }

        // ModalityInStudy (study.mods_in_study contains values separated by '\')
        if let Some(mod_in_study) = query.modality_in_study {
            // Delimiter-safe contains check without LIKE/backslash-escape issues.
            // Use CHAR(92) ("\\") to avoid any dependence on NO_BACKSLASH_ESCAPES.
            qb.push(" AND INSTR(CONCAT(CHAR(92), study.mods_in_study, CHAR(92)), CONCAT(CHAR(92), ")
                .push_bind(mod_in_study)
                .push(", CHAR(92))) > 0");
        }

        // CUIDsInStudy (study.cuids_in_study contains values separated by '\')
        if let Some(cuids) = query.cuids_in_study {
            let values = split_backslash(cuids);
            if !values.is_empty() {
                qb.push(" AND (");
                let mut or_sep = qb.separated(" OR ");
                for v in values {
                    or_sep
                        .push(" INSTR(CONCAT(CHAR(92), study.cuids_in_study, CHAR(92)), CONCAT(CHAR(92), ")
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
        fn to_dicom_date_digits(date: &str) -> String {
            date.chars().filter(|c| c.is_ascii_digit()).collect()
        }

        fn yyyymmdd_to_iso(date: &str) -> Option<String> {
            let d: String = date.chars().filter(|c| c.is_ascii_digit()).collect();
            if d.len() != 8 {
                return None;
            }
            Some(format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8]))
        }

        let overrides = query.metadata_overrides;
        let pat_name_expr = override_or_default(overrides, "PatientName", "patient.pat_name");
        let pat_id_expr = override_or_default(overrides, "PatientID", "patient.pat_id");
        let pat_birthdate_expr = override_or_default(
            overrides,
            "PatientBirthDate",
            "CAST(patient.pat_birthdate AS CHAR)",
        );
        let pat_sex_expr = override_or_default(overrides, "PatientSex", "patient.pat_sex");
        let study_desc_expr = override_or_default(overrides, "StudyDescription", "study.study_desc");
        let accession_expr = override_or_default(overrides, "AccessionNumber", "study.accession_no");
        let modalities_expr = override_or_default(overrides, "ModalitiesInStudy", "study.mods_in_study");
        let ref_phys_expr = override_or_default(overrides, "ReferringPhysicianName", "study.ref_physician");

        let mut qb = QueryBuilder::<MySql>::new("SELECT ");
        qb.push(
            format!(
                "CAST(DATE(study.study_datetime) AS CHAR) AS study_date,\
                CAST(TIME(study.study_datetime) AS CHAR) AS study_time,\
                {} AS accession_no,\
                {} AS mods_in_study,\
                study.study_iuid AS study_iuid,\
                study.study_id AS study_id,\
                {} AS study_desc,\
                {} AS ref_physician,\
                CAST(study.num_instances AS SIGNED) AS num_instances,\
                (SELECT COUNT(*) FROM series WHERE series.study_fk = study.pk) AS num_series,\
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
            ),
        );

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
            let issuer_expr = override_or_default(overrides, "IssuerOfPatientID", "''");
            qb.push(", ").push(issuer_expr).push(" AS includefield_00100021");
        } else {
            qb.push(", NULL AS includefield_00100021");
        }

        qb.push(" FROM study INNER JOIN patient ON patient.pk = study.patient_fk WHERE 1=1");

        if let Some(value) = query.patient_id {
            let patient_id_where = override_or_default(overrides, "PatientID", "patient.pat_id");
            qb.push(" AND ").push(patient_id_where).push(" = ").push_bind(value);
        }

        if let Some(value) = query.patient_name {
            let patient_name_where = override_or_default(overrides, "PatientName", "patient.pat_name");
            qb.push(" AND ")
                .push(patient_name_where)
                .push(" REGEXP ")
                .push_bind(value);
        }

        if let Some(value) = query.referring_physician_name {
            let ref_phys_where = override_or_default(overrides, "ReferringPhysicianName", "study.ref_physician");
            qb.push(" AND ").push(ref_phys_where).push(" REGEXP ").push_bind(value);
        }

        if let Some(value) = query.accession_no {
            let accession_where = override_or_default(overrides, "AccessionNumber", "study.accession_no");
            qb.push(" AND ").push(accession_where).push(" = ").push_bind(value);
        }

        if let Some(value) = query.modalities_in_study {
            let modalities_where = override_or_default(overrides, "ModalitiesInStudy", "study.mods_in_study");
            qb.push(" AND ")
                .push(modalities_where)
                .push(" REGEXP ")
                .push_bind(value);
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

        // Study date: QIDO uses DICOM date range syntax (YYYYMMDD-YYYYMMDD).
        // We also accept ISO-like YYYY-MM-DD by stripping digits.
        if let Some(value) = query.study_date {
            let raw = value.trim();
            if raw.ends_with('-') {
                let start = raw.trim_end_matches('-');
                let start_iso = yyyymmdd_to_iso(start).unwrap_or_else(|| start.to_string());
                qb.push(" AND DATE(study.study_datetime) >= ").push_bind(start_iso);
            } else if raw.starts_with('-') {
                let end = raw.trim_start_matches('-');
                let end_iso = yyyymmdd_to_iso(end).unwrap_or_else(|| end.to_string());
                qb.push(" AND DATE(study.study_datetime) <= ").push_bind(end_iso);
            } else if let Some((start, end)) = raw.split_once('-') {
                let start_iso = yyyymmdd_to_iso(start).unwrap_or_else(|| start.to_string());
                let end_iso = yyyymmdd_to_iso(end).unwrap_or_else(|| end.to_string());
                qb.push(" AND DATE(study.study_datetime) BETWEEN ")
                    .push_bind(start_iso)
                    .push(" AND ")
                    .push_bind(end_iso);
            } else {
                let exact_iso = yyyymmdd_to_iso(raw).unwrap_or_else(|| raw.to_string());
                qb.push(" AND DATE(study.study_datetime) = ").push_bind(exact_iso);
            }
        }

        if let Some(value) = query.study_time {
            let t = to_dicom_date_digits(value);
            if t.len() >= 4 {
                let iso = if t.len() >= 6 {
                    format!("{}:{}:{}", &t[0..2], &t[2..4], &t[4..6])
                } else {
                    format!("{}:{}:00", &t[0..2], &t[2..4])
                };
                qb.push(" AND TIME(study.study_datetime) = ").push_bind(iso);
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
