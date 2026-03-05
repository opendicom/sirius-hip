# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project adheres to Semantic Versioning.

## [Unreleased]

## [1.3.0] - 2026-03-02
### Changed
- FS vs WADO selection is now consistently **FS-first** for viewer-style clients (OHIF/Weasis/Cornerstone) when eligible (post-cutoff and not dirty), regardless of `jwt_auth` mode.
- `dicomarchive.filesystem_cutoff_date` is interpreted as a **PACS-local** date/datetime (no timezone suffixes), to match PACS MySQL `DATETIME` semantics.
- Guardrail in PACS SQL: `use_filesystem` is only true when filesystem references are present (filesystem id + non-empty relative path), avoiding hard failures due to inconsistent PACS file reference data.

### Removed
- Removed the legacy timestamp-based FS vs WADO stability heuristic and its configuration knob.
  - Filesystem selection now uses `dicomarchive.filesystem_cutoff_date` + a persistent dirty signal (`HIP_dirty_study`) in the PACS DB.

## [1.2.0] - 2026-02-20
### Added
- Startup index bootstrap for MySQL study repositories (dcm4chee 2.18.3 and 4.4.0):
  - `series(study_fk, updated_time)`
  - `instance(series_fk, updated_time)`
  - Indexes are created only when missing.

### Changed
- FS vs WADO selection heuristic study repositories updated to cross-entity stability checks:
  - Study-level propagation to all series/instances.
  - Series-level propagation to all instances.
  - Instance-level stability check based on `updated_time` vs `created_time` plus window.
- SQL selection logic optimized for performance:
  - Replaced `MAX(...)` patterns with correlated `EXISTS` checks.
  - Reordered predicates for cheaper short-circuit evaluation.
  - Removed repeated evaluation of the stability expression in SELECT projections.
- Study repository factory now initializes MySQL repositories asynchronously to support startup index/bootstrap tasks.

### Documentation
- Updated FS/WADO selection documentation.

## [1.1.0] - 2026-02-16
### Added
- New optimized QIDO-RS endpoint for studies: **`/qido/studies`**
  - Validates query parameters and formats responses as DICOM JSON.
  - Supports **`includefield`** with validation against an allowlist.
- DICOM wildcard support in QIDO filters
  - Supports `*` (any sequence) and `?` (single char) via MySQL `LIKE ... ESCAPE`.
- Metadata overrides for study query attributes
  - New **`metadata_overrides`** configuration in `sirius-hip.toml`.
  - Allows customizing patient/study attributes returned by queries.
  - Entries use `keyword` and `source`.
- One-time JWT tokens for **`/studyToken`** sessions
  - Enforces one-time token semantics (claim-once) and improves validation/errors.
- Documentation for QIDO and StudyToken.
- Reference database schemas for dcm4chee 2.18.3 and 4.4.0.
- Static URL builders served by nginx
  - StudyToken builder: `/urlbuilder/study-token.html`
  - QIDO studies builder: `/urlbuilder/qido-studies.html`

### Changed
- `InstitutionName` handling for study queries and OHIF models.
- QIDO and StudyToken flows refactored into functional use-cases (`src2/application/use_cases`).
- StudyToken URL builder updated to work with the updated token/session flow.
- QIDO studies SQL significantly optimized (pagination-first + fewer correlated subqueries).
- dcm4chee 2.18.3 and 4.4.0 adapters aligned for QIDO search semantics and returned attributes.
- Docker image now ships the full `www/` static tree for nginx.

### Removed
- Legacy URL builder paths: `/studyToken/urlbuilder` and `/qido/urlbuilder`.

## [1.0.2] - 2025-10-02
### Added
- **`includefield`** query parameter in the QIDO implementation
  - Supported values:
    - 00080062/SOPClassesInStudy
    - 00081030/StudyDescription
    - 00100021/IssuerOfPatientID

### Changed
- Add context to errors in the QIDO protocol implementation for DB dcm4chee440.

## [1.0.0-beta.15] - 2025-01-22
### Added
- Support for viewing colored DICOM images in OHIF.

## [1.0.0-beta.14] - 2025-01-22
### Added
- SeriesDescription added to OHIF DICOM JSON Datasource.
- `SIRIUS_HIP_MAX_DEFAULT` environment variable configurable via Docker.

### Fixed
- studyToken urlbuilder website self hosting.
- studyToken urlbuilder website CSS Dark/Light theme.

## [1.0.0-beta.12] - 2024-04-05
### Changed
- The studyToken urlbuilder website is no longer hosted on Sirius HIP, but on nginx in the Docker image.
- Database field where the institution name is stored can now be configured from the configuration file or environment variable for a docker setup.

### Fixed
- Docker image shows error when hosting studyToken urlbuilder web page.
- Docker image failed to start when some env vars were configured.

## [1.0.0-beta.11] - 2023-12-27
### Added
- StudyToken url builder tool (`/studyToken/urlbuilder`).
- Default port changed from `6001` to `5001`.

## [1.0.0-beta.10] - 2023-12-27
### Changed
- WADO proxy implementation.

### Fixed
- Docker image changed from alpine-3 to debian:bookworm to fix a bug in the wado proxy endpoint.