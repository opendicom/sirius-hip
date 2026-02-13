# QIDO-RS (src2)

This document describes the current `/qido` implementation, focusing on the new optimized **src2** endpoint that was introduced.

## Endpoints

### `GET /qido/studies`

- Handler (src2): `src/src2/api/qido_handler.rs` (`qido_studies_handler`)
- Wired in: `src/main.rs` (route `/qido/studies` now points to the src2 handler)

The handler is intentionally thin (parsing + validation + DICOM JSON mapping). SQL is implemented in the PACS repository layer:

- Repository trait: `src/src2/pacs/repositories/study_repository.rs` (`StudyRepository::fetch_qido_studies_rows`)
- Read model: `src/src2/pacs/read_models/qido_study_row.rs` (`QidoStudyRow`)
- MySQL implementations:
   - `src/src2/pacs/infrastructure/dcm4chee2183/mysql/study_repository.rs`
   - `src/src2/pacs/infrastructure/dcm4chee440/mysql/study_repository.rs`

The other endpoints exist but are still not implemented:

- `GET /qido/series` → Not implemented
- `GET /qido/instances` → Not implemented
- Study-scoped paths (`/qido/studies/{StudyInstanceUID}/series`, etc.) remain NotImplemented.

## Query parameters

The src2 handler **reuses** the existing `QidoStudiesParams` struct for compatibility:

- `StudyDate` / `00080020`
- `StudyTime` / `00080030`
- `AccessionNumber` / `00080050`
- `ModalitiesInStudy` / `00080061`
- `ReferringPhysicianName` / `00080090`
- `PatientName` / `00100010`
- `PatientID` / `00100020`
- `StudyInstanceUID` / `0020000D` (supports multiple values separated by `\\`)
- `StudyID` / `00200010`

Plus:

- `includefield`: repeated key supported (querystring duplicate parsing)
- `limit`, `offset`
- `fuzzymatching` (currently ignored)
- `token` (JWT auth; see Token transport)

## Token transport

When `settings.jwt_auth` is `standard` or `onetime`, a valid JWT is required and can be sent either:

- Preferred: HTTP header `Authorization: Bearer <token>`
- Compatibility fallback: query parameter `token=...`

No other custom headers are supported for JWT.

### Wildcard matching

For string-like query parameters, Sirius HIP follows DICOM QIDO-RS wildcard semantics:

- `*` matches any sequence of characters (including empty)
- `?` matches any single character

This is implemented in SQL using `LIKE` (not regex). If you need partial matching, include `*` explicitly.

Examples:

- `PatientName=DOE^J*` (prefix match on given name)
- `ReferringPhysicianName=*SMITH*` (contains)
- `AccessionNumber=2026*`
- `StudyID=*ABC*`
- `ModalitiesInStudy=C*` (matches modalities in a backslash-separated list, e.g. `CT`, `CR`)

Notes:

- Parameters without `*`/`?` are treated as exact matches.
- `StudyInstanceUID` still supports multiple values separated by `\\` (exact UID values).

## Response format

The endpoint returns a JSON array of DICOM JSON objects (using `dicom-json` + `InMemDicomObject`).

It populates the required Study-level attributes from the QIDO-RS specification (minimal set), including:

- (0008,0005) Specific Character Set: `ISO_IR 100`
- (0008,0020) Study Date (normalized to DICOM `YYYYMMDD`)
- (0008,0030) Study Time (normalized to DICOM `HHMMSS` or `HHMM` depending on source)
- (0008,0050) Accession Number
- (0008,0056) Instance Availability: `ONLINE` (currently constant)
- (0008,0061) Modalities in Study
- (0008,0090) Referring Physician’s Name
- (0008,1190) Retrieve URL: empty string
- (0010,0010) Patient’s Name
- (0010,0020) Patient ID
- (0010,0030) Patient’s Birth Date (normalized)
- (0010,0040) Patient’s Sex
- (0020,000D) Study Instance UID
- (0020,0010) Study ID
- (0020,1206) Number of Study Related Series
- (0020,1208) Number of Study Related Instances

### `includefield`

Only a small allowlist is supported (validated via `QIDO_STUDY_INCLUDEFIELD_DIC`):

- `SOPClassesInStudy` / `00080062`
- `StudyDescription` / `00081030`
- `IssuerOfPatientID` / `00100021`

Notes:
- `00100021` (IssuerOfPatientID) depends on the PACS schema:
   - dcm4chee **4.4.0**: returned from `patient_id` → `issuer` when available.
   - dcm4chee **2.18.3**: returned from `patient.pat_id_issuer` (or empty if NULL).

## Implementation details (performance-oriented)

The goal of the src2 QIDO endpoint is to reduce overhead compared to the legacy implementation:

1. **Repository-owned SQL**.
   - The HTTP handler does not embed schema-specific SQL.
   - DB-version differences live in `StudyRepository` implementations.
2. **One row per study**.
   - Queries are written to avoid row explosion from joining `instance`/`file_ref`.
   - Pagination (`limit` / `offset`) is applied at the study level.
3. **No N+1 queries**.
   - All filters (including PatientID in dcm4chee440) are handled in SQL.
4. **Count strategy**.
   - dcm4chee2183: `num_series` / `num_instances` come from `study.num_series` / `study.num_instances`.
   - dcm4chee440: uses `study.num_series1` / `study.num_instances1`.
5. **DICOM formatting normalization**.
   - Dates/times are normalized by stripping non-digits and reformatting into DICOM `DA/TM`.

## Backend compatibility

The src2 endpoint supports both MySQL schemas via repository implementations:

- dcm4chee **2.18.3** MySQL
- dcm4chee **4.4.0** MySQL

Postgres repositories exist as stubs and return an `UnsupportedDatabase("postgres")` error for QIDO.

## Examples

### Search by PatientID

```bash
curl -G "http://localhost:5001/qido/studies" \
   -H "Authorization: Bearer ..." \
  -H "content-type: application/json" \
  --data-urlencode "PatientID=123" \
  --data-urlencode "limit=50"
```

### Include extra fields

```bash
curl -G "http://localhost:5001/qido/studies" \
   -H "Authorization: Bearer ..." \
  -H "content-type: application/json" \
  --data-urlencode "PatientID=123" \
  --data-urlencode "includefield=StudyDescription" \
  --data-urlencode "includefield=SOPClassesInStudy"
```

## Known limitations

- `/qido/series` and `/qido/instances` are not implemented in src2 yet.
- `RetrieveURL (0008,1190)` is returned as empty.
- `fuzzymatching` is currently ignored.
