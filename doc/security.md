# Security Model (JWT auth, download sessions, and data access)

This document explains how Sirius HIP protects access to DICOM data across the main endpoints, focusing on:

- JWT-based authorization (`jwt_auth` modes)
- Download sessions (`download_session`) persisted in the application DB
- One-time semantics and per-file/per-instance claiming
- How `/studyToken`, `/qido/studies`, `/files`, and `/wado` enforce access rules

## Concepts

### 1) Main JWT (viewer/session token)

Clients call endpoints like `/studyToken`, `/qido/studies`, `/files/{session}/{index}`, and `/wado` using a JWT.

Validation rules:

- Signature is validated using `jwt_secret` and `jwt_algorithm`.
- Expiration (`exp`) is enforced.
- Audience (`aud`) must match one of the allowed values.

Intended audience:

- `aud = "sirius-hip"`

Note:

- The current code also accepts `aud = "wezen"`. If you don’t want that in production, restrict it in `auth::validate_jwt_token`.

Token transport:

- Preferred: `Authorization: Bearer <jwt>`
- Supported for some endpoints: `?token=<jwt>`


### 2) Download session (application DB state)

A `download_session` is a persisted snapshot of the allowed downloadable objects for a viewer request.

A session includes:

- `session_id` (public ID, UUID)
- `expires_at` (derived from the JWT `exp`)
- `total_files`
- the list of allowed files, indexed by `file_index`
- **token binding**: `token_hash` (SHA-256 of the main JWT that created the session)

The session and its files are stored in the **application DB** (separate from PACS DB).

Why sessions exist:

- Enforce one-time downloads (OneTime mode)
- Resolve filesystem-first delivery without querying PACS DB on every instance request
- Provide a fast “allowlist” for `/files` and `/wado`

---

## Configuration: `jwt_auth`

`jwt_auth` controls the enforcement level.

### `jwt_auth = "standard"`

Goal: require a valid JWT and bind downloads to the exact token that created the session.

Properties:

- `/studyToken` validates the JWT.
- `/studyToken` creates a `download_session` for viewer access types (OHIF, Weasis).
- `/files/{session}/{index}` and `/wado` require the JWT and verify it matches the session (`token_hash`).
- “One-time” behavior is **not** enabled; downloads can be repeated until `expires_at`.

### `jwt_auth = "onetime"`

Goal: strictly prevent token replay for “planning endpoints” and enforce one-time downloads.

Properties:

- `/studyToken` validates the JWT.
- `/studyToken` **claims the token** so it cannot be reused.
- `/qido/studies` validates the JWT and **claims the token** so it cannot be reused.
- `/files/{session}/{index}` requires the JWT and verifies session binding AND enforces one-time claiming per file.
- `/wado` requires the JWT and verifies session binding AND enforces one-time claiming per `(instance_uid, contentType)`.

Important nuance:

- The token is “consumed” for `/studyToken` and `/qido/studies` to block replays, but the same JWT remains usable for downloads **for its bound session** until expiration.

---

## Endpoint behavior (active `src2` routes)

### 1) `/studyToken`

Purpose:

- Validate the main JWT.
- Query PACS for the requested study/series/instance rows.
- Create a `download_session` in the application DB.
- Return a manifest in the requested `accessType`.

Token input:

- `Authorization: Bearer <jwt>` (preferred)
- `?token=<jwt>` (supported)

Standard vs OneTime:

- Standard:
  - JWT validated.
  - Session created with `token_hash` binding.
- OneTime:
  - JWT validated.
  - Token is checked/claimed to prevent reusing it for `/studyToken`.
  - Session created with `token_hash` binding.

Manifests:

- OHIF (`accessType=ohif`): returns JSON containing retrieval URLs.
  - Session-backed URLs: `/files/{session_id}/{file_index}?token=<jwt>`
- Weasis (`accessType=weasis.xml`): returns XML pointing to `/wado`.
  - XML includes `session=<session_id>` and `token=<jwt>`
- ZIP (`accessType=dicom.zip`): streams a ZIP response.
  - In OneTime mode, the session is consumed up-front to keep one-time semantics strict.
  - In Standard mode, ZIP is streamed directly (no additional one-time enforcement).

### 2) `/qido/studies`

Purpose:

- Provide QIDO SearchForStudies.

Standard:

- Validates JWT.

OneTime:

- Validates JWT.
- Claims the token early (before PACS queries) to prevent replay.

### 3) `/files/{session_id}/{file_index}`

Purpose:

- Download a specific file (by session + index).
- Resolve filesystem-first when possible.
- Fall back to upstream PACS WADO when needed.

Authorization:

- Requires the main JWT in **both** `standard` and `onetime`.
- Validates JWT and then enforces **token ↔ session binding** (`token_hash`).

Enforcement:

- Standard:
  - Uses session metadata (`get_file`) to find the allowed object.
  - No one-time claim; repeated downloads are allowed until expiration.
- OneTime:
  - Atomically claims the `(session_id, file_index)` using `HIP_download_session_claims`.
  - If a file was already claimed, the endpoint rejects it.

Filesystem vs WADO fallback:

- If the session file includes a filesystem reference, Sirius HIP attempts to open and stream it.
- If the file does not exist (or has no FS reference), Sirius HIP proxies the upstream WADO request.

### 4) `/wado`

Purpose:

- Secure WADO-URI proxy endpoint.
- Resolve filesystem-vs-WADO source using `download_session`.

Required inputs:

- Main JWT (`Authorization: Bearer` or `?token=`)
- `session=<session_id>`
- WADO params including `objectUID=<SOPInstanceUID>`

Authorization:

- Validates JWT.
- Enforces **token ↔ session binding** (`token_hash`).
- Uses the session allowlist to locate the instance by UID.

OneTime enforcement (Weasis-friendly):

- In OneTime mode, `/wado` atomically claims by `(session_id, instance_uid, content_type)`.
- This allows Weasis to request the same SOP multiple times with different `contentType` values (e.g. `application/dicom` and `image/jpeg`).

Data delivery:

- For `contentType=application/dicom`, filesystem-first is attempted.
- For rendered outputs (e.g. `image/jpeg`), Sirius HIP always proxies upstream (the PACS renders).

Privacy:

- Internal parameters `token` and `session` are stripped before forwarding to the upstream PACS.

---

## Database tables (application DB)

The MySQL repository creates these tables if missing:

- `HIP_download_sessions`
  - includes `token_hash` (SHA-256 of the creating JWT)
- `HIP_download_session_files`
  - per file allowlist rows (by `file_index`)
- `HIP_download_session_claims`
  - one-time claims for `/files/{session}/{index}`
- `HIP_download_session_wado_claims`
  - one-time claims for `/wado` by `(instance_uid, content_type)`
- `HIP_one_time_tokens`
  - hashes of consumed JWTs to prevent replay

---

## Security invariants (what the system guarantees)

With `jwt_auth = standard`:

- A request without a valid JWT cannot access data.
- Knowing a `session_id` is **not sufficient** to download; the JWT must match the session binding.
- Downloads can be repeated until session expiration.

With `jwt_auth = onetime`:

- `/studyToken` and `/qido/studies` reject replays of the same JWT.
- Downloads are tied to a session and a token.
- `/files/{session}/{index}` enforces strict one-time download per file index.
- `/wado` enforces one-time per `(SOPInstanceUID, contentType)`.

---

## Operational notes

- Query-string tokens (`?token=`) may appear in logs, caches, and referrers. Prefer `Authorization` when clients allow it and redact tokens in access logs.
- If you change any backend implementation of `DownloadSessionRepository`, it must implement session-token binding. The trait defaults to fail closed.
