use std::borrow::Cow;

/// Constructs a WADO URL from the given UIDs and settings.
pub(crate) fn wado_url_from_uids(
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

/// Builds an upstream URL by appending the raw query string to a base URL.
pub(crate) fn build_upstream_url(base: &str, query_string: &str) -> String {
    if query_string.is_empty() {
        return base.to_string();
    }
    let sep = if base.contains('?') { '&' } else { '?' };
    format!("{base}{sep}{query_string}")
}

/// Normalizes WADO `contentType`.
/// - If missing/empty: defaults to `application/dicom`
/// - Otherwise returns the provided value without allocating when possible.
pub(crate) fn normalize_content_type<'a>(v: Option<&'a str>) -> Cow<'a, str> {
    match v.map(str::trim).filter(|s| !s.is_empty()) {
        Some(ct) => Cow::Borrowed(ct),
        None => Cow::Borrowed("application/dicom"),
    }
}

// Normalizes the transfer syntax by treating missing or empty values as the application settings.
// This is because in WADO-URI, the transferSyntax parameter is optional and defaults to the application configuration if not provided.
/// This function ensures that we have a consistent transfer syntax to work with, which is important for determining
/// how to serve the file (e.g., from filesystem or via WADO proxy).
pub(crate) fn normalize_transfer_syntax<'a>(v: Option<&'a str>, default: &'a str) -> &'a str {
    // WADO-URI: transferSyntax is optional; treat missing/empty as application config.
    match v.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some(ts) => ts,
        None => default,
    }
}

/// Strips internal query parameters (e.g. token, session) from the original query string before forwarding to WADO.
pub(crate) fn strip_internal_query_params(query_string: &str) -> String {
    // Never leak our internal security params to the upstream PACS.
    // Keep everything else intact (percent-encoding included).
    let mut out = Vec::new();
    for part in query_string.split('&') {
        if part.is_empty() {
            continue;
        }
        let key = part.split_once('=').map(|(k, _)| k).unwrap_or(part);
        if key == "token" || key == "session" {
            continue;
        }
        out.push(part);
    }
    out.join("&")
}

#[cfg(test)]
mod tests {
    use super::wado_url_from_uids;

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
