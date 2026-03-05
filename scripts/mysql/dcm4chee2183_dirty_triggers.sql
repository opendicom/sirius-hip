-- Sirius HIP - Dirty study triggers (dcm4chee 2.18.3 / MySQL)
--
-- Purpose:
--   Maintain HIP_dirty_study (sticky dirty signal) when patients/studies/series/instances are UPDATED.
--   This supports Sirius HIP FS vs WADO selection (cutoff + dirty table).
--
-- How to run (example):
--   mysql -h <host> -u <user> -p <database> < scripts/mysql/dcm4chee2183_dirty_triggers.sql
--
-- Notes:
--   - Requires privileges: CREATE TABLE, CREATE TRIGGER.
--   - Triggers are AFTER UPDATE only (no INSERT triggers).
--     Rationale: normal ingestion inserts new study/series/instance rows, and we do not want
--     new data to be considered "dirty" (dirty is meant to represent post-ingest corrections).
--   - Triggers are intentionally "strict": they mark dirty only on meaningful metadata changes
--     (e.g. *_attrs, descriptions, identifiers), not on counter-only updates or updated_time alone.
--   - dcm4chee performs several ingestion-time "housekeeping" UPDATEs (especially NULL -> value
--     transitions) after the initial INSERTs. Those should NOT mark the study dirty, otherwise
--     every ingested study would become dirty and filesystem selection would never activate.
--
--     Specifically, these ingestion-time transitions are ignored:
--       - study.patient_fk: NULL -> non-NULL
--       - study.accno_issuer_fk: NULL -> non-NULL
--       - instance.retrieve_aets: NULL -> non-NULL
--
--     Dirty is still marked when the same columns change in any other way:
--       - value -> different value (true reassignment/correction)
--       - value -> NULL
--
--   - INSERTs from normal ingestion do NOT mark dirty.

-- ---------------------------------------------------------------------------
-- Dirty table
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS HIP_dirty_study (
  study_iuid     VARCHAR(250) BINARY NOT NULL,
  dirty_since    DATETIME NOT NULL,
  last_dirty_at  DATETIME NOT NULL,
  reason         VARCHAR(64)  BINARY NOT NULL,
  source_table   VARCHAR(16)  BINARY NOT NULL,
  PRIMARY KEY (study_iuid),
  INDEX hip_dirty_last_dirty_at (last_dirty_at)
) ENGINE=INNODB;

-- ---------------------------------------------------------------------------
-- Triggers
-- ---------------------------------------------------------------------------

DROP TRIGGER IF EXISTS hip_dirty_study_u_patient;
DROP TRIGGER IF EXISTS hip_dirty_study_u_study;
DROP TRIGGER IF EXISTS hip_dirty_study_u_series;
DROP TRIGGER IF EXISTS hip_dirty_study_u_instance;

DELIMITER $$

CREATE TRIGGER hip_dirty_study_u_patient
AFTER UPDATE ON `patient`
FOR EACH ROW
BEGIN
  IF
    -- Patient merge is a meaningful correction (do not ignore NULL -> value)
    NOT (OLD.merge_fk <=> NEW.merge_fk) OR
    (
      NOT (OLD.pat_id <=> NEW.pat_id)
      AND NOT (OLD.pat_id IS NULL AND NEW.pat_id IS NOT NULL)
    ) OR
    (
      NOT (OLD.pat_id_issuer <=> NEW.pat_id_issuer)
      AND NOT (OLD.pat_id_issuer IS NULL AND NEW.pat_id_issuer IS NOT NULL)
    ) OR
    (
      NOT (OLD.pat_name <=> NEW.pat_name)
      AND NOT (OLD.pat_name IS NULL AND NEW.pat_name IS NOT NULL)
    ) OR
    (
      NOT (OLD.pat_fn_sx <=> NEW.pat_fn_sx)
      AND NOT (OLD.pat_fn_sx IS NULL AND NEW.pat_fn_sx IS NOT NULL)
    ) OR
    (
      NOT (OLD.pat_gn_sx <=> NEW.pat_gn_sx)
      AND NOT (OLD.pat_gn_sx IS NULL AND NEW.pat_gn_sx IS NOT NULL)
    ) OR
    (
      NOT (OLD.pat_i_name <=> NEW.pat_i_name)
      AND NOT (OLD.pat_i_name IS NULL AND NEW.pat_i_name IS NOT NULL)
    ) OR
    (
      NOT (OLD.pat_p_name <=> NEW.pat_p_name)
      AND NOT (OLD.pat_p_name IS NULL AND NEW.pat_p_name IS NOT NULL)
    ) OR
    (
      NOT (OLD.pat_birthdate <=> NEW.pat_birthdate)
      AND NOT (OLD.pat_birthdate IS NULL AND NEW.pat_birthdate IS NOT NULL)
    ) OR
    (
      NOT (OLD.pat_sex <=> NEW.pat_sex)
      AND NOT (OLD.pat_sex IS NULL AND NEW.pat_sex IS NOT NULL)
    ) OR
    (
      NOT (OLD.pat_custom1 <=> NEW.pat_custom1)
      AND NOT (OLD.pat_custom1 IS NULL AND NEW.pat_custom1 IS NOT NULL)
    ) OR
    (
      NOT (OLD.pat_custom2 <=> NEW.pat_custom2)
      AND NOT (OLD.pat_custom2 IS NULL AND NEW.pat_custom2 IS NOT NULL)
    ) OR
    (
      NOT (OLD.pat_custom3 <=> NEW.pat_custom3)
      AND NOT (OLD.pat_custom3 IS NULL AND NEW.pat_custom3 IS NOT NULL)
    ) OR
    (
      -- Ignore ingestion-time attrs population (NULL -> value)
      NOT (OLD.pat_attrs <=> NEW.pat_attrs)
      AND NOT (OLD.pat_attrs IS NULL AND NEW.pat_attrs IS NOT NULL)
    )
  THEN
    -- Patient is the top of the hierarchy: a patient change makes all its studies dirty.
    INSERT INTO HIP_dirty_study (study_iuid, dirty_since, last_dirty_at, reason, source_table)
    SELECT st.study_iuid, NOW(), NOW(), 'patient_metadata_change', 'patient'
    FROM `study` st
    WHERE st.patient_fk = NEW.pk
    ON DUPLICATE KEY UPDATE
      last_dirty_at = NOW(),
      reason = 'patient_metadata_change',
      source_table = 'patient';
  END IF;
