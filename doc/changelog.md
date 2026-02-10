# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project adheres to Semantic Versioning.

## [Unreleased]

## [1.1.0] - 2026-02-10
### Added
- New optimized QIDO-RS endpoint for studies: **`/qido/studies`**
  - Validates query parameters and formats responses as DICOM JSON.
  - Supports **`includefield`** with validation against an allowlist.
- Dataset overrides for metadata in study queries
  - New **`metadata_overrides`** configuration in `sirius-hip.toml`.
  - Allows customizing patient/study attributes returned by queries.
- One-time JWT tokens for **`/studyToken`** sessions
  - Enforces one-time token semantics (claim-once) and improves validation/errors.
- Documentation for QIDO, StudyToken, and dataset overrides.
- Reference database schemas for dcm4chee 2.18.3 and 4.4.0.

### Changed
- `InstitutionName` handling for study queries and OHIF models.
- QIDO and StudyToken flows refactored into functional use-cases (`src2/application/use_cases`).
- StudyToken URL builder updated to work with the updated token/session flow.

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