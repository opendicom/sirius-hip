use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use futures::StreamExt;
use reqwest;
use reqwest::header::HeaderName as ReqwestHeaderName;

use crate::errors::app_error::AppError;

fn should_forward_header(name: &ReqwestHeaderName) -> bool {
    !name.as_str().eq_ignore_ascii_case("connection")
}

/// Proxies a WADO response by streaming the bytes back to the client while preserving headers and status code.
pub(crate) fn proxy_wado_response(res: reqwest::Response) -> Result<HttpResponse, AppError> {
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
pub(crate) async fn proxy_wado_url(
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
    use reqwest::header::HeaderName as ReqwestHeaderName;

    #[test]
    fn should_filter_connection_header() {
        let connection = ReqwestHeaderName::from_static("connection");
        let content_type = ReqwestHeaderName::from_static("content-type");

        assert!(!should_forward_header(&connection));
        assert!(should_forward_header(&content_type));
    }
}