END$$

CREATE TRIGGER hip_dirty_study_u_study
AFTER UPDATE ON `study`
FOR EACH ROW
BEGIN
  IF
    NOT (OLD.study_desc <=> NEW.study_desc) OR
    NOT (OLD.study_attrs <=> NEW.study_attrs) OR
    NOT (OLD.study_id <=> NEW.study_id) OR
    NOT (OLD.accession_no <=> NEW.accession_no) OR
    NOT (OLD.ref_physician <=> NEW.ref_physician) OR
    NOT (OLD.study_datetime <=> NEW.study_datetime) OR
    (
      -- Ignore ingestion-time FK linking (NULL -> value)
      NOT (OLD.patient_fk <=> NEW.patient_fk)
      AND NOT (OLD.patient_fk IS NULL AND NEW.patient_fk IS NOT NULL)
    ) OR
    (
      -- Ignore ingestion-time issuer FK linking (NULL -> value)
      NOT (OLD.accno_issuer_fk <=> NEW.accno_issuer_fk)
      AND NOT (OLD.accno_issuer_fk IS NULL AND NEW.accno_issuer_fk IS NOT NULL)
    ) OR
    NOT (OLD.study_custom1 <=> NEW.study_custom1) OR
    NOT (OLD.study_custom2 <=> NEW.study_custom2) OR
    NOT (OLD.study_custom3 <=> NEW.study_custom3)
  THEN
    INSERT INTO HIP_dirty_study (study_iuid, dirty_since, last_dirty_at, reason, source_table)
    VALUES (NEW.study_iuid, NOW(), NOW(), 'metadata_change', 'study')
    ON DUPLICATE KEY UPDATE
      last_dirty_at = NOW(),
      reason = 'metadata_change',
      source_table = 'study';
  END IF;
END$$

CREATE TRIGGER hip_dirty_study_u_series
AFTER UPDATE ON `series`
FOR EACH ROW
BEGIN
  DECLARE v_study_iuid VARCHAR(250) BINARY;

  IF
    NOT (OLD.series_desc <=> NEW.series_desc) OR
    NOT (OLD.series_attrs <=> NEW.series_attrs) OR
    NOT (OLD.modality <=> NEW.modality) OR
    NOT (OLD.institution <=> NEW.institution) OR
    NOT (OLD.series_no <=> NEW.series_no) OR
    NOT (OLD.series_custom1 <=> NEW.series_custom1) OR
    NOT (OLD.series_custom2 <=> NEW.series_custom2) OR
    NOT (OLD.series_custom3 <=> NEW.series_custom3)
  THEN
    SELECT s.study_iuid INTO v_study_iuid
    FROM `study` s
    WHERE s.pk = NEW.study_fk
    LIMIT 1;

    IF v_study_iuid IS NOT NULL THEN
      INSERT INTO HIP_dirty_study (study_iuid, dirty_since, last_dirty_at, reason, source_table)
      VALUES (v_study_iuid, NOW(), NOW(), 'metadata_change', 'series')
      ON DUPLICATE KEY UPDATE
        last_dirty_at = NOW(),
        reason = 'metadata_change',
        source_table = 'series';
    END IF;
  END IF;
END$$

