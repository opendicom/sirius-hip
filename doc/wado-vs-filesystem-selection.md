# WADO vs Filesystem Selection

This document explains how Sirius HIP selects the retrieval source for DICOM instances when handling `/studyToken`, and how the actual byte delivery happens in `/files/...` endpoints.

## Why there are two sources

Sirius HIP can retrieve a DICOM instance in two different ways:

- **Filesystem (FS)**: read the file from a local (or mounted) archive path.
- **WADO-URI**: fetch (proxy) the instance from the PACS WADO endpoint.

The goal is to **prefer the fastest and cheapest source** (filesystem) when it is safe, but still be **robust** during ingestion/migration windows or when the archive file is not available locally (fallback to WADO).

The selection algorithm is intentionally conservative: it uses filesystem only for studies that are within an explicit rollout window and are not marked as “dirty”. Otherwise it prefers WADO, which remains the authoritative retrieval path.

## High-level request flow

### 1) `/studyToken`

`/studyToken` is an orchestration endpoint. It:

1. Validates JWT (depending on `settings.jwt_auth`).
2. Queries the PACS database for the study/series/instances matching the request.
3. Prepares a response payload (OHIF/Weasis/Cornerstone) or a ZIP plan.
4. Optionally creates a **download session** (OneTime mode).

Important: `/studyToken` intentionally avoids filesystem I/O. It does not open DICOM files; it only prepares **URLs** and/or **sources** for later retrieval.

### 2) `/files/...`

All actual DICOM bytes are served through `/files/...` endpoints. These endpoints:

- **Try filesystem first** (only when a filesystem reference is available).
- **Fallback to WADO proxy** if the file is missing or cannot be opened.

This fallback is a key reliability feature.

## Where the FS vs WADO decision is made

The decision is primarily made in the **PACS SQL query** used by the study repository implementations. The repository returns per-instance rows with:

- `use_filesystem: bool`
- `filesystem_fk: Option<i32>`
- `relative_file_path: Option<String>`

These fields are precomputed in SQL so that `/studyToken` can be fast and predictable.

### When filesystem references are even selected

`/studyToken` only asks the repository for filesystem references when they can be used:

- The server must have filesystem mounts configured (`settings.dicomarchive.filesystems` not empty).
- The request must be in a mode that benefits from FS refs:
  - `JwtAuthMethod::OneTime` (session-backed downloads persist FS refs), or
  - `accessType=dicom.zip` (ZIP may prefer `file://...` sources), or
  - viewer-style responses (OHIF/Weasis/Cornerstone), where filesystem refs are embedded into local `/files/...` URLs/tokens so downloads can be **FS-first**.

## Current selection logic (as implemented today)

In the MySQL study repositories (dcm4chee 2.18.3 and 4.4.0), `use_filesystem` is computed in SQL using:

1. A **cutoff date** (`dicomarchive.filesystem_cutoff_date`) to force legacy studies to WADO.
2. A persistent **dirty signal** (`HIP_dirty_study`) to force corrected/normalized studies to WADO.

### Decision rules

Filesystem is eligible only when *all* of these are true:

- `settings.dicomarchive.filesystems` is configured (non-empty)
- `study.created_time >= settings.dicomarchive.filesystem_cutoff_date`
- The PACS database contains `HIP_dirty_study` and the study is **not** present in it

Otherwise, `use_filesystem = false` and Sirius HIP prefers WADO.

### Dirty table (`HIP_dirty_study`)

`HIP_dirty_study` is a small table (keyed by `study_iuid`, i.e. Study Instance UID / `(0020,000D)`) that acts as a **sticky** indicator that a study must be served via WADO.

It is meant to be populated by the DB triggers shipped in the `scripts/mysql/*_dirty_triggers.sql` scripts at the time a study is corrected.

- Once a study is dirty, later ingestion (e.g. adding a new series) does not erase the dirty signal.
- Triggers are **AFTER UPDATE** only (no INSERT triggers): inserts from normal ingestion must not mark a study dirty.

#### When triggers mark a study dirty

Triggers insert/update `HIP_dirty_study` when they observe **meaningful metadata changes**, such as:

- patient-level changes (patient is the top of the hierarchy: a patient change dirties all its studies)
- changes to descriptions/identifiers (study/series/instance fields)
- attribute-pointer changes (`*_attrs` in dcm4chee 2.18.3, `dicomattrs_fk` in dcm4chee 4.4.0)
- patient reassignment (patient FK changes)

#### When triggers do NOT mark a study dirty

To avoid false positives during ingestion, the scripts intentionally ignore certain ingestion-time “housekeeping” UPDATEs (typically **NULL → value** transitions performed after INSERTs).

- dcm4chee 2.18.3 ignores:
  - `study.patient_fk`: NULL → non-NULL
  - `study.accno_issuer_fk`: NULL → non-NULL
  - `instance.retrieve_aets`: NULL → non-NULL
- dcm4chee 4.4.0 ignores:
  - `study.patient_fk`: NULL → non-NULL
  - `instance.retrieve_aets`: NULL → non-NULL
  - `instance.ext_retr_aet`: NULL → non-NULL

Rationale: without these exceptions, normal ingestion would mark essentially every new study as dirty, and filesystem selection would never activate.

Note: the same columns still mark dirty on real corrections (e.g. value → different value, or value → NULL).

#### Operational behavior

When filesystem selection is enabled, Sirius HIP expects the dirty-triggers to exist and will **fail fast at startup** if the required triggers are missing (instead of silently falling back to WADO). Run the appropriate script for your PACS version to create the table and triggers.


## Configuration knobs

- `settings.dicomarchive.filesystem_cutoff_date` (required when `settings.dicomarchive.filesystems` is configured)
  - Interpreted as a **PACS local time** cutoff (MySQL `DATETIME`).
  - Use a date-only value (e.g. `2026-03-01`) to mean **local midnight**.
  - Do not include timezone suffixes like `Z` or offsets.
  - Studies created before this cutoff are forced to WADO.
  - Studies created on/after this cutoff can use filesystem when not marked dirty.
- `HIP_dirty_study` table in the PACS DB
  - Presence forces WADO for corrected/normalized studies.
  - Expected to be maintained by the `scripts/mysql/*_dirty_triggers.sql` triggers when filesystem selection is enabled.

### Practical meaning

- If `use_filesystem = true`, Sirius HIP can attempt `file://` access.
- If `use_filesystem = false`, Sirius HIP prefers WADO.

Note: even when filesystem is preferred, `/files/...` endpoints still *may* fall back to WADO if the filesystem read fails (file missing, mount not present, permissions). The selection logic only decides the **preferred** source.

Note: `use_filesystem` is decided **per instance** (row), but the cutoff date and dirty signal are **study-level** rules.
