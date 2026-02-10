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
it is possible to configure several parameters through environment variables.

#### `SIRIUS_HIP_LOGLEVEL` 
Optional variable. Default value: `info`.

#### `CORS_WHITELIST` 
Optional variable. Default value: `["*"]`.

#### `SIRIUS_HIP_JWT_AUTH` 
Optional variable. Values: `none`, `standard`, `onetime`.

#### `AUTH_JWT_SECRET`
Optional variable. Default value: `secret`.

#### `AUTH_JWT_ALGO` 
Optional variable. Default value: `HS256`.

#### `PACS_HOST` 
Optional variable. Default value: `opendicom_pacs`.

#### `PACS_PORT` 
Optional variable. Default value: `8080`.

#### `SIRIUS_HIP_PACS_VERSION` 
Optional variable. Default value: `dcm4chee2183`.

#### `SIRIUS_HIP_PACS_WADOURI` 
Optional variable. Default value: `http://${PACS_HOST}:8080/wado`.

#### `SIRIUS_HIP_FS_MAPPINGS` 
Optional variable. Default value: `[{id=1,path="/DICOM/archive"}]`.

#### `SIRIUS_HIP_CUSTODIAN_OID` 
Optional variable. Default value: unset.

#### `SIRIUS_HIP_PACS_OID` 
Optional variable. Default value: unset.

#### `SIRIUS_HIP_PACS_AET` 
Optional variable. Default value: unset.

#### `SIRIUS_HIP_MANIFEST_BASE_URL` 
Optional variable. Default value: unset.

#### `SIRIUS_HIP_NUM_FRAMES_FIELD` 
Optional variable. Default value: unset.

#### `SIRIUS_HIP_INSTITUTION_FIELD` 
Optional variable. Default value: unset.

#### `SIRIUS_HIP_TRANSFER_SYNTAX` 
Optional variable. Default value: `1.2.840.10008.1.2`.

#### `SIRIUS_HIP_MAX_DEFAULT` 
Optional variable. Default value: `5000`.

#### `MYSQL_HOST` 
Optional variable. Default value: `opendicom_pacs_db`.

#### `MYSQL_PORT` 
Optional variable. Default value: `3306`.

#### `MYSQL_DATABASE` 
Optional variable. Default value: `pacsdb`.

#### `MYSQL_USER` 
Optional variable. Default value: `pacs`.

#### `MYSQL_PASSWORD` 
Optional variable. Default value: `pacs`.
