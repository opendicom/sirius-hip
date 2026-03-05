-- Sirius HIP - Dirty study triggers (dcm4chee 4.4.0 / MySQL)
--
-- Purpose:
--   Maintain HIP_dirty_study (sticky dirty signal) when patients/studies/series/instances are UPDATED.
--   This supports Sirius HIP FS vs WADO selection (cutoff + dirty table).
--
-- How to run (example):
--   mysql -h <host> -u <user> -p <database> < scripts/mysql/dcm4chee440_dirty_triggers.sql
--
-- Notes:
--   - Requires privileges: CREATE TABLE, CREATE TRIGGER.
--   - Triggers are AFTER UPDATE only (no INSERT triggers).
--     Rationale: normal ingestion inserts new study/series/instance rows, and we do not want
--     new data to be considered "dirty" (dirty is meant to represent post-ingest corrections).
--   - Triggers are intentionally "strict": they mark dirty only on meaningful metadata changes
--     (e.g. descriptions, IDs, dicomattrs pointer changes on study/series/instance), not on
--     counter-only updates or updated_time alone.
--   - Note: `patient.dicomattrs_fk` may change multiple times during ingestion even when patient
--     demographics remain identical. We ignore `dicomattrs_fk` churn when the only other change
--     in the UPDATE is `updated_time`.
--   - dcm4chee performs ingestion-time "housekeeping" UPDATEs in some deployments (especially
--     NULL -> value transitions). Those should NOT mark the study dirty, otherwise every ingested
--     study would become dirty and filesystem selection would never activate.
--
--     Specifically, these ingestion-time transitions are ignored:
--       - study.patient_fk: NULL -> non-NULL
--       - instance.retrieve_aets: NULL -> non-NULL
--       - instance.ext_retr_aet: NULL -> non-NULL
--
--     Dirty is still marked when the same columns change in any other way:
--       - value -> different value (true reassignment/correction)
--       - value -> NULL
--
--   - INSERTs from normal ingestion do NOT mark dirty.
--   - dcm4chee 4 stores most DICOM attrs in dicomattrs. Most corrections update the owning row
--     (often by changing dicomattrs_fk). If in your deployment attrs are updated without touching
--     study/series/instance, we can add an optional dicomattrs trigger, but it's heavier.

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
DROP TRIGGER IF EXISTS hip_dirty_study_u_patient_id;
DROP TRIGGER IF EXISTS hip_dirty_study_u_person_name;
DROP TRIGGER IF EXISTS hip_dirty_study_u_study;
DROP TRIGGER IF EXISTS hip_dirty_study_u_series;
DROP TRIGGER IF EXISTS hip_dirty_study_u_instance;

DELIMITER $$

