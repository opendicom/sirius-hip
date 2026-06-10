use axum::http::HeaderMap;

pub fn extract_bearer_token(headers: &HeaderMap, query_token: Option<&str>) -> Option<String> {
    if let Some(raw) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        let raw = raw.trim();
        if raw.len() >= 7 && raw[..7].eq_ignore_ascii_case("bearer ") {
            let token = raw[7..].trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }

    query_token
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| token.to_string())
}
