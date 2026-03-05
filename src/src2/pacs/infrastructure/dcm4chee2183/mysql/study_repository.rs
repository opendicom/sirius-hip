use async_trait::async_trait;
use chrono::NaiveDateTime;
use sqlx::{MySql, MySqlPool, QueryBuilder, Row};

use crate::src2::errors::PacsError;
use crate::src2::pacs::read_models::QidoStudyRow;
use crate::src2::pacs::read_models::StudyTokenRow;
use crate::src2::pacs::infrastructure::mysql_sql_helpers::{override_col, override_or_default};
use crate::src2::pacs::repositories::StudyRepository;
use crate::src2::pacs::repositories::study_repository::{
    QidoStudiesIncludeFields, QidoStudiesQuery, StudyTokenQuery,
};

pub struct Dcm4chee2183MySqlStudyRepository {
    pool: MySqlPool,
    filesystem_cutoff_date: Option<NaiveDateTime>,
    dirty_table_available: bool,
}

impl Dcm4chee2183MySqlStudyRepository {
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
            "hip_dirty_study_u_study",
            "hip_dirty_study_u_series",
            "hip_dirty_study_u_instance",
        ];

        let rows = sqlx::query(
            "SELECT TRIGGER_NAME AS name \
             FROM information_schema.triggers \
             WHERE TRIGGER_SCHEMA = DATABASE() \
             AND TRIGGER_NAME IN (?, ?, ?, ?)",
        )
        .bind(required[0])
        .bind(required[1])
        .bind(required[2])
         .bind(required[3])
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

        // Best-effort create; if this fails, we will keep filesystem disabled.
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
impl StudyRepository for Dcm4chee2183MySqlStudyRepository {

