#!/bin/sh
# WARN: In alpine version can't use /bin/bash is not instaled by defalut
set -e

TEMPLATE_FILE=/template/sirius-hip.conf.template

# Required parameters: set default values 
export CORS_WHITELIST=${CORS_WHITELIST:-'["*"]'}

export SIRIUS_HIP_LOGLEVEL=${SIRIUS_HIP_LOGLEVEL:-info}
export SIRIUS_HIP_JWT_AUTH=${SIRIUS_HIP_JWT_AUTH:-none}
export AUTH_JWT_SECRET=${AUTH_JWT_SECRET:-secret}
export AUTH_JWT_ALGO=${AUTH_JWT_ALGO:-HS256}

export PACS_HOST=${PACS_HOST:-opendicom_pacs}
export PACS_PORT=${PACS_HOST:-8080}

export SIRIUS_HIP_PACS_VERSION=${SIRIUS_HIP_PACS_VERSION:-dcm4chee2183}
export SIRIUS_HIP_PACS_WADOURI=${SIRIUS_HIP_PACS_WADOURI:-"http://${PACS_HOST}:8080/wado"}
export SIRIUS_HIP_FS_MAPPINGS=${SIRIUS_HIP_FS_MAPPINGS:-'[{id=1,path="/DICOM/archive"}]'}

export SIRIUS_HIP_CUSTODIAN_OID=${SIRIUS_HIP_CUSTODIAN_OID:-}
export SIRIUS_HIP_PACS_OID=${SIRIUS_HIP_PACS_OID:-}
export SIRIUS_HIP_PACS_AET=${SIRIUS_HIP_PACS_AET:-}
export SIRIUS_HIP_MANIFEST_BASE_URL=${SIRIUS_HIP_MANIFEST_BASE_URL:-}
export SIRIUS_HIP_NUM_FRAMES_FIELD=${SIRIUS_HIP_NUM_FRAMES_FIELD:-}
export SIRIUS_HIP_INSTITUTION_FIELD=${SIRIUS_HIP_INSTITUTION_FIELD:-}
export SIRIUS_HIP_TRANSFER_SYNTAX=${SIRIUS_HIP_TRANSFER_SYNTAX:-'1.2.840.10008.1.2'}
export SIRIUS_HIP_MAX_DEFAULT=${SIRIUS_HIP_MAX_DEFAULT:-5000}


export MYSQL_HOST=${MYSQL_HOST:-opendicom_pacs_db}
export MYSQL_PORT=${MYSQL_PORT:-3306}
export MYSQL_DATABASE=${MYSQL_DATABASE:-pacsdb}
export MYSQL_USER=${MYSQL_USER:-pacs}
export MYSQL_PASSWORD=${MYSQL_PASSWORD:-pacs}


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