CREATE TRIGGER hip_dirty_study_u_instance
AFTER UPDATE ON `instance`
FOR EACH ROW
BEGIN
  DECLARE v_study_iuid VARCHAR(250) BINARY;

  IF
    NOT (OLD.inst_attrs <=> NEW.inst_attrs) OR
    NOT (OLD.sop_cuid <=> NEW.sop_cuid) OR
    NOT (OLD.inst_no <=> NEW.inst_no) OR
    NOT (OLD.content_datetime <=> NEW.content_datetime) OR
    (
      -- Ignore ingestion-time AET population (NULL -> value)
      NOT (OLD.retrieve_aets <=> NEW.retrieve_aets)
      AND NOT (OLD.retrieve_aets IS NULL AND NEW.retrieve_aets IS NOT NULL)
    ) OR
    (
      NOT (OLD.ext_retr_aet <=> NEW.ext_retr_aet)
      AND NOT (OLD.ext_retr_aet IS NULL AND NEW.ext_retr_aet IS NOT NULL)
    ) OR
    NOT (OLD.inst_custom1 <=> NEW.inst_custom1) OR
    NOT (OLD.inst_custom2 <=> NEW.inst_custom2) OR
    NOT (OLD.inst_custom3 <=> NEW.inst_custom3)
  THEN
    SELECT st.study_iuid INTO v_study_iuid
    FROM `series` se
    INNER JOIN `study` st ON st.pk = se.study_fk
    WHERE se.pk = NEW.series_fk
    LIMIT 1;

    IF v_study_iuid IS NOT NULL THEN
      INSERT INTO HIP_dirty_study (study_iuid, dirty_since, last_dirty_at, reason, source_table)
      VALUES (v_study_iuid, NOW(), NOW(), 'metadata_change', 'instance')
      ON DUPLICATE KEY UPDATE
        last_dirty_at = NOW(),
        reason = 'metadata_change',
        source_table = 'instance';
    END IF;
  END IF;
END$$

DELIMITER ;

-- ---------------------------------------------------------------------------
-- Performance indexes (recommended)
-- ---------------------------------------------------------------------------
-- These indexes improve Sirius HIP query performance for /studyToken and /qido/studies.
-- They are safe to run multiple times (create-if-missing by index name).

SET @hip_db := DATABASE();

-- Pick latest files row per instance efficiently (MAX(pk) WHERE instance_fk = ?)
SET @hip_idx_exists := (
  SELECT COUNT(*)
  FROM information_schema.statistics
  WHERE table_schema = @hip_db
    AND table_name = 'files'
    AND index_name = 'hip_files_instance_pk'
);
SET @hip_sql := IF(
  @hip_idx_exists = 0,
  'CREATE INDEX hip_files_instance_pk ON files(instance_fk, pk)',
  'DO 0'
);
PREPARE hip_stmt FROM @hip_sql;
EXECUTE hip_stmt;
DEALLOCATE PREPARE hip_stmt;

-- Fast patient filters (patient.pat_id = ?)
SET @hip_idx_exists := (
  SELECT COUNT(*)
  FROM information_schema.statistics
  WHERE table_schema = @hip_db
    AND table_name = 'patient'
    AND index_name = 'hip_patient_pat_id'
);
SET @hip_sql := IF(
  @hip_idx_exists = 0,
  'CREATE INDEX hip_patient_pat_id ON patient(pat_id)',
  'DO 0'
);
PREPARE hip_stmt FROM @hip_sql;
EXECUTE hip_stmt;
DEALLOCATE PREPARE hip_stmt;

-- Common study filters (study_datetime range predicates)
SET @hip_idx_exists := (
  SELECT COUNT(*)
  FROM information_schema.statistics
  WHERE table_schema = @hip_db
    AND table_name = 'study'
    AND index_name = 'hip_study_study_datetime'
);
SET @hip_sql := IF(
  @hip_idx_exists = 0,
  'CREATE INDEX hip_study_study_datetime ON study(study_datetime)',
  'DO 0'
);
PREPARE hip_stmt FROM @hip_sql;
EXECUTE hip_stmt;
DEALLOCATE PREPARE hip_stmt;

-- Optional: speed up joins (usually already indexed in dcm4chee schemas)
SET @hip_idx_exists := (
  SELECT COUNT(*)
  FROM information_schema.statistics
  WHERE table_schema = @hip_db
    AND table_name = 'series'
    AND index_name = 'hip_series_study_fk'
);
SET @hip_sql := IF(
  @hip_idx_exists = 0,
  'CREATE INDEX hip_series_study_fk ON series(study_fk)',
  'DO 0'
);
PREPARE hip_stmt FROM @hip_sql;
EXECUTE hip_stmt;
DEALLOCATE PREPARE hip_stmt;

SET @hip_idx_exists := (
  SELECT COUNT(*)
  FROM information_schema.statistics
  WHERE table_schema = @hip_db
    AND table_name = 'instance'
    AND index_name = 'hip_instance_series_fk'
);
SET @hip_sql := IF(
  @hip_idx_exists = 0,
  'CREATE INDEX hip_instance_series_fk ON instance(series_fk)',
  'DO 0'
);
PREPARE hip_stmt FROM @hip_sql;
EXECUTE hip_stmt;
DEALLOCATE PREPARE hip_stmt;

-- ---------------------------------------------------------------------------
-- Optional quick verification helpers (run manually if desired)
-- ---------------------------------------------------------------------------
-- SHOW TRIGGERS LIKE 'study';
-- SELECT COUNT(*) FROM HIP_dirty_study;