CREATE TRIGGER hip_dirty_study_u_patient
AFTER UPDATE ON `patient`
FOR EACH ROW
BEGIN
  IF
    NOT (OLD.no_pat_id <=> NEW.no_pat_id) OR
    NOT (OLD.pat_birthdate <=> NEW.pat_birthdate) OR
    NOT (OLD.pat_custom1 <=> NEW.pat_custom1) OR
    NOT (OLD.pat_custom2 <=> NEW.pat_custom2) OR
    NOT (OLD.pat_custom3 <=> NEW.pat_custom3) OR
    NOT (OLD.pat_sex <=> NEW.pat_sex) OR
    -- Patient merge is a meaningful correction (do not ignore NULL -> value)
    NOT (OLD.merge_fk <=> NEW.merge_fk) OR
    (
      -- `dicomattrs_fk` can churn during ingestion. Mark dirty only if more than
      -- (`dicomattrs_fk`, `updated_time`) changed, and still ignore NULL -> value linking.
      NOT (OLD.dicomattrs_fk <=> NEW.dicomattrs_fk)
      AND NOT (OLD.dicomattrs_fk IS NULL AND NEW.dicomattrs_fk IS NOT NULL)
      AND NOT (
        -- Only housekeeping: everything equal except updated_time and dicomattrs_fk
        (OLD.created_time <=> NEW.created_time) AND
        (OLD.no_pat_id <=> NEW.no_pat_id) AND
        (OLD.pat_birthdate <=> NEW.pat_birthdate) AND
        (OLD.pat_custom1 <=> NEW.pat_custom1) AND
        (OLD.pat_custom2 <=> NEW.pat_custom2) AND
        (OLD.pat_custom3 <=> NEW.pat_custom3) AND
        (OLD.pat_sex <=> NEW.pat_sex) AND
        (OLD.merge_fk <=> NEW.merge_fk) AND
        (OLD.pat_name_fk <=> NEW.pat_name_fk) AND
        NOT (OLD.updated_time <=> NEW.updated_time)
      )
    ) OR
    (
      -- Ignore ingestion-time name linking (NULL -> value)
      NOT (OLD.pat_name_fk <=> NEW.pat_name_fk)
      AND NOT (OLD.pat_name_fk IS NULL AND NEW.pat_name_fk IS NOT NULL)
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

CREATE TRIGGER hip_dirty_study_u_patient_id
AFTER UPDATE ON `patient_id`
FOR EACH ROW
BEGIN
  IF
    NOT (OLD.pat_id <=> NEW.pat_id) OR
    (
      -- Ignore ingestion-time issuer linking (NULL -> value)
      NOT (OLD.issuer_fk <=> NEW.issuer_fk)
      AND NOT (OLD.issuer_fk IS NULL AND NEW.issuer_fk IS NOT NULL)
    ) OR
    (
      -- Ignore ingestion-time patient linking (NULL -> value)
      NOT (OLD.patient_fk <=> NEW.patient_fk)
      AND NOT (OLD.patient_fk IS NULL AND NEW.patient_fk IS NOT NULL)
    )
  THEN
    IF OLD.patient_fk IS NOT NULL THEN
      INSERT INTO HIP_dirty_study (study_iuid, dirty_since, last_dirty_at, reason, source_table)
      SELECT st.study_iuid, NOW(), NOW(), 'patient_metadata_change', 'patient_id'
      FROM `study` st
      WHERE st.patient_fk = OLD.patient_fk
      ON DUPLICATE KEY UPDATE
        last_dirty_at = NOW(),
        reason = 'patient_metadata_change',
        source_table = 'patient_id';
    END IF;

    IF NEW.patient_fk IS NOT NULL THEN
      INSERT INTO HIP_dirty_study (study_iuid, dirty_since, last_dirty_at, reason, source_table)
      SELECT st.study_iuid, NOW(), NOW(), 'patient_metadata_change', 'patient_id'
      FROM `study` st
      WHERE st.patient_fk = NEW.patient_fk
      ON DUPLICATE KEY UPDATE
        last_dirty_at = NOW(),
        reason = 'patient_metadata_change',
        source_table = 'patient_id';
    END IF;
  END IF;
END$$

CREATE TRIGGER hip_dirty_study_u_person_name
AFTER UPDATE ON `person_name`
FOR EACH ROW
BEGIN
  IF
    (
      NOT (OLD.family_name <=> NEW.family_name)
      AND NOT (OLD.family_name IS NULL AND NEW.family_name IS NOT NULL)
    ) OR
    (
      NOT (OLD.given_name <=> NEW.given_name)
      AND NOT (OLD.given_name IS NULL AND NEW.given_name IS NOT NULL)
    ) OR
    (
      NOT (OLD.middle_name <=> NEW.middle_name)
      AND NOT (OLD.middle_name IS NULL AND NEW.middle_name IS NOT NULL)
    ) OR
    (
      NOT (OLD.name_prefix <=> NEW.name_prefix)
      AND NOT (OLD.name_prefix IS NULL AND NEW.name_prefix IS NOT NULL)
    ) OR
    (
      NOT (OLD.name_suffix <=> NEW.name_suffix)
      AND NOT (OLD.name_suffix IS NULL AND NEW.name_suffix IS NOT NULL)
    ) OR
    (
      NOT (OLD.i_family_name <=> NEW.i_family_name)
      AND NOT (OLD.i_family_name IS NULL AND NEW.i_family_name IS NOT NULL)
    ) OR
    (
      NOT (OLD.i_given_name <=> NEW.i_given_name)
      AND NOT (OLD.i_given_name IS NULL AND NEW.i_given_name IS NOT NULL)
    ) OR
    (
      NOT (OLD.i_middle_name <=> NEW.i_middle_name)
      AND NOT (OLD.i_middle_name IS NULL AND NEW.i_middle_name IS NOT NULL)
    ) OR
    (
      NOT (OLD.i_name_prefix <=> NEW.i_name_prefix)
      AND NOT (OLD.i_name_prefix IS NULL AND NEW.i_name_prefix IS NOT NULL)
    ) OR
    (
      NOT (OLD.i_name_suffix <=> NEW.i_name_suffix)
      AND NOT (OLD.i_name_suffix IS NULL AND NEW.i_name_suffix IS NOT NULL)
    ) OR
    (
      NOT (OLD.p_family_name <=> NEW.p_family_name)
      AND NOT (OLD.p_family_name IS NULL AND NEW.p_family_name IS NOT NULL)
    ) OR
    (
      NOT (OLD.p_given_name <=> NEW.p_given_name)
      AND NOT (OLD.p_given_name IS NULL AND NEW.p_given_name IS NOT NULL)
    ) OR
    (
      NOT (OLD.p_middle_name <=> NEW.p_middle_name)
      AND NOT (OLD.p_middle_name IS NULL AND NEW.p_middle_name IS NOT NULL)
    ) OR
    (
      NOT (OLD.p_name_prefix <=> NEW.p_name_prefix)
      AND NOT (OLD.p_name_prefix IS NULL AND NEW.p_name_prefix IS NOT NULL)
    ) OR
    (
      NOT (OLD.p_name_suffix <=> NEW.p_name_suffix)
      AND NOT (OLD.p_name_suffix IS NULL AND NEW.p_name_suffix IS NOT NULL)
    )
  THEN
    -- Patient name changes must dirty all studies for affected patients.
    INSERT INTO HIP_dirty_study (study_iuid, dirty_since, last_dirty_at, reason, source_table)
    SELECT st.study_iuid, NOW(), NOW(), 'patient_metadata_change', 'person_name'
    FROM `patient` p
    INNER JOIN `study` st ON st.patient_fk = p.pk
    WHERE p.pat_name_fk = NEW.pk
    ON DUPLICATE KEY UPDATE
      last_dirty_at = NOW(),
      reason = 'patient_metadata_change',
      source_table = 'person_name';
  END IF;
END$$

CREATE TRIGGER hip_dirty_study_u_study
AFTER UPDATE ON `study`
FOR EACH ROW
BEGIN
  IF
    NOT (OLD.study_desc <=> NEW.study_desc) OR
    NOT (OLD.dicomattrs_fk <=> NEW.dicomattrs_fk) OR
    NOT (OLD.study_id <=> NEW.study_id) OR
    NOT (OLD.accession_no <=> NEW.accession_no) OR
    NOT (OLD.access_control_id <=> NEW.access_control_id) OR
    (
      -- Ignore ingestion-time FK linking (NULL -> value)
      NOT (OLD.patient_fk <=> NEW.patient_fk)
      AND NOT (OLD.patient_fk IS NULL AND NEW.patient_fk IS NOT NULL)
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
    NOT (OLD.dicomattrs_fk <=> NEW.dicomattrs_fk) OR
    NOT (OLD.modality <=> NEW.modality) OR
    NOT (OLD.institution <=> NEW.institution) OR
    NOT (OLD.series_no <=> NEW.series_no) OR
    NOT (OLD.body_part <=> NEW.body_part) OR
    NOT (OLD.laterality <=> NEW.laterality) OR
    NOT (OLD.series_custom1 <=> NEW.series_custom1) OR
    NOT (OLD.series_custom2 <=> NEW.series_custom2) OR
    NOT (OLD.series_custom3 <=> NEW.series_custom3)
  THEN
    SELECT st.study_iuid INTO v_study_iuid
    FROM `study` st
    WHERE st.pk = NEW.study_fk
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
    NOT (OLD.dicomattrs_fk <=> NEW.dicomattrs_fk) OR
    NOT (OLD.sop_cuid <=> NEW.sop_cuid) OR
    NOT (OLD.inst_no <=> NEW.inst_no) OR
    (
      -- Ignore ingestion-time AET population (NULL -> value)
      NOT (OLD.retrieve_aets <=> NEW.retrieve_aets)
      AND NOT (OLD.retrieve_aets IS NULL AND NEW.retrieve_aets IS NOT NULL)
    ) OR
    (
      -- Ignore ingestion-time external retrieve AET population (NULL -> value)
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

-- Pick latest file_ref row per instance efficiently (MAX(pk) WHERE instance_fk = ?)
SET @hip_idx_exists := (
  SELECT COUNT(*)
  FROM information_schema.statistics
  WHERE table_schema = @hip_db
    AND table_name = 'file_ref'
    AND index_name = 'hip_file_ref_instance_pk'
);
SET @hip_sql := IF(
  @hip_idx_exists = 0,
  'CREATE INDEX hip_file_ref_instance_pk ON file_ref(instance_fk, pk)',
  'DO 0'
);
PREPARE hip_stmt FROM @hip_sql;
EXECUTE hip_stmt;
DEALLOCATE PREPARE hip_stmt;

-- Fast patient_id filtering and EXISTS checks
SET @hip_idx_exists := (
  SELECT COUNT(*)
  FROM information_schema.statistics
  WHERE table_schema = @hip_db
    AND table_name = 'patient_id'
    AND index_name = 'hip_patient_id_patient_fk_pat_id'
);
SET @hip_sql := IF(
  @hip_idx_exists = 0,
  'CREATE INDEX hip_patient_id_patient_fk_pat_id ON patient_id(patient_fk, pat_id)',
  'DO 0'
);
PREPARE hip_stmt FROM @hip_sql;
EXECUTE hip_stmt;
DEALLOCATE PREPARE hip_stmt;

-- Optimize MIN(pk) per patient_fk (pid_first join)
SET @hip_idx_exists := (
  SELECT COUNT(*)
  FROM information_schema.statistics
  WHERE table_schema = @hip_db
    AND table_name = 'patient_id'
    AND index_name = 'hip_patient_id_patient_fk_pk'
);
SET @hip_sql := IF(
  @hip_idx_exists = 0,
  'CREATE INDEX hip_patient_id_patient_fk_pk ON patient_id(patient_fk, pk)',
  'DO 0'
);
PREPARE hip_stmt FROM @hip_sql;
EXECUTE hip_stmt;
DEALLOCATE PREPARE hip_stmt;

-- Common study filters (study_date range predicates)
SET @hip_idx_exists := (
  SELECT COUNT(*)
  FROM information_schema.statistics
  WHERE table_schema = @hip_db
    AND table_name = 'study'
    AND index_name = 'hip_study_study_date'
);
SET @hip_sql := IF(
  @hip_idx_exists = 0,
  'CREATE INDEX hip_study_study_date ON study(study_date)',
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
