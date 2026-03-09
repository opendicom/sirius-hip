# How it works

Sirius HIP is designed to sit **in front of one or multiple PACS** and expose a consistent HTTP API that other systems can consume (e.g., RIS, portals, viewers, or integration middleware).

At runtime, Sirius HIP decides how to satisfy each request:

- Query metadata from the PACS database (QIDO-style searches).
- Serve DICOM bytes either from **filesystem** mappings (fast path) or by proxying upstream via **WADO-URI** (fallback).
- Generate viewer-oriented payloads (OHIF / Weasis / ZIP, etc.) using the `/studyToken` workflow.

## Multi-PACS model

Sirius HIP can be configured for different topologies:

- **Single PACS** (one backend).
- **Multiple PACS** (e.g., by institution, branch, or tenant).

The typical model is hierarchical:

- A **local PACS** per branch/site (studies produced locally).
- An optional **centralized PACS** per institution (aggregation / replication from branches).

This allows Sirius HIP to support scenarios where studies may exist in more than one location and where storage vs. retrieval strategy differs per site.

## Terminology

- **Backend PACS**: An upstream PACS system Sirius HIP integrates with (for example, dcm4chee).
- **Local PACS**: Branch/site-level PACS. Typically contains only studies produced at that site.
- **Centralized PACS**: Institution-level PACS that contains studies from multiple branches.
- **Custodian / Institution**: Tenant-like grouping used by some endpoints (e.g., `/custodians/...`).

## How study access works (high level)

Sirius HIP intentionally generates URLs that always hit Sirius HIP first, so it can enforce authorization and decide the retrieval strategy.

- `/studyToken` builds a viewer-specific response containing URLs.
- Downloads go through one of the `/files/...` endpoints:
	- Session-backed downloads: `GET /files/{session_id}/{file_index}`

See the deep-dive document: [doc/studyToken.md](studyToken.md).

## Metadata search (QIDO)

Study search is exposed via QIDO-style endpoints:

- `GET /qido/studies`

See: [doc/qido.md](qido.md).

## Diagrams

- [doc/sirius-hip_diagrams.drawio](sirius-hip_diagrams.drawio)