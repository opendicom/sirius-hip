
use actix_web::{get, web, HttpResponse};
use serde_json::json;

use crate::settings::Settings;

// -- CUSTODIANS ----------------------------------------------------------------------------- //

#[get("/oids")]
pub async fn oids(settings: web::Data<Settings>) -> HttpResponse {
    HttpResponse::Ok().json(json!([settings.dicomarchive.custodianoid]))
}

/// Get `pacsoid` from where `custodianoid = {oid}`
#[get("/oids/{oid}/aeis")]
pub async fn aeis(settings: web::Data<Settings>, oid: web::Path<String>) -> HttpResponse {
    if let Some(value) = &settings.dicomarchive.custodianoid {
        if value.eq(&*oid) {
            HttpResponse::Ok().json(json!([settings.dicomarchive.pacsoid]))
        } else {
            HttpResponse::NotFound().finish()
        }
    } else {
        HttpResponse::NotFound().finish()
    }
}

// -- PACS ----------------------------------------------------------------------------- //

/// Get `pacsoid` from where `custodianoid = {oid}`
#[get("/{oid}/properties/wadouri")]
pub async fn wadouri(settings: web::Data<Settings>, oid: web::Path<String>) -> HttpResponse {
    if let Some(value) = &settings.dicomarchive.pacsoid {
        if value.eq(&*oid) {
            HttpResponse::Ok().body(settings.dicomarchive.wadouri.clone())
        } else {
            HttpResponse::Ok().finish()
        }  
    } else {
        HttpResponse::Ok().finish()
    }
}

/// Get `pacsoid` from where `custodianoid = {oid}`
#[get("/{oid}/properties/stow")]
pub async fn stow(settings: web::Data<Settings>, oid: web::Path<String>) -> HttpResponse {
    if let Some(value) = &settings.dicomarchive.pacsoid {
        if value.eq(&*oid) {
            if let Some(stow) = &settings.dicomarchive.stow {
                HttpResponse::Ok().body(stow.clone())
            } else {
                HttpResponse::Ok().finish()
            }
        } else {
            HttpResponse::Ok().finish()
        }
    } else {
        HttpResponse::Ok().finish()
    }  
}

/// Get `pacsoid` from where `custodianoid = {oid}`
#[get("/{oid}/properties/qido")]
pub async fn qido(settings: web::Data<Settings>, oid: web::Path<String>) -> HttpResponse {
    if let Some(value) = &settings.dicomarchive.pacsoid {
        if value.eq(&*oid) {
            if let Some(qido) = &settings.dicomarchive.qido {
                HttpResponse::Ok().body(qido.clone())
            } else {
                HttpResponse::Ok().finish()
            }
        } else {
            HttpResponse::Ok().finish()
        }
    } else {
        HttpResponse::Ok().finish()
    }  
}