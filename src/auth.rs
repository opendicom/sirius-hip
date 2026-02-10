use serde::{Serialize, Deserialize};
use jsonwebtoken as jwt;

use crate::settings::Settings;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthClaims {
    pub aud: String,
    pub exp: usize,          
}

/// Validates a JWT token and returns the claims if valid, or an error if invalid or expired.
pub fn validate_jwt_token(
    token: &String, 
    settings: &Settings
) -> Result<AuthClaims, jwt::errors::Error> {
    
    // -- Get session jwt from request
    let secret = jwt::DecodingKey::from_secret(settings.jwt_secret.as_bytes());
    let mut validation = jwt::Validation::new(settings.jwt_algorithm);
    validation.set_audience(&["wezen", "sirius-hip"]);

    // -- Decode jwt
    jwt::decode::<AuthClaims>(&token, &secret, &validation)
        .map(|t|t.claims)
}   


// =====================================================================================
// Download tokens (stateless, signed)
// =====================================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DownloadClaims {
    pub aud: String,
    pub exp: usize,
    pub study_uid: String,
    pub series_uid: String,
    pub sop_uid: String,
    pub filesystem_fk: Option<i32>,
    pub relative_file_path: Option<String>,
}

pub fn encode_download_token(
    claims: &DownloadClaims,
    settings: &Settings,
) -> Result<String, jwt::errors::Error> {
    let key = jwt::EncodingKey::from_secret(settings.jwt_secret.as_bytes());
    let header = jwt::Header::new(settings.jwt_algorithm);
    jwt::encode(&header, claims, &key)
}

pub fn validate_download_token(
    token: &str,
    settings: &Settings,
) -> Result<DownloadClaims, jwt::errors::Error> {
    let secret = jwt::DecodingKey::from_secret(settings.jwt_secret.as_bytes());
    let mut validation = jwt::Validation::new(settings.jwt_algorithm);
    validation.set_audience(&["sirius-hip-dl"]);
    jwt::decode::<DownloadClaims>(token, &secret, &validation).map(|t| t.claims)
}