    async fn fetch_study_token_rows(
        &self,
        query: StudyTokenQuery<'_>,
        include_filesystem: bool,
        include_ohif_metadata: bool,
    ) -> Result<Vec<StudyTokenRow>, PacsError> {
        
        fn split_backslash(value: &str) -> Vec<&str> {
            value.split('\\').filter(|s| !s.is_empty()).collect()
        }

        // Common case optimisation: /studyToken frequently targets a single study.
        let study_uid_values = query.study_instance_uid.map(split_backslash);

        // Filesystem/WADO selection (cutoff date):
        // - Before cutoff: always WADO.
        // - On/after cutoff: filesystem is allowed ONLY when the study is NOT marked dirty.

        let mut qb = QueryBuilder::<MySql>::new("SELECT ");

        let overrides = query.metadata_overrides;
        let patient_id_override = override_col(overrides, "PatientID");
        let patient_name_override = override_col(overrides, "PatientName");
        let needs_patient_name_filter = query.patient_fullname.is_some();
        let needs_patient_id_filter_on_override = if query.patient_id.is_some() {
            matches!(patient_id_override.as_deref(), Some(c) if c.starts_with("patient."))
        } else {
            false
        };
        // Optimization: join patient table when filtering by patient_id (if no override or override references patient)
        // This allows the optimizer to use indexes directly instead of executing a subquery.
        let needs_patient_join_for_filter = query.patient_id.is_some() && patient_id_override.is_none();
        let needs_patient_join = include_ohif_metadata
            || needs_patient_name_filter
            || needs_patient_id_filter_on_override
            || needs_patient_join_for_filter
            || matches!(patient_name_override.as_deref(), Some(c) if c.starts_with("patient."));
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
        // - Optionally it can be overridden as a direct column value, e.g. from `study.study_custom1`.
        let institution_name_expr = override_col(overrides, "InstitutionName").unwrap_or_else(|| "NULL".to_string());

        if include_ohif_metadata {
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
                     NULL AS study_attrs,
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

        if include_ohif_metadata {
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
        } else {
            qb.push(
                "NULL AS series_no,
                 NULL AS series_description,
                 NULL AS modality,
                 NULL AS series_updated_time,
                 NULL AS instance_pk,
                 NULL AS inst_no,
                 NULL AS sop_cuid,
                 NULL AS inst_attrs,",
            );
        }

        if include_filesystem {
            // Always select the raw file reference columns; Rust will use them only
            // when `use_filesystem` is true, avoiding a triple evaluation of the
            // stability expression.
            qb.push("files_pick.filepath AS relative_file_path,");
            qb.push("CAST(files_pick.filesystem_fk AS SIGNED) AS filesystem_fk,");
        } else {
            qb.push("NULL AS relative_file_path, NULL AS filesystem_fk,");
        }

        qb.push("CASE WHEN ");
        if include_filesystem && self.dirty_table_available {
            if let Some(cutoff) = self.filesystem_cutoff_date.as_ref() {
                qb.push("(study.created_time >= ")
                    .push_bind(cutoff.clone())
                    .push(" AND NOT EXISTS (SELECT 1 FROM HIP_dirty_study ds WHERE ds.study_iuid = study.study_iuid)")
                    .push(" AND files_pick.filesystem_fk IS NOT NULL")
                    .push(" AND files_pick.filepath IS NOT NULL")
                    .push(" AND files_pick.filepath <> ''")
                    .push(")");
            } else {
                // Defensive: config validation should prevent this, but keep a safe behavior.
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
        }

        // `files` can have multiple rows per instance; pick one to avoid multiplying result rows.
        // Avoid a derived GROUP BY which can scan the full `files` table.
        if include_filesystem {
            qb.push(
                " LEFT JOIN files files_pick
                    ON files_pick.instance_fk = instance.pk
                   AND files_pick.pk = (
                        SELECT MAX(f2.pk)
                        FROM files f2
                        WHERE f2.instance_fk = instance.pk
                   )",
            );
        }

        qb.push(" WHERE 1=1");

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
            if needs_patient_join {
                // Patient table is joined, use direct filter for optimal performance
                let patient_id_where = override_or_default(overrides, "PatientID", "patient.pat_id");
                qb.push(" AND ")
                    .push(patient_id_where)
                    .push(" = ")
                    .push_bind(patient_id);
            } else if let Some(col) = patient_id_override {
                // If an override exists but we didn't join patient, apply it directly.
                // (Typical overrides reference `study.*` or `patient.*`; patient.* forces join via `needs_patient_join`.)
                qb.push(" AND ").push(col).push(" = ").push_bind(patient_id);
            } else {
                // Fallback: should not reach here due to needs_patient_join_for_filter,
                // but kept for safety if logic changes.
                qb.push(" AND study.patient_fk IN (SELECT p.pk FROM patient p WHERE p.pat_id = ");
                qb.push_bind(patient_id).push(")");
            }
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
        if let Some(values) = study_uid_values.as_ref() {
            if !values.is_empty() {
                if values.len() == 1 {
                    qb.push(" AND study.study_iuid = ")
                        .push_bind(values[0]);
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
        if let Some(accession) = query.accession_number {
            qb.push(" AND study.accession_no = ").push_bind(accession);
        }

        // Study ID (LIKE)
        if let Some(study_id) = query.study_id {
            qb.push(" AND study.study_id LIKE ").push_bind(format!("%{}%", study_id));
        }

        // Study date filter for dcm4chee 2.18.3
        // `study.study_datetime` is a DATETIME, so avoid wrapping it in DATE(...)
        // to keep predicates sargable (i.e. allow index usage).
        if let Some(study_date) = query.study_date {
            let parts = study_date.split('|').collect::<Vec<_>>();
            match parts.as_slice() {
                // "YYYY-MM-DD" (no pipe present)
                [single] if !study_date.contains('|') => {
                    qb.push(" AND study.study_datetime >= ")
                        .push_bind(*single)
                        .push(" AND study.study_datetime < DATE_ADD(")
                        .push_bind(*single)
                        .push(", INTERVAL 1 DAY)");
                }
                // "YYYY-MM-DD|" (>=)
                [start, ""] => {
                    qb.push(" AND study.study_datetime >= ").push_bind(*start);
                }
                // "|YYYY-MM-DD" (<=)
                ["", end] => {
                    qb.push(" AND study.study_datetime < DATE_ADD(")
                        .push_bind(*end)
                        .push(", INTERVAL 1 DAY)");
                }
                // "YYYY-MM-DD|YYYY-MM-DD" (between)
                [start, end] => {
                    qb.push(" AND study.study_datetime >= ")
                        .push_bind(*start)
                        .push(" AND study.study_datetime < DATE_ADD(")
                        .push_bind(*end)
                        .push(", INTERVAL 1 DAY)");
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
        // Use numeric PKs to avoid expensive CAST() on instance numbers and reduce sort CPU.
        qb.push(" ORDER BY study.pk ASC, series.pk ASC, instance.pk ASC");
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
        fn split_backslash(value: &str) -> Vec<&str> {
            value.split('\\').filter(|s| !s.is_empty()).collect()
        }

        fn has_wildcards(value: &str) -> bool {
            value.contains('*') || value.contains('?')
        }

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
                CAST(study.num_series AS SIGNED) AS num_series,\
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
            let issuer_expr = override_or_default(
                overrides,
                "IssuerOfPatientID",
                "COALESCE(patient.pat_id_issuer, '')",
            );
            qb.push(", ").push(issuer_expr).push(" AS includefield_00100021");
        } else {
            qb.push(", NULL AS includefield_00100021");
        }

        // Apply pagination at the study level first, then join back to fetch full row fields.
        qb.push(" FROM (SELECT study.pk AS pk FROM study INNER JOIN patient ON patient.pk = study.patient_fk WHERE 1=1");

        if let Some(value) = query.patient_id {
            let patient_id_where = override_or_default(overrides, "PatientID", "patient.pat_id");
            if has_wildcards(value) {
                qb.push(" AND ")
                    .push(patient_id_where)
                    .push(" LIKE ")
                    .push_bind(to_mysql_like_pattern(value))
                    .push(" ESCAPE '!' ");
            } else {
                qb.push(" AND ")
                    .push(patient_id_where)
                    .push(" = ")
                    .push_bind(value);
            }
        }

        if let Some(value) = query.patient_name {
            let patient_name_where = override_or_default(overrides, "PatientName", "patient.pat_name");
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

        if let Some(value) = query.referring_physician_name {
            let ref_phys_where = override_or_default(overrides, "ReferringPhysicianName", "study.ref_physician");
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

        if let Some(value) = query.accession_no {
            let accession_where = override_or_default(overrides, "AccessionNumber", "study.accession_no");
            if has_wildcards(value) {
                qb.push(" AND ")
                    .push(accession_where)
                    .push(" LIKE ")
                    .push_bind(to_mysql_like_pattern(value))
                    .push(" ESCAPE '!' ");
            } else {
                qb.push(" AND ")
                    .push(accession_where)
                    .push(" = ")
                    .push_bind(value);
            }
        }

        if let Some(value) = query.modalities_in_study {
            let modalities_where = override_or_default(overrides, "ModalitiesInStudy", "study.mods_in_study");
            // `study.mods_in_study` is a backslash-separated list. QIDO matching applies to an item,
            // so we search within `\\<item>\\` boundaries using LIKE and DICOM wildcards.
            let values = split_backslash(value);
            if !values.is_empty() {
                qb.push(" AND (");
                for (idx, v) in values.iter().enumerate() {
                    if idx > 0 {
                        qb.push(" OR ");
                    }
                    qb.push("CONCAT(CHAR(92), IFNULL(")
                        .push(modalities_where.clone())
                        .push(", ''), CHAR(92)) LIKE CONCAT('%', CHAR(92), ")
                        .push_bind(to_mysql_like_pattern(v))
                        .push(", CHAR(92), '%') ESCAPE '!' ");
                }
                qb.push(")");
            }
        }

        if let Some(value) = query.study_id {
            if has_wildcards(value) {
                qb.push(" AND study.study_id LIKE ")
                    .push_bind(to_mysql_like_pattern(value))
                    .push(" ESCAPE '!' ");
            } else {
                qb.push(" AND study.study_id = ").push_bind(value);
            }
        }

        if let Some(value) = query.study_iuid {
            let values: Vec<String> = split_backslash(value).into_iter().map(|s| s.to_string()).collect();
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

        // Study date: QIDO uses DICOM date range syntax (YYYYMMDD-YYYYMMDD).
        // We also accept ISO-like date strings by extracting digits.
        // As above, keep predicates sargable by avoiding DATE(study.study_datetime).
        if let Some(value) = query.study_date {
            let raw = value.trim();
            if raw.starts_with('-') {
                let end = to_dicom_date_digits(raw.trim_start_matches('-'));
                if end.len() == 8 {
                    if let Some(end_iso) = yyyymmdd_to_iso(&end) {
                        qb.push(" AND study.study_datetime < DATE_ADD(")
                            .push_bind(end_iso)
                            .push(", INTERVAL 1 DAY)");
                    }
                }
            } else if raw.ends_with('-') {
                let start = to_dicom_date_digits(raw.trim_end_matches('-'));
                if start.len() == 8 {
                    if let Some(start_iso) = yyyymmdd_to_iso(&start) {
                        qb.push(" AND study.study_datetime >= ").push_bind(start_iso);
                    }
                }
            } else {
                let digits = to_dicom_date_digits(raw);
                if digits.len() == 8 {
                    if let Some(exact_iso) = yyyymmdd_to_iso(&digits) {
                        qb.push(" AND study.study_datetime >= ")
                            .push_bind(exact_iso.clone())
                            .push(" AND study.study_datetime < DATE_ADD(")
                            .push_bind(exact_iso)
                            .push(", INTERVAL 1 DAY)");
                    }
                } else if digits.len() == 16 {
                    let start = &digits[0..8];
                    let end = &digits[8..16];
                    if let (Some(start_iso), Some(end_iso)) = (yyyymmdd_to_iso(start), yyyymmdd_to_iso(end)) {
                        qb.push(" AND study.study_datetime >= ")
                            .push_bind(start_iso)
                            .push(" AND study.study_datetime < DATE_ADD(")
                            .push_bind(end_iso)
                            .push(", INTERVAL 1 DAY)");
                    }
                } else if let Some((start, end)) = raw.split_once('-') {
                    let start = to_dicom_date_digits(start);
                    let end = to_dicom_date_digits(end);
                    if start.len() == 8 && end.len() == 8 {
                        if let (Some(start_iso), Some(end_iso)) = (yyyymmdd_to_iso(&start), yyyymmdd_to_iso(&end)) {
                            qb.push(" AND study.study_datetime >= ")
                                .push_bind(start_iso)
                                .push(" AND study.study_datetime < DATE_ADD(")
                                .push_bind(end_iso)
                                .push(", INTERVAL 1 DAY)");
                        }
                    }
                }
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

        qb.push(") ids INNER JOIN study ON study.pk = ids.pk INNER JOIN patient ON patient.pk = study.patient_fk");

        qb.push(" ORDER BY study.study_iuid ASC");

        let rows = qb
            .build_query_as::<QidoStudyRow>()
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
    }
}
