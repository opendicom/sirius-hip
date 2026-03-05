# Sirius HIP - Docker image

## How to build
Download a copy of this repo and extract it.

```bash
cd sirius-hip/
docker build -t opendicom/sirius-hip:latest -f docker/build/Dockerfile .
```

## How to run
In `docker/` folder there is a `docker-compose.yml` file with an example on how to run this image.

```bash
cd docker/
docker compose up -d
```

Notes:

- `docker-compose.yml` loads `docker-compose.env` + `docker-compose.prod.env`.
- `docker-compose-dev.yml` loads `docker-compose.env` + `docker-compose.dev.env`.
 
## Environment variables
Sirius HIP Docker image supports the following environment variables.

### Core
- `SIRIUS_HIP_LOGLEVEL` (default: `info`)
- `SIRIUS_HIP_MAX_DEFAULT` (default: `5000`)
- `CORS_WHITELIST` (default: `["*"]`)

### JWT
- `SIRIUS_HIP_JWT_AUTH` (default: `none`, values: `none|standard|onetime`)
- `AUTH_JWT_SECRET` (default: `secret`)
- `AUTH_JWT_ALGO` (default: `HS256`)

### PACS routing helpers
- `PACS_HOST` (default: `opendicom_pacs`)
- `PACS_PORT` (default: `8080`)

### PACS configuration (`[dicomarchive]`)
- `SIRIUS_HIP_PACS_VERSION` (default: `dcm4chee2183`)
- `SIRIUS_HIP_PACS_WADOURI` (default: `http://${PACS_HOST}:${PACS_PORT}/wado`)
- `SIRIUS_HIP_TRANSFER_SYNTAX` (default: `1.2.840.10008.1.2.1`)
- `SIRIUS_HIP_FILESYSTEM_CUTOFF_DATE` (default: `2026-03-01`)
- `SIRIUS_HIP_FS_MAPPINGS` (default: `[{id=1,path="/DICOM/archive"}]`)
- `SIRIUS_HIP_CUSTODIAN_OID` (optional, default: unset)
- `SIRIUS_HIP_PACS_OID` (optional, default: unset)
- `SIRIUS_HIP_PACS_AET` (optional, default: unset)
- `SIRIUS_HIP_MANIFEST_BASE_URL` (optional, default: unset)
- `SIRIUS_HIP_NUM_FRAMES_FIELD` (optional, default: unset)
- `SIRIUS_HIP_INSTITUTION_FIELD` (optional, default: unset)

### PACS database connection
- `SIRIUS_HIP_PACS_DATABASE_URL` (default derived from MySQL vars below)
- `SIRIUS_HIP_PACS_DATABASE_MAX_CONNECTIONS` (default: `40`)

### App database connection (download sessions / one-time)
- `SIRIUS_HIP_APP_DATABASE_URL` (default derived from MySQL vars below)
- `SIRIUS_HIP_APP_DATABASE_MAX_CONNECTIONS` (default: `20`)

### OneTime cleanup (`[onetime_cleanup]`)
- `SIRIUS_HIP_ONETIME_CLEANUP_ENABLED` (default: `true`)
- `SIRIUS_HIP_ONETIME_CLEANUP_INTERVAL_SECS` (default: `300`)
- `SIRIUS_HIP_ONETIME_CLEANUP_RETENTION_HOURS` (default: `24`)
- `SIRIUS_HIP_ONETIME_CLEANUP_SESSION_BATCH` (default: `200`)
- `SIRIUS_HIP_ONETIME_CLEANUP_MAX_BATCHES` (default: `20`)
- `SIRIUS_HIP_ONETIME_CLEANUP_TOKEN_DELETE_LIMIT` (default: `5000`)
- `SIRIUS_HIP_ONETIME_CLEANUP_INITIAL_JITTER_MAX_SECS` (default: `60`)

### MySQL helper variables (used to build default DB URLs)
- `MYSQL_HOST` (default: `opendicom_pacs_db`)
- `MYSQL_PORT` (default: `3306`)
- `MYSQL_DATABASE` (default: `pacsdb`)
- `MYSQL_USER` (default: `pacs`)
- `MYSQL_PASSWORD` (default: `pacs`)
