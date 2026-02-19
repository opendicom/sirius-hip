# WADO vs Filesystem selection (src2)

This document explains how Sirius HIP selects the retrieval source for DICOM instances when handling `/studyToken`, and how the actual byte delivery happens in `/files/...`.

## Why there are two sources

Sirius HIP can retrieve a DICOM instance in two different ways:

- **Filesystem (FS)**: read the file from a local (or mounted) archive path.
- **WADO-URI**: fetch (proxy) the instance from the PACS WADO endpoint.

The goal is to **prefer the fastest and cheapest source** (filesystem) when it is safe, but still be **robust** during ingestion/migration windows or when the archive file is not available locally (fallback to WADO).

In addition, the selection uses a *stability heuristic* based on database timestamps: **`created_time` vs `updated_time`**.
Some DICOM-related values can be changed/normalized in the PACS database without immediately producing a finalized, merged on-disk representation. When we detect a large divergence between `created_time` and `updated_time`, we assume the entity was modified “in DB” and we prefer **WADO**, so the backend PACS can serve the authoritative object after applying its own merge/overlay rules.
These updates can happen at **study**, **series**, or **instance** level.

In other words:

- FS is used when the PACS state looks *stable* (small `created_time`/`updated_time` delta at study/series/instance) and a valid file reference exists.
- WADO is used otherwise, because it is the authoritative retrieval method when the filesystem reference might be stale, incomplete, missing, or when DB-level updates require the PACS to build the final merged content.

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

The decision is primarily made in the **PACS SQL query** used by the src2 study repository implementations. The repository returns per-instance rows with:

- `use_filesystem: bool`
- `filesystem_fk: Option<i32>`
- `relative_file_path: Option<String>`

These fields are precomputed in SQL so that `/studyToken` can be fast and predictable.

### When filesystem references are even selected

`/studyToken` only asks the repository for filesystem references when they can be used:

- The server must have filesystem mounts configured (`settings.dicomarchive.filesystems` not empty).
- The request must be in a mode that benefits from FS refs:
  - `JwtAuthMethod::OneTime` (session-backed downloads may persist FS refs), or
  - `accessType=dicom.zip` (ZIP may prefer `file://...` sources).

For viewer-style responses (OHIF/Weasis/Cornerstone), filesystem refs are not required during `/studyToken` rendering.

## Current selection logic (as implemented today)

In the MySQL study repositories (dcm4chee 2.18.3 and 4.4.0), `use_filesystem` is computed by an SQL expression that requires *all* of the following:

1. **Stability window checks**

The row is considered filesystem-eligible only if:

- `ABS(TIMESTAMPDIFF(SECOND, study.created_time, study.updated_time)) <= 600`
- `ABS(TIMESTAMPDIFF(SECOND, series.created_time, series.updated_time)) <= 600`
- `ABS(TIMESTAMPDIFF(SECOND, instance.created_time, instance.updated_time)) <= 600`

This is effectively a “stability window” heuristic: if an object’s `updated_time` diverges too much from its `created_time`, the object is treated as not stable enough to safely rely on a direct filesystem reference.

2. **A valid file reference exists**

The repository requires a picked file reference (`file_ref` / `files_pick`) with:

- non-null `filepath`
- non-null `filesystem_fk`

If any of these are missing, `use_filesystem` is false.

### Practical meaning

- If `use_filesystem = true`, Sirius HIP can attempt `file://` access.
- If `use_filesystem = false`, Sirius HIP prefers WADO.

Note: `use_filesystem` is decided **per instance** (row), not per study.

## How `/studyToken` uses this decision

### Session-backed downloads (JWT OneTime)

In `JwtAuthMethod::OneTime`, `/studyToken` creates a download session and persists one row per instance, including:

- `use_wado = !use_filesystem`
- filesystem reference only when `use_filesystem` is true

Later, `/files/{session_id}/{file_index}` uses the persisted data to serve the file.

### Stateless downloads (JWT Standard / None)

When not in OneTime mode, `/studyToken` can create a **signed download token** for each instance, embedding the filesystem reference (only if present). Clients download via:

- `/files/{token}`

The `/files/{token}` handler tries filesystem first if the token contains a filesystem reference; otherwise it proxies WADO.

### ZIP (`accessType=dicom.zip`)

For ZIP responses, `/studyToken` builds a per-instance source list:

- If `use_filesystem` is true and the filesystem path can be built: source is `file://<absolute path>`.
- Otherwise: source is the PACS WADO URL.

In OneTime mode, ZIP sources are derived from the persisted session file list to keep the output consistent with what will be downloadable.

## How `/files` performs FS-first with WADO fallback

Both download endpoints implement the same strategy:

1. If a filesystem reference exists (`filesystem_fk` + `relative_file_path`):
   - Build an absolute path using `settings.dicomarchive.get_fs_path_by_id(filesystem_fk)`.
   - Try opening the file and stream it.
2. If the file cannot be opened (missing, permissions, bad path, etc.):
   - Build a WADO URL using `settings.dicomarchive.wadouri` + UIDs.
   - Proxy the WADO response back to the client.

This means the system remains functional even if:

- file refs are stale,
- the filesystem mount is temporarily unavailable,
- or the archive has not finished moving files to the final location.

## Configuration knobs

- `settings.dicomarchive.wadouri`: PACS WADO-URI base URL.
- `settings.dicomarchive.transfer_syntax`: transfer syntax used for WADO requests.
- `settings.dicomarchive.filesystems`: list of filesystem roots; each has an `id` and a `path`.
- (Implicit) the “stability window” constant is currently hardcoded in SQL as `600` seconds.

## Known limitation / future improvement

The current heuristic is based on `created_time` vs `updated_time` deltas and does not incorporate cross-entity comparisons (e.g. study vs series activity).

A planned improvement (see TODO) is to make a *study-level* decision such as:

- “Prefer WADO when `study.updated_time + WINDOW_TIME` is newer than the `updated_time` of any series in the study.”

That approach can better capture “recent updates” at the study/series level, even when created/updated deltas do not.
