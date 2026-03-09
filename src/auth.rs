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

