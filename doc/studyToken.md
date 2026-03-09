# StudyToken (src2)

This document describes the current `/studyToken` implementation in the **src2** flow and how it interacts with the download endpoints.

## Endpoints

### `GET /studyToken`

Resolves a study query and returns a viewer-specific payload (JSON, XML, or ZIP stream) depending on `accessType`.

- Handler: `src/src2/api/study_token_handler.rs`
- Use case: `src/src2/application/use_cases/study_token.rs`

#### Query parameters

The request is parsed using `StudyTokenParams`:

- Operation-level:
  - `token`: required JWT token. Preferred via `Authorization: Bearer <token>`, with a querystring fallback `?token=...`.
  - `session`: optional, passed through to some viewer outputs
  - `institution`: optional tenant filter
  - `proxyURI`: optional base URL used to build absolute download URLs
  - `accessType`: required. Supported values:
    - `ohif`
    - `weasis.xml`
    - `dicom.zip`
  - `max`: optional limit (defaults to `settings.max_default` when missing/0)

- Patient-level:
  - `PatientID`
  - `patient` (regex-style patient name search)

- Study-level:
  - `StudyInstanceUID`
  - `AccessionNumber`
  - `StudyID`
  - `StudyDate` (supports `YYYY-MM-DD`, `YYYY-MM-DD|`, `|YYYY-MM-DD`, `YYYY-MM-DD|YYYY-MM-DD`)
  - `ModalityInStudy`
  - `cuidsInStudy`

- Series-level:
  - `SeriesInstanceUID`
  - `SeriesNumber`
  - `SeriesDescription`
  - `Modality`
  - `ModalityOff`
  - `SOPClass`
  - `SOPClassOff`

## Auth modes and URL strategy

`settings.jwt_auth` controls the enforcement level:

### 1) `standard`

- A valid JWT is required and validated (see Token transport).
- `/studyToken` creates a **download session** in the *application DB*.
- Viewer payloads use **session-backed** URLs:
  - `GET /files/{session_id}/{file_index}`

### 2) `onetime`

- A valid JWT is required and validated (see Token transport).
- `/studyToken` creates a **download session** in the *application DB*.
- Viewer payloads use **session-backed** enforced URLs:
  - `GET /files/{session_id}/{file_index}`

## Token transport

When `settings.jwt_auth` is `standard` or `onetime`, the JWT can be sent either:

- Preferred: HTTP header `Authorization: Bearer <token>`
- Compatibility fallback: query parameter `token=...`

No other custom headers are supported for JWT.

The key design requirement is that **generated URLs always hit an application endpoint**. The application decides at request time whether to serve bytes from the filesystem or proxy from WADO-URI.

Performance note: `/studyToken` rendering performs **no filesystem I/O**. It only uses the PACS database rows (`inst_attrs` and other columns). If some DICOM tags are missing from those blobs, the corresponding fields in the viewer payload are left null/empty.

> Note: `settings.jwt_auth` is deserialized with `lowercase` names, so valid values in config are `standard`, `onetime`.

## Download endpoints (byte serving)

### `GET /files/{session_id}/{file_index}` (session-backed)

- Handler: `src/src2/api/files_handler.rs` (`download_file_handler`)
- In `onetime` mode:
  - First performs an **atomic claim** (`claim_file(session_id, file_index)`).
  - If the file has already been claimed, the request fails (download-once semantics).
- In `standard` mode:
  - Fetches the file metadata without claiming; repeated downloads are allowed until expiration.

Resolution logic:

1. Attempt filesystem open if `(filesystem_fk, relative_file_path)` exists.
2. Otherwise (or if missing), proxy WADO-URI.

## OneTime persistence model (application DB)

OneTime is enforced with a **claim table** to avoid session-row locking.

### Tables

Created at startup by the MySQL repository:

- `HIP_download_sessions`
  - `session_id` (PK)
  - `expires_at`
  - `total_files`
  - `token_hash` (SHA-256 of the JWT that created the session)
  - `created_at`

- `HIP_download_session_files`
  - PK: `(session_id, file_index)`
  - UIDs: `study_uid`, `series_uid`, `instance_uid`
  - `use_wado` (hint)
  - filesystem reference:
    - `filesystem_fk` (INT)
    - `relative_file_path` (TEXT)

- `HIP_download_session_claims`
  - PK: `(session_id, file_index)`
  - `claimed_at`
  - FK to `HIP_download_session_files` with `ON DELETE CASCADE`

### Claim algorithm

- Fast path: `INSERT INTO download_session_claims ... SELECT ...` guarded by:
  - session exists
  - session not expired
  - file exists
  - primary key uniqueness enforces download-once
- Duplicate key (`1062`) is mapped to a “already downloaded” error.

### Large sessions (5k–10k files)

The repository uses chunked inserts to avoid huge SQL statements:

- `add_files()` inserts in chunks of 500 rows.

### Strict ZIP semantics

When `accessType=dicom.zip` and `jwt_auth=onetime`, the server **consumes the session up-front** by inserting claims for all files.

This makes “one-time” semantics strict even if the ZIP download is interrupted.

## Filesystem references

To minimize DB storage and avoid persisting absolute paths:

- OneTime stores `(filesystem_fk, relative_file_path)`.
- Absolute path is reconstructed at request time:

```text
abs_path = settings.dicomarchive.filesystems[filesystem_fk].path + "/" + relative_file_path
```

If the filesystem reference is missing or the file cannot be opened, the server falls back to WADO-URI proxying.

## WADO proxying

- The app uses `settings.dicomarchive.wadouri` and `settings.dicomarchive.transfer_syntax`.
- The WADO URL is constructed from UIDs (no persisted per-file WADO URL in OneTime tables).

## Example requests

### OHIF manifest (Standard)

```bash
curl -G "http://localhost:5001/studyToken" \
  --data-urlencode "accessType=ohif" \
  --data-urlencode "PatientID=123" \
  --data-urlencode "max=200" \
  -H "Authorization: Bearer ..."
```

### OHIF manifest (OneTime)

```bash
curl -G "http://localhost:5001/studyToken" \
  --data-urlencode "accessType=ohif" \
  --data-urlencode "PatientID=123" \
  -H "Authorization: Bearer ..."
```

The response will contain URLs of the form:

- `.../files/{session_id}/{file_index}` (OneTime)
- `.../files/{session_id}/{file_index}` (Standard)

## Operational notes / caveats

- The application DB schema is created with `CREATE TABLE IF NOT EXISTS ...`.
  - Existing tables are reused.
  - The app DB is configured via `settings.app_database_url` (separate from the PACS DB in `settings.dicomarchive.database_url`).

- **Automatic cleanup (MySQL app DB)**
  - A background job periodically purges OneTime persistence rows older than a cutoff.
  - Defaults: **24 hours after expiration**, run every **300s**.
  - In multi-instance deployments, cleanup is guarded with MySQL `GET_LOCK` so only one instance performs deletions at a time.
  - Config is provided via the `onetime_cleanup` table in the main TOML:

    ```toml
    [onetime_cleanup]
    enabled = true
    interval_secs = 300
    retention_hours = 24
    session_batch = 200
    max_batches = 20
    token_delete_limit = 5000
    initial_jitter_max_secs = 60
    ```
