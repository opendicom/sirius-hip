use async_trait::async_trait;
use chrono::NaiveDateTime;
use sqlx::{MySql, MySqlPool, QueryBuilder, Row};

use crate::src2::errors::PacsError;
use crate::src2::pacs::read_models::QidoStudyRow;
use crate::src2::pacs::read_models::StudyTokenRow;
use crate::src2::pacs::infrastructure::mysql_sql_helpers::{
    MetadataMode, include_patient_metadata, metadata_mode, override_col, override_or_default,
    select_non_none, select_ohif_only, select_patient_metadata,
};
use crate::src2::pacs::repositories::StudyRepository;
use crate::src2::pacs::repositories::study_repository::{
    QidoStudiesIncludeFields, QidoStudiesSearchCriteria, StudyTokenSearchCriteria,
};

pub struct Dcm4chee440MySqlStudyRepository {
    pool: MySqlPool,
    filesystem_cutoff_date: Option<NaiveDateTime>,
    dirty_table_available: bool,
}

impl Dcm4chee440MySqlStudyRepository {
    pub async fn new(
        pool: MySqlPool,
        filesystem_cutoff_date: Option<NaiveDateTime>,
    ) -> Result<Self, PacsError> {
        let dirty_table_available = Self::ensure_dirty_table(&pool).await;

        // Triggers are created manually (scripts/mysql/*_dirty_triggers.sql).
        // Enforce presence at startup to avoid silently running without the sticky dirty signal.
        Self::require_dirty_triggers(&pool).await?;

        Ok(Self {
            pool,
            filesystem_cutoff_date,
            dirty_table_available,
        })
    }

    async fn require_dirty_triggers(pool: &MySqlPool) -> Result<(), PacsError> {
        let required = [
            "hip_dirty_study_u_patient",
            "hip_dirty_study_u_patient_id",
            "hip_dirty_study_u_person_name",
            "hip_dirty_study_u_study",
            "hip_dirty_study_u_series",
            "hip_dirty_study_u_instance",
        ];

        let rows = sqlx::query(
            "SELECT TRIGGER_NAME AS name \
             FROM information_schema.triggers \
             WHERE TRIGGER_SCHEMA = DATABASE() \
             AND TRIGGER_NAME IN (?, ?, ?, ?, ?, ?)",
        )
        .bind(required[0])
        .bind(required[1])
        .bind(required[2])
         .bind(required[3])
         .bind(required[4])
         .bind(required[5])
        .fetch_all(pool)
        .await?;

        let mut present = std::collections::HashSet::with_capacity(required.len());
        for row in rows {
            let name: String = row.get("name");
            present.insert(name);
        }

        let missing = required
            .iter()
            .filter(|t| !present.contains(**t))
            .map(|t| t.to_string())
            .collect::<Vec<_>>();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(PacsError::MissingRequiredTriggers { missing })
        }
    }

    /// Ensures the `HIP_dirty_study` table exists.
    /// If it cannot be created/verified (e.g. read-only user), filesystem delivery is disabled.
    async fn ensure_dirty_table(pool: &MySqlPool) -> bool {
        async fn table_exists(pool: &MySqlPool, table_name: &str) -> bool {
            sqlx::query(
                "SELECT COUNT(*) AS cnt FROM information_schema.tables \
                 WHERE table_schema = DATABASE() AND table_name = ?",
            )
            .bind(table_name)
            .fetch_one(pool)
            .await
            .map(|row| {
                let cnt: i64 = row.get("cnt");
                cnt > 0
            })
            .unwrap_or(false)
        }

        let name = "HIP_dirty_study";
        if table_exists(pool, name).await {
            return true;
        }

        let ddl = r#"
            CREATE TABLE IF NOT EXISTS HIP_dirty_study (
              study_iuid     VARCHAR(250) BINARY NOT NULL,
              dirty_since    DATETIME NOT NULL,
              last_dirty_at  DATETIME NOT NULL,
              reason         VARCHAR(64)  BINARY NOT NULL,
              source_table   VARCHAR(16)  BINARY NOT NULL,
              PRIMARY KEY (study_iuid),
              INDEX hip_dirty_last_dirty_at (last_dirty_at)
            ) ENGINE=INNODB
        "#;

        match sqlx::query(ddl).execute(pool).await {
            Ok(_) => {
                let ok = table_exists(pool, name).await;
                if !ok {
                    log::warn!("Could not verify presence of {name}; filesystem delivery will be disabled");
                }
                ok
            }
            Err(e) => {
                log::warn!("Could not create {name}: {e}; filesystem delivery will be disabled");
                false
            }
        }
    }
}

