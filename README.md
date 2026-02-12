![Header](doc/resources/logo.png)

# Sirius HIP
Health Integration Platform for multi-PACS environments

Sirius HIP is a PACS-centric integration platform that connects one or multiple PACS (including open-source PACS) with RIS and other healthcare systems.

> One platform. Multiple PACS. Total integration.

## Key Features

- High-performance REST API built in [Rust](https://www.rust-lang.org/)
- Proprietary endpoints for integration workflows (sessions, URL building, settings discovery)
- DICOMweb entrypoints for study search and related workflows
- JWT-based authorization (configurable modes)
- Multi-backend support via PACS “adapters”

## Architecture (high level)

- **HTTP API** (Actix-web) exposes proprietary + DICOMweb-style endpoints
- **PACS adapters** encapsulate backend-specific logic (e.g., dcm4chee versions)
- **Persistence** (MySQL via SQLx) for application sessions and download workflows
- **Token-driven study access** via `/studyToken` for viewer interoperability

## Supported PACS backends

- dcm4chee v2.18.3
- dcm4chee-arc v4.4.0

## Supported standards / APIs

- DICOMweb: **QIDO-RS** via `GET /qido`
- PACS URL discovery helpers for WADO/STOW per PACS (see proprietary methods)

## Documentation

- How it works: [doc/howitswork.md](doc/howitswork.md)
- Deployment: [doc/deployment.md](doc/deployment.md)
- API methods index: [doc/methods/README.md](doc/methods/README.md)
  - Proprietary methods: [doc/methods/proprietary.md](doc/methods/proprietary.md)
  - DICOMweb methods: [doc/methods/dicomweb.md](doc/methods/dicomweb.md)

## Getting started

### Docker

- Docker image: `opendicom/sirius-hip:latest`
- Compose examples live in [docker/](docker/)
- Configuration is generated from environment variables with the `SIRIUS_HIP_*` prefix (see [docker/docker-compose.env](docker/docker-compose.env))

### Local (Rust)

Run with a TOML config file:

```bash
cargo run --release -- -c ./sirius-hip.toml
```

## Useful endpoints

- `GET /echo` health check
- `GET /settings` runtime config (secrets redacted)
- `GET /studyToken` viewer/session manifest builder
- StudyToken URL builder (Docker nginx): `/urlbuilder/study-token.html`
- `GET /qido/studies` QIDO-RS SearchForStudies
- QIDO URL builder (Docker nginx): `/urlbuilder/qido-studies.html`

# License

**Sirius HIP** is licensed by [Mozilla Public License 2.0](https://choosealicense.com/licenses/mpl-2.0/).
