use std::fmt;

use actix_web::{
    error,
    http::StatusCode,
    HttpResponse,
};
use jsonwebtoken as jwt;


// ---------------------------------------------------------------- //
#[derive(Debug)]
pub struct HttpError {
    anyerr: Option<anyhow::Error>,
    httperr: Option<actix_web::error::Error>

}
impl  HttpError {
    pub fn _new_anyhow_err(err: anyhow::Error) -> Self {
        Self {
            anyerr: Some(err),
            httperr: None
        }
    }
    pub fn new_http_err(err: actix_web::error::Error) -> Self {
        Self {
            anyerr: None,
            httperr: Some(err),
        }
    }
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(value) = &self.anyerr {
            write!(f, "{}", value)
        } else {
            write!(f, "{}", self.httperr.as_ref().unwrap())
        }    
    }
}

impl From<anyhow::Error> for HttpError {
    fn from(err: anyhow::Error) -> HttpError {
        HttpError { 
            anyerr: Some(err),
            httperr: None
        }
    }
}


impl error::ResponseError for HttpError {
    fn error_response(&self) -> HttpResponse {
        if let Some(err) = &self.anyerr {
            log::error!("{:?}",err);
            if err.downcast_ref::<dicom_object::AccessByNameError>().is_some() {
                HttpResponse::build(self.status_code()).finish()
            }
            
            else if err.downcast_ref::<dicom_core::value::ConvertValueError>().is_some() {
                HttpResponse::build(self.status_code()).finish()
            }
            
            else if err.downcast_ref::<sqlx::Error>().is_some() {
                HttpResponse::build(self.status_code()).finish()
            }
    
            else if err.downcast_ref::<jwt::errors::Error>().is_some() {
                HttpResponse::build(self.status_code()).body(err.to_string())
            }
            
            else {
                log::warn!("Unhandled error type:\n{:#?}",err);
                HttpResponse::build(self.status_code()).finish()
            }  
        } else {
            self.httperr.as_ref().unwrap().error_response()
        }
        
        
    }
        
    fn status_code(&self) -> StatusCode {
        if let Some(err) = &self.anyerr {
            if err.downcast_ref::<dicom_object::AccessByNameError>().is_some() {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            else if err.downcast_ref::<dicom_core::value::ConvertValueError>().is_some() {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            else if err.downcast_ref::<sqlx::Error>().is_some() {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            else if err.downcast_ref::<jwt::errors::Error>().is_some() {
                match err.downcast_ref::<jwt::errors::Error>().unwrap().kind() {
                    jwt::errors::ErrorKind::InvalidToken => StatusCode::UNAUTHORIZED,
                    jwt::errors::ErrorKind::InvalidKeyFormat => StatusCode::UNAUTHORIZED,
                    jwt::errors::ErrorKind::MissingRequiredClaim(_) => StatusCode::UNAUTHORIZED,
                    jwt::errors::ErrorKind::ExpiredSignature => StatusCode::UNAUTHORIZED,
                    jwt::errors::ErrorKind::InvalidIssuer => StatusCode::UNAUTHORIZED,
                    jwt::errors::ErrorKind::InvalidAudience => StatusCode::UNAUTHORIZED,
                    jwt::errors::ErrorKind::InvalidSubject => StatusCode::UNAUTHORIZED,
                    jwt::errors::ErrorKind::InvalidAlgorithm => StatusCode::UNAUTHORIZED,
                    jwt::errors::ErrorKind::MissingAlgorithm => StatusCode::UNAUTHORIZED,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                }
            }
            else {
                StatusCode::NOT_IMPLEMENTED
            }
        } else {
            StatusCode::NOT_IMPLEMENTED
        }
    }
}