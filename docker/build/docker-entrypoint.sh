#!/bin/sh
# WARN: In alpine version can't use /bin/bash is not instaled by defalut
set -e

TEMPLATE_FILE=/template/sirius-hip.conf.template

# Required parameters: set default values 
export CORS_WHITELIST=${CORS_WHITELIST:-'["*"]'}

export SIRIUS_HIP_LOGLEVEL=${SIRIUS_HIP_LOGLEVEL:-'info'}
export SIRIUS_HIP_JWT_AUTH=${SIRIUS_HIP_JWT_AUTH:-'none'}
export AUTH_JWT_SECRET=${AUTH_JWT_SECRET:-secret}
export AUTH_JWT_ALGO=${AUTH_JWT_ALGO:-HS256}

export PACS_HOST=${PACS_HOST:-opendicom_pacs}
export PACS_PORT=${PACS_PORT:-8080}

export SIRIUS_HIP_PACS_VERSION=${SIRIUS_HIP_PACS_VERSION:-dcm4chee2183}
export SIRIUS_HIP_PACS_WADOURI=${SIRIUS_HIP_PACS_WADOURI:-"http://${PACS_HOST}:${PACS_PORT}/wado"}
export SIRIUS_HIP_FS_MAPPINGS=${SIRIUS_HIP_FS_MAPPINGS:-'[{id=1,path="/DICOM/archive"}]'}

export SIRIUS_HIP_CUSTODIAN_OID=${SIRIUS_HIP_CUSTODIAN_OID:-}
export SIRIUS_HIP_PACS_OID=${SIRIUS_HIP_PACS_OID:-}
export SIRIUS_HIP_PACS_AET=${SIRIUS_HIP_PACS_AET:-}
export SIRIUS_HIP_MANIFEST_BASE_URL=${SIRIUS_HIP_MANIFEST_BASE_URL:-}
export SIRIUS_HIP_NUM_FRAMES_FIELD=${SIRIUS_HIP_NUM_FRAMES_FIELD:-}
export SIRIUS_HIP_INSTITUTION_FIELD=${SIRIUS_HIP_INSTITUTION_FIELD:-}
export SIRIUS_HIP_TRANSFER_SYNTAX=${SIRIUS_HIP_TRANSFER_SYNTAX:-'1.2.840.10008.1.2.1'}
export SIRIUS_HIP_MAX_DEFAULT=${SIRIUS_HIP_MAX_DEFAULT:-5000}


export MYSQL_HOST=${MYSQL_HOST:-opendicom_pacs_db}
export MYSQL_PORT=${MYSQL_PORT:-3306}
export MYSQL_DATABASE=${MYSQL_DATABASE:-pacsdb}
export MYSQL_USER=${MYSQL_USER:-pacs}
export MYSQL_PASSWORD=${MYSQL_PASSWORD:-pacs}

# PACS DB settings (required by Settings.dicomarchive.*)
export SIRIUS_HIP_PACS_DATABASE_URL=${SIRIUS_HIP_PACS_DATABASE_URL:-"mysql://${MYSQL_USER}:${MYSQL_PASSWORD}@${MYSQL_HOST}:${MYSQL_PORT}/${MYSQL_DATABASE}"}
export SIRIUS_HIP_PACS_DATABASE_MAX_CONNECTIONS=${SIRIUS_HIP_PACS_DATABASE_MAX_CONNECTIONS:-40}

# App DB settings (required by init_download_session_repo)
export SIRIUS_HIP_APP_DATABASE_URL=${SIRIUS_HIP_APP_DATABASE_URL:-"mysql://${MYSQL_USER}:${MYSQL_PASSWORD}@${MYSQL_HOST}:${MYSQL_PORT}/${MYSQL_DATABASE}"}
export SIRIUS_HIP_APP_DATABASE_MAX_CONNECTIONS=${SIRIUS_HIP_APP_DATABASE_MAX_CONNECTIONS:-20}

# OneTime cleanup settings (maps to Settings.onetime_cleanup)
export SIRIUS_HIP_ONETIME_CLEANUP_ENABLED=${SIRIUS_HIP_ONETIME_CLEANUP_ENABLED:-true}
export SIRIUS_HIP_ONETIME_CLEANUP_INTERVAL_SECS=${SIRIUS_HIP_ONETIME_CLEANUP_INTERVAL_SECS:-300}
export SIRIUS_HIP_ONETIME_CLEANUP_RETENTION_HOURS=${SIRIUS_HIP_ONETIME_CLEANUP_RETENTION_HOURS:-24}
export SIRIUS_HIP_ONETIME_CLEANUP_SESSION_BATCH=${SIRIUS_HIP_ONETIME_CLEANUP_SESSION_BATCH:-200}
export SIRIUS_HIP_ONETIME_CLEANUP_MAX_BATCHES=${SIRIUS_HIP_ONETIME_CLEANUP_MAX_BATCHES:-20}
export SIRIUS_HIP_ONETIME_CLEANUP_TOKEN_DELETE_LIMIT=${SIRIUS_HIP_ONETIME_CLEANUP_TOKEN_DELETE_LIMIT:-5000}
export SIRIUS_HIP_ONETIME_CLEANUP_INITIAL_JITTER_MAX_SECS=${SIRIUS_HIP_ONETIME_CLEANUP_INITIAL_JITTER_MAX_SECS:-60}


envsubst < "$TEMPLATE_FILE" > /etc/sirius-hip/sirius-hip.toml

# Optional Parameters
# If env var has a value un-comment the parameter
if [ "$SIRIUS_HIP_CUSTODIAN_OID" != "" ]; then
    sed -i 's| *# *custodianoid|custodianoid|g' /etc/sirius-hip/sirius-hip.toml
fi
if [ "$SIRIUS_HIP_PACS_OID" != "" ]; then
    sed -i 's| *# *pacsoid|pacsoid|g' /etc/sirius-hip/sirius-hip.toml
fi
if [ "$SIRIUS_HIP_PACS_AET" != "" ]; then
    sed -i 's| *# *pacsaet|pacsaet|g' /etc/sirius-hip/sirius-hip.toml
fi
if [ "$SIRIUS_HIP_MANIFEST_BASE_URL" != "" ]; then
    sed -i 's| *# *manifest_base_url|manifest_base_url|g' /etc/sirius-hip/sirius-hip.toml
fi
if [ "$SIRIUS_HIP_NUM_FRAMES_FIELD" != "" ]; then
    sed -i 's| *# *number_frames_field|number_frames_field|g' /etc/sirius-hip/sirius-hip.toml
fi
if [ "$SIRIUS_HIP_INSTITUTION_FIELD" != "" ]; then
    sed -i 's| *# *institution_field|institution_field|g' /etc/sirius-hip/sirius-hip.toml
fi

nginx

exec "$@"