#[async_trait]
impl StudyRepository for Dcm4chee440MySqlStudyRepository {
    async fn fetch_study_token_rows(
        &self,
        criteria: StudyTokenSearchCriteria<'_>,
        include_filesystem: bool,
        include_ohif_metadata: bool,
        include_weasis_metadata: bool,
    ) -> Result<Vec<StudyTokenRow>, PacsError> {
        let metadata_mode = metadata_mode(include_ohif_metadata, include_weasis_metadata);
        let include_patient_metadata = include_patient_metadata(metadata_mode);

        /// Splits a string by backslashes and filters out empty segments.
        fn split_backslash(value: &str) -> Vec<&str> {
            value.split('\\').filter(|s| !s.is_empty()).collect()
        }

        /// Sanitizes a date string by removing dashes, allowing both "YYYY-MM-DD" and "YYYYMMDD" formats.
        fn sanitize_yyyymmdd_maybe(value: &str) -> String {
            value.replace('-', "")
        }

        // Common case optimisation: /studyToken frequently targets a single study.
        // When study_instance_uid is provided, restrict MAX(series.created_time) aggregation
        // to those studies to avoid scanning/grouping the full `series` table.
        let study_uid_values = criteria.study_instance_uid.map(split_backslash);

        // Filesystem/WADO selection (cutoff date):
        // - Before cutoff: always WADO.
        // - On/after cutoff: filesystem is allowed ONLY when the study is NOT marked dirty.
        let mut qb = QueryBuilder::<MySql>::new("SELECT ");

        let overrides = criteria.metadata_overrides;

        // Join patient tables only when required.
        // Avoid patient joins unless required by requested output or filters.
        let patient_id_override = override_col(overrides, "PatientID");
        let has_patient_name_filter = criteria.patient_fullname.is_some();
        let patient_name_override = override_col(overrides, "PatientName");
        let needs_patient_id_filter_on_override = if criteria.patient_id.is_some() {
            match patient_id_override.as_deref() {
                Some(c)
                    if c.starts_with("patient.")
                        || c.starts_with("person_name.")
                        || c.starts_with("patient_id") =>
                {
                    true
                }
                _ => false,
            }
        } else {
            false
        };
        // Optimization: join patient tables when filtering by patient_id (if no override)
        // This allows the optimizer to use indexes directly instead of executing an EXISTS subquery.
        let needs_patient_join_for_filter = criteria.patient_id.is_some() && patient_id_override.is_none();

        let patient_sex_override = override_col(overrides, "PatientSex");
        let patient_birthdate_override = override_col(overrides, "PatientBirthDate");

        let needs_patient_join_for_patient_name_filter = if has_patient_name_filter {
            match patient_name_override.as_deref() {
                // If PatientName is overridden to a non-patient table (e.g. study.*), don't join patient.
                Some(expr) => expr.starts_with("patient.") || expr.starts_with("person_name."),
                // Default PatientName uses person_name.
                None => true,
            }
        } else {
            false
        };

        // Join for SELECT only if selected expressions (per MetadataMode) depend on patient/person_name/patient_id.
        let needs_patient_join_for_select = include_patient_metadata
            && (
                // Default PatientName references person_name.
                match patient_name_override.as_deref() {
                    Some(expr) => expr.starts_with("patient.") || expr.starts_with("person_name."),
                    None => true,
                }
                // Default PatientID references patient_id_first.
                || match patient_id_override.as_deref() {
                    Some(expr) => expr.starts_with("patient.")
                        || expr.starts_with("patient_id")
                        || expr.starts_with("patient_id_first."),
                    None => true,
                }
                // Default PatientSex references patient.
                || match patient_sex_override.as_deref() {
                    Some(expr) => expr.starts_with("patient."),
                    None => true,
                }
                // PatientBirthDate is only selected for OHIF.
                || (metadata_mode == MetadataMode::Ohif
                    && match patient_birthdate_override.as_deref() {
                        Some(expr) => expr.starts_with("patient."),
                        None => true,
                    })
            );

        let needs_patient_join = needs_patient_join_for_select
            || needs_patient_join_for_patient_name_filter
            || needs_patient_id_filter_on_override
            || needs_patient_join_for_filter;
        let patient_name_expr = override_or_default(
            overrides,
            "PatientName",
            "CONCAT_WS('^', person_name.family_name, person_name.given_name, person_name.middle_name)",
        );
        let patient_id_expr = override_or_default(
            overrides,
            "PatientID",
            "patient_id_first.pat_id",
        );
        let patient_sex_expr = override_or_default(overrides, "PatientSex", "patient.pat_sex");
        let patient_birthdate_expr = override_or_default(overrides, "PatientBirthDate", "patient.pat_birthdate");
        let accession_no_select = override_or_default(overrides, "AccessionNumber", "study.accession_no");
        let modalities_select = override_or_default(overrides, "ModalitiesInStudy", "study.mods_in_study");
        let study_description_select = override_or_default(overrides, "StudyDescription", "study.study_desc");
        // InstitutionName (0008,0080) is special:
        // - Default comes from decoding `study_attrs` (currently not selected in src2).
        // - Optionally it can be overridden as a direct column value.
        let institution_name_select = override_col(overrides, "InstitutionName").unwrap_or_else(|| "NULL".to_string());

        // ------------------------------------------------------------
        // SELECT: Patient + Study columns
        // ------------------------------------------------------------
        let patient_name_select = select_patient_metadata(metadata_mode, &patient_name_expr);
        let patient_id_select = select_patient_metadata(metadata_mode, &patient_id_expr);
        let patient_sex_select = select_patient_metadata(metadata_mode, &patient_sex_expr);
        let patient_birthdate_select = select_ohif_only(metadata_mode, &patient_birthdate_expr);

        let study_date_select = select_patient_metadata(metadata_mode, "study.study_date");
        let study_time_select = select_patient_metadata(metadata_mode, "study.study_time");
        let study_description_select = select_patient_metadata(metadata_mode, &study_description_select);
        let accession_no_select = select_patient_metadata(metadata_mode, &accession_no_select);
        let num_instances_select = select_ohif_only(metadata_mode, "study.num_instances1");
        let modalities_select = select_ohif_only(metadata_mode, &modalities_select);
        let institution_name_select = select_ohif_only(metadata_mode, &institution_name_select);

        qb.push(patient_name_select).push(" AS patient_name, ");
        qb.push(patient_id_select).push(" AS patient_id, ");
        qb.push(patient_sex_select).push(" AS patient_sex, ");
        qb.push(patient_birthdate_select).push(" AS patient_birthdate, ");
        qb.push(study_date_select).push(" AS study_date, ");
        qb.push(study_time_select).push(" AS study_time, ");
        qb.push(study_description_select).push(" AS study_description, ");
        qb.push(accession_no_select).push(" AS accession_no, ");
        qb.push(num_instances_select).push(" AS num_instances, ");
        qb.push(modalities_select).push(" AS modalities, ");
        qb.push(institution_name_select).push(" AS institution_name, ");
        qb.push("NULL AS study_attrs,");

        qb.push(
            "study.study_iuid AS study_instance_uid,
             series.series_iuid AS series_instance_uid,
             instance.sop_iuid AS sop_instance_uid,",
        );

        // ------------------------------------------------------------
        // SELECT: Series + Instance columns
        // ------------------------------------------------------------
        let series_no_select = select_non_none(metadata_mode, "series.series_no");
        let series_description_select = select_non_none(metadata_mode, "series.series_desc");
        let modality_select = select_non_none(metadata_mode, "series.modality");
        let series_updated_time_select =
            select_ohif_only(metadata_mode, "CAST(DATE(series.updated_time) AS CHAR)");
        let instance_pk_select = select_ohif_only(metadata_mode, "CAST(instance.pk AS SIGNED)");
        let inst_no_select = select_non_none(metadata_mode, "instance.inst_no");
        let sop_cuid_select = select_ohif_only(metadata_mode, "instance.sop_cuid");
        let inst_attrs_select = select_ohif_only(metadata_mode, "dicomattrs.attrs");

        qb.push(series_no_select).push(" AS series_no, ");
        qb.push(series_description_select).push(" AS series_description, ");
        qb.push(modality_select).push(" AS modality, ");
        qb.push(series_updated_time_select).push(" AS series_updated_time, ");
        qb.push(instance_pk_select).push(" AS instance_pk, ");
        qb.push(inst_no_select).push(" AS inst_no, ");
        qb.push(sop_cuid_select).push(" AS sop_cuid, ");
        qb.push(inst_attrs_select).push(" AS inst_attrs, ");

        if include_filesystem {
            // Always select the raw file reference columns; Rust will use them only
            // when `use_filesystem` is true, avoiding a triple evaluation of the
            // stability expression.
            qb.push("file_ref.filepath AS relative_file_path,");
            qb.push("CAST(file_ref.filesystem_fk AS SIGNED) AS filesystem_fk,");
        } else {
            qb.push("NULL AS relative_file_path, NULL AS filesystem_fk,");
        }

        qb.push("CASE WHEN ");
        if include_filesystem && self.dirty_table_available {
            if let Some(cutoff) = self.filesystem_cutoff_date.as_ref() {
                qb.push("(study.created_time >= ")
                    .push_bind(cutoff.clone())
                    .push(" AND NOT EXISTS (SELECT 1 FROM HIP_dirty_study ds WHERE ds.study_iuid = study.study_iuid)")
                    .push(" AND file_ref.filesystem_fk IS NOT NULL")
                    .push(" AND file_ref.filepath IS NOT NULL")
                    .push(" AND file_ref.filepath <> ''")
                    .push(")");
            } else {
                qb.push("(0)");
            }
        } else {
            qb.push("(0)");
        }
        qb.push(" THEN 1 ELSE 0 END AS use_filesystem ");

        qb.push(
            "FROM `study`
             INNER JOIN `series` ON series.study_fk = study.pk
             INNER JOIN `instance` ON instance.series_fk = series.pk",
        );

        if needs_patient_join {
            qb.push(" INNER JOIN `patient` ON patient.pk = study.patient_fk");

            // `person_name` is only needed when the PatientName expression references it.
            if has_patient_name_filter
                || (include_patient_metadata && patient_name_expr.contains("person_name."))
            {
                qb.push(" LEFT JOIN `person_name` ON person_name.pk = patient.pat_name_fk");
            }

            // Optimization: when filtering by patient_id, join patient_id table directly
            // for better performance than EXISTS subquery.
            if criteria.patient_id.is_some() && patient_id_override.is_none() {
                qb.push(
                    " INNER JOIN patient_id patient_id_filter
                        ON patient_id_filter.patient_fk = patient.pk",
                );
            }

            // Avoid correlated subqueries in SELECT by joining the first patient_id row.
            // Only needed when selecting PatientID (OHIF/Weasis) and there is no PatientID override.
            if include_patient_metadata && patient_id_override.is_none() {
                qb.push(
                    // IMPORTANT: do not use a derived table with GROUP BY over the full `patient_id`.
                    // For single-study requests (common in /studyToken) that forces MySQL to scan the
                    // entire patient_id table. A correlated MIN(pk) join is typically much faster
                    // because it uses the patient_id(patient_fk) index and only touches rows for the
                    // patient selected by this query.
                    " LEFT JOIN patient_id patient_id_first
                        ON patient_id_first.pk = (
                            SELECT MIN(pid.pk)
                            FROM patient_id pid
                            WHERE pid.patient_fk = patient.pk
                        )",
                );
            }
        }

        if include_filesystem {
            // IMPORTANT: Avoid a derived table with GROUP BY over the full `file_ref`.
            // In large PACS databases that can force a scan of `file_ref` even when the
            // request targets a single study. A correlated MAX(pk) join is typically faster
            // here because it uses the `file_ref(instance_fk)` index and only touches rows
            // for the instances selected by this query.
            qb.push(
                " LEFT JOIN file_ref
                    ON file_ref.instance_fk = instance.pk
                   AND file_ref.pk = (
                        SELECT MAX(fr2.pk)
                        FROM file_ref fr2
                        WHERE fr2.instance_fk = instance.pk
                   )",
            );
        }

        // DICOM dataset blobs are only needed for OHIF rendering.
        if metadata_mode == MetadataMode::Ohif {
            qb.push(" LEFT JOIN `dicomattrs` ON dicomattrs.pk = instance.dicomattrs_fk");

        }

        qb.push(" WHERE 1=1");

        // ------------------------------------------------------------
        // Dynamic filters based on StudyTokenQuery
        // ------------------------------------------------------------

        // Institution filter
        if let Some(institution) = criteria.institution {
            qb.push(" AND series.institution = ").push_bind(institution);
        }

        // Patient filters
        if let Some(patient_id) = criteria.patient_id {
            if let Some(col) = override_col(overrides, "PatientID") {
                qb.push(" AND ").push(col).push(" = ").push_bind(patient_id);
            } else if needs_patient_join {
                // Patient tables are joined, use direct filter with patient_id table for optimal performance
                qb.push(" AND patient_id_filter.pat_id = ").push_bind(patient_id);
            } else {
                // Fallback: should not reach here due to needs_patient_join_for_filter,
                // but kept for safety if logic changes.
                qb.push(" AND EXISTS (SELECT 1 FROM patient_id pid WHERE pid.patient_fk = study.patient_fk AND pid.pat_id = ")
                    .push_bind(patient_id)
                    .push(")");
            }
        }
        if let Some(patient_regex) = criteria.patient_fullname {
            let patient_name_where = override_or_default(
                overrides,
                "PatientName",
                "CONCAT_WS(' ', person_name.family_name, person_name.given_name, person_name.middle_name)",
            );
            qb.push(" AND ").push(patient_name_where).push(" REGEXP ").push_bind(patient_regex);
        }

        // StudyInstanceUID: one-or-more separated by '\'
        if let Some(values) = study_uid_values.as_ref() {
            if !values.is_empty() {
                if values.len() == 1 {
                    qb.push(" AND study.study_iuid = ").push_bind(values[0]);
                } else {
                    qb.push(" AND study.study_iuid IN (");
                    let mut separated = qb.separated(", ");
                    for v in values {
                        separated.push_bind(*v);
                    }
                    separated.push_unseparated(")");
                }
            }
        }

        // Accession number
        if let Some(accession) = criteria.accession_number {
            if let Some(col) = override_col(overrides, "AccessionNumber") {
                qb.push(" AND ").push(col).push(" = ").push_bind(accession);
            } else {
                qb.push(" AND study.accession_no = ").push_bind(accession);
            }
        }

        // Study ID (LIKE)
        if let Some(study_id) = criteria.study_id {
            qb.push(" AND study.study_id LIKE ").push_bind(format!("%{}%", study_id));
        }

        // Study date filter on s.study_date (string YYYYMMDD in dcm4chee 4.4)
        if let Some(study_date) = criteria.study_date {
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
                    // If malformed, ignore at DB level; validation should happen at API layer.
                }
            }
        }

        // ModalityInStudy (study.mods_in_study contains values separated by '\\')
        if let Some(mod_in_study) = criteria.modality_in_study {
            let mods_col = override_or_default(overrides, "ModalitiesInStudy", "study.mods_in_study");
            qb.push(" AND INSTR(CONCAT(CHAR(92), IFNULL(")
                .push(mods_col)
                .push(", ''), CHAR(92)), CONCAT(CHAR(92), ")
                .push_bind(mod_in_study)
                .push(", CHAR(92))) > 0");
        }

        // CUIDsInStudy (study.cuids_in_study contains values separated by '\\')
        if let Some(cuids) = criteria.cuids_in_study {
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
        if let Some(series_uids) = criteria.series_instance_uid {
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
        if let Some(series_number) = criteria.series_number {
            qb.push(" AND series.series_no = ").push_bind(series_number);
        }
        if let Some(series_desc) = criteria.series_description {
            qb.push(" AND series.series_desc LIKE ")
                .push_bind(format!("%{}%", series_desc));
        }
        if let Some(modality) = criteria.modality {
            qb.push(" AND series.modality = ").push_bind(modality);
        }
        if let Some(modality_off) = criteria.modality_off {
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
        if let Some(sop_class) = criteria.sop_class {
            qb.push(" AND instance.sop_cuid = ").push_bind(sop_class);
        }
        if let Some(sop_off) = criteria.sop_class_off {
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
        // Use numeric PKs to avoid expensive CAST() on instance numbers and reduce sort CPU.
        // Any deterministic order is fine (file_index is just an internal stable mapping).
        qb.push(" ORDER BY study.pk ASC, series.pk ASC, instance.pk ASC");
        if let Some(max) = criteria.max {
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
        criteria: QidoStudiesSearchCriteria<'_>,
        include: QidoStudiesIncludeFields,
    ) -> Result<Vec<QidoStudyRow>, PacsError> {
        fn digits_only(value: &str) -> String {
            value.chars().filter(|c| c.is_ascii_digit()).collect()
        }

        fn has_wildcards(value: &str) -> bool {
            value.contains('*') || value.contains('?')
        }

        // Convert DICOM QIDO wildcards to a MySQL LIKE pattern.
        // - `*` => `%`
        // - `?` => `_`
        // Uses `!` as the LIKE escape character.
        fn to_mysql_like_pattern(value: &str) -> String {
            let mut out = String::with_capacity(value.len());
            for ch in value.chars() {
                match ch {
                    '!' => out.push_str("!!"),
                    '%' => out.push_str("!%"),
                    '_' => out.push_str("!_"),
                    '*' => out.push('%'),
                    '?' => out.push('_'),
                    _ => out.push(ch),
                }
            }
            out
        }

        let overrides = criteria.metadata_overrides;

        // Avoid correlated subqueries in SELECT by joining the first patient_id row.
        // If an override supplies a direct column, we use it and skip the join.
        let patient_id_override = override_col(overrides, "PatientID");
        let issuer_override = override_col(overrides, "IssuerOfPatientID");

        // Fast path for exact PatientID searches:
        // - Filter in the paginated subquery with an indexed join on `patient_id.pat_id`.
        // - Return PatientID as the exact searched value (constant) and skip the global `pid_first` derived join.
        // This avoids scanning/grouping the entire `patient_id` table just to render PatientID.
        let exact_patient_id_for_fast_path = match criteria.patient_id {
            Some(v) if !has_wildcards(v) => Some(v),
            _ => None,
        };
        let use_patient_id_fast_path = exact_patient_id_for_fast_path.is_some()
            && patient_id_override.is_none()
            && !include.includefield_00100021;

        let mut needs_pid_first_join = patient_id_override.is_none()
            || (include.includefield_00100021 && issuer_override.is_none());
        let needs_issuer_join = include.includefield_00100021 && issuer_override.is_none();

        if use_patient_id_fast_path {
            needs_pid_first_join = false;
        }

        let pat_name_expr = override_or_default(
            overrides,
            "PatientName",
            "CONCAT_WS('^', person_name.family_name, person_name.given_name, person_name.middle_name)",
        );
        let pat_id_expr = patient_id_override
            .clone()
            .unwrap_or_else(|| "pid_first.pat_id".to_string());
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
            "study.study_date AS study_date,
             study.study_time AS study_time,
             {} AS accession_no,
             {} AS mods_in_study,
             study.study_iuid AS study_iuid,
             study.study_id AS study_id,
             {} AS study_desc,
             {} AS ref_physician,
             CAST(study.num_instances1 AS SIGNED) AS num_instances,
             CAST(study.num_series1 AS SIGNED) AS num_series,
             {} AS pat_name,",
            accession_expr,
            modalities_expr,
            study_desc_expr,
            ref_phys_expr,
            pat_name_expr,
        ));

        if use_patient_id_fast_path {
            qb.push(" ")
                .push_bind(exact_patient_id_for_fast_path.expect("fast path requires exact PatientID"))
                .push(" AS pat_id,");
        } else {
            qb.push(" ").push(pat_id_expr).push(" AS pat_id,");
        }

        qb.push(format!(
            " {} AS pat_birthdate,
             {} AS pat_sex",
            pat_birthdate_expr,
            pat_sex_expr,
        ));

        // Include optional fields based on `includefield` parameters. 
        // If an include field is requested, select the corresponding 
        // value; otherwise select NULL to keep the column present but 
        // empty.

        // SopClassesInStudy
        if include.includefield_00080062 {
            let sop_classes_expr = override_or_default(overrides, "SOPClassesInStudy", "study.cuids_in_study");
            qb.push(", ").push(sop_classes_expr).push(" AS includefield_00080062");
        } else {
            qb.push(", NULL AS includefield_00080062");
        }
        
        // StudyDescription
        if include.includefield_00081030 {
            qb.push(", ");
            qb.push(study_desc_expr);
            qb.push(" AS includefield_00081030");
        } else {
            qb.push(", NULL AS includefield_00081030");
        }

        // IssuerOfPatientID
        if include.includefield_00100021 {
            let issuer_expr = issuer_override.clone().unwrap_or_else(|| "iss_first.entity_id".to_string());
            qb.push(", ").push(issuer_expr).push(" AS includefield_00100021");
        } else {
            qb.push(", NULL AS includefield_00100021");
        }

            // Apply filters + pagination first (study-level semantics: 1 row per study).
            // This keeps expensive expressions/joins limited to the requested window.
            qb.push(
                " FROM (
                SELECT study.pk
                FROM study
                INNER JOIN patient ON patient.pk = study.patient_fk
                LEFT JOIN person_name ON person_name.pk = patient.pat_name_fk
                LEFT JOIN person_name refpn ON refpn.pk = study.ref_phys_name_fk",
            );

            if use_patient_id_fast_path {
                qb.push(" INNER JOIN patient_id pid_filter ON pid_filter.patient_fk = patient.pk AND pid_filter.pat_id = ")
                    .push_bind(exact_patient_id_for_fast_path.expect("fast path requires exact PatientID"));
            }

            qb.push(" WHERE 1=1");

        if let Some(value) = criteria.patient_id {
            if use_patient_id_fast_path {
                // already filtered by the `pid_filter` join in the subquery
            } else
            if let Some(col) = override_col(overrides, "PatientID") {
                if has_wildcards(value) {
                    qb.push(" AND ")
                        .push(col)
                        .push(" LIKE ")
                        .push_bind(to_mysql_like_pattern(value))
                        .push(" ESCAPE '!' ");
                } else {
                    qb.push(" AND ").push(col).push(" = ").push_bind(value);
                }
            } else {
                qb.push(" AND EXISTS (SELECT 1 FROM patient_id pid WHERE pid.patient_fk = patient.pk AND ");
                if has_wildcards(value) {
                    qb.push("pid.pat_id LIKE ")
                        .push_bind(to_mysql_like_pattern(value))
                        .push(" ESCAPE '!' ");
                } else {
                    qb.push("pid.pat_id = ").push_bind(value);
                }
                qb.push(")");
            }
        }

        if let Some(value) = criteria.patient_name {
            let patient_name_where = override_or_default(
                overrides,
                "PatientName",
                "CONCAT_WS('^', person_name.family_name, person_name.given_name, person_name.middle_name)",
            );
            if has_wildcards(value) {
                qb.push(" AND ")
                    .push(patient_name_where)
                    .push(" LIKE ")
                    .push_bind(to_mysql_like_pattern(value))
                    .push(" ESCAPE '!' ");
            } else {
                qb.push(" AND ")
                    .push(patient_name_where)
                    .push(" = ")
                    .push_bind(value);
            }
        }

        if let Some(value) = criteria.referring_physician_name {
            let ref_phys_where = override_or_default(
                overrides,
                "ReferringPhysicianName",
                "CONCAT_WS('^', refpn.family_name, refpn.given_name, refpn.middle_name)",
            );
            if has_wildcards(value) {
                qb.push(" AND ")
                    .push(ref_phys_where)
                    .push(" LIKE ")
                    .push_bind(to_mysql_like_pattern(value))
                    .push(" ESCAPE '!' ");
            } else {
                qb.push(" AND ")
                    .push(ref_phys_where)
                    .push(" = ")
                    .push_bind(value);
            }
        }

        if let Some(value) = criteria.accession_no {
            if let Some(col) = override_col(overrides, "AccessionNumber") {
                if has_wildcards(value) {
                    qb.push(" AND ")
                        .push(col)
                        .push(" LIKE ")
                        .push_bind(to_mysql_like_pattern(value))
                        .push(" ESCAPE '!' ");
                } else {
                    qb.push(" AND ").push(col).push(" = ").push_bind(value);
                }
            } else {
                if has_wildcards(value) {
                    qb.push(" AND study.accession_no LIKE ")
                        .push_bind(to_mysql_like_pattern(value))
                        .push(" ESCAPE '!' ");
                } else {
                    qb.push(" AND study.accession_no = ").push_bind(value);
                }
            }
        }

        if let Some(value) = criteria.modalities_in_study {
            let mods_col = override_or_default(overrides, "ModalitiesInStudy", "study.mods_in_study");
            // `study.mods_in_study` is a backslash-separated list. QIDO matching applies to an item,
            // so we search within `\\<item>\\` boundaries using LIKE and wildcards.
            qb.push(" AND CONCAT(CHAR(92), IFNULL(")
                .push(mods_col)
                .push(", ''), CHAR(92)) LIKE CONCAT('%', CHAR(92), ")
                .push_bind(to_mysql_like_pattern(value))
                .push(", CHAR(92), '%') ESCAPE '!' ");
        }

        if let Some(value) = criteria.study_id {
            if has_wildcards(value) {
                qb.push(" AND study.study_id LIKE ")
                    .push_bind(to_mysql_like_pattern(value))
                    .push(" ESCAPE '!' ");
            } else {
                qb.push(" AND study.study_id = ").push_bind(value);
            }
        }

        if let Some(value) = criteria.study_iuid {
            let values: Vec<String> = value
                .split('\\')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
            if !values.is_empty() {
                if values.len() == 1 {
                    let v = values[0].clone();
                    qb.push(" AND study.study_iuid = ").push_bind(v);
                } else {
                    qb.push(" AND study.study_iuid IN (");
                    let mut separated = qb.separated(", ");
                    for v in values {
                        separated.push_bind(v);
                    }
                    separated.push_unseparated(")");
                }
            }
        }

        if let Some(value) = criteria.study_date {
            let raw = value.trim();
            if raw.starts_with('-') {
                let end = digits_only(raw.trim_start_matches('-'));
                if end.len() == 8 {
                    qb.push(" AND study.study_date <= ").push_bind(end);
                }
            } else if raw.ends_with('-') {
                let start = digits_only(raw.trim_end_matches('-'));
                if start.len() == 8 {
                    qb.push(" AND study.study_date >= ").push_bind(start);
                }
            } else {
                let digits = digits_only(raw);
                if digits.len() == 8 {
                    qb.push(" AND study.study_date = ").push_bind(digits);
                } else if digits.len() == 16 {
                    let start = digits[0..8].to_string();
                    let end = digits[8..16].to_string();
                    qb.push(" AND study.study_date BETWEEN ")
                        .push_bind(start)
                        .push(" AND ")
                        .push_bind(end);
                } else if let Some((start, end)) = raw.split_once('-') {
                    let start = digits_only(start);
                    let end = digits_only(end);
                    if start.len() == 8 && end.len() == 8 {
                        qb.push(" AND study.study_date BETWEEN ")
                            .push_bind(start)
                            .push(" AND ")
                            .push_bind(end);
                    }
                }
            }
        }

        if let Some(value) = criteria.study_time {
            let t = digits_only(value);
            if !t.is_empty() {
                qb.push(" AND study.study_time = ").push_bind(t);
            }
        }

        qb.push(" ORDER BY study.study_iuid ASC LIMIT ")
            .push_bind(criteria.limit);
        if let Some(offset) = criteria.offset {
            qb.push(" OFFSET ").push_bind(offset);
        }

                qb.push(
                        ") ids
                            INNER JOIN study ON study.pk = ids.pk
                            INNER JOIN patient ON patient.pk = study.patient_fk
                            LEFT JOIN person_name ON person_name.pk = patient.pat_name_fk
                            LEFT JOIN person_name refpn ON refpn.pk = study.ref_phys_name_fk",
                );

        if needs_pid_first_join {
            qb.push(
                " LEFT JOIN (
                    SELECT pid.patient_fk, pid.pat_id, pid.issuer_fk
                    FROM patient_id pid
                    INNER JOIN (
                        SELECT patient_fk, MIN(pk) AS min_pk
                        FROM patient_id
                        GROUP BY patient_fk
                    ) pid_min
                        ON pid_min.patient_fk = pid.patient_fk
                       AND pid_min.min_pk = pid.pk
                ) pid_first ON pid_first.patient_fk = patient.pk",
            );

            if needs_issuer_join {
                qb.push(" LEFT JOIN issuer iss_first ON iss_first.pk = pid_first.issuer_fk");
            }
        }

        qb.push(" ORDER BY study.study_iuid ASC");

        let rows = qb
            .build_query_as::<QidoStudyRow>()
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
    }

}
