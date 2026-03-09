use actix_web::HttpResponse;
use actix_web::http::StatusCode;
use actix_web::http::header::{HeaderName, HeaderValue};
use futures::StreamExt;
use reqwest;
use reqwest::header::HeaderName as ReqwestHeaderName;

use crate::src2::errors::app_error::AppError;

/// Constructs a WADO URL from the given UIDs and settings.
pub fn wado_url_from_uids(
    settings: &crate::settings::Settings,
    study_uid: &str,
    series_uid: &str,
    sop_uid: &str,
) -> String {
    format!(
        "{}?requestType=WADO&studyUID={}&seriesUID={}&objectUID={}&contentType=application/dicom&transferSyntax={}",
        settings.dicomarchive.wadouri,
        study_uid,
        series_uid,
        sop_uid,
        settings.dicomarchive.transfer_syntax,
    )
}

fn should_forward_header(name: &ReqwestHeaderName) -> bool {
    !name.as_str().eq_ignore_ascii_case("connection")
}

/// Proxies a WADO response by streaming the bytes back to the client while preserving headers and status code.
pub fn proxy_wado_response(res: reqwest::Response) -> Result<HttpResponse, AppError> {
    let status = StatusCode::from_u16(res.status().as_u16())
        .map_err(|e| AppError::Internal(e.into()))?;
    let mut out = HttpResponse::build(status);
    for (header_name, header_value) in res
        .headers()
        .iter()
        .filter(|(h, _)| should_forward_header(*h))
    {
        let Ok(header_name) = HeaderName::from_bytes(header_name.as_str().as_bytes()) else {
            continue;
        };
        let Ok(header_value) = HeaderValue::from_bytes(header_value.as_bytes()) else {
            continue;
        };
        out.insert_header((header_name, header_value));
    }

    Ok(out.streaming(res.bytes_stream().map(|chunk| {
        chunk.map_err(|e| actix_web::error::ErrorInternalServerError(e))
    })))
}

/// Proxies a WADO request by forwarding the query parameters to the WADO service and streaming the response back.
pub async fn proxy_wado_url(
    client: &reqwest::Client,
    url: String,
) -> Result<HttpResponse, AppError> {
    let res = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    proxy_wado_response(res)
}

#[cfg(test)]
mod tests {
    use super::should_forward_header;
    use super::wado_url_from_uids;
    use reqwest::header::HeaderName as ReqwestHeaderName;

    #[test]
    fn should_filter_connection_header() {
        let connection = ReqwestHeaderName::from_static("connection");
        let content_type = ReqwestHeaderName::from_static("content-type");

        assert!(!should_forward_header(&connection));
        assert!(should_forward_header(&content_type));
    }

    #[test]
    fn builds_wado_url_from_uids() {
        let toml = r#"
    loglevel = "info"
    max_default = 100
    cors_whitelist = ["*"]

    app_database_url = "mysql://user:pass@localhost/app"

    jwt_auth = "standard"
    jwt_secret = "secret"
    jwt_algorithm = "HS256"

    [dicomarchive]
    version = "dcm4chee2183"
    database_url = "mysql://user:pass@localhost/pacs"
    database_max_connections = 1
    wadouri = "http://pacs.example/wado"
    transfer_syntax = "1.2.840.10008.1.2.1"
    filesystems = []
    "#;

        let settings = toml::from_str::<crate::settings::Settings>(toml)
            .expect("valid settings toml");

        let url = wado_url_from_uids(&settings, "1", "2", "3");

        assert_eq!(
            url,
            "http://pacs.example/wado?requestType=WADO&studyUID=1&seriesUID=2&objectUID=3&contentType=application/dicom&transferSyntax=1.2.840.10008.1.2.1"
        );
    }
}
