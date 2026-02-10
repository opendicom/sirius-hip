// -- Re-export sub modules ---------------------------------------------------------------------- //
pub mod params;

// ----------------------------------------------------------------------------------------------- //

use actix_web::{web::{Query, Data}, HttpResponse, HttpRequest};
use sqlx::MySqlPool;

use crate::{settings::Settings, 
            models, error::HttpError};

use self::params::StudyTokenParams;


// -- End Points --------------------------------------------------------------------------------------- //
pub async fn endpoint(params: Query<StudyTokenParams>, req: HttpRequest, pool: Data<MySqlPool>, settings: Data<Settings>) -> Result<HttpResponse, HttpError> {

    let params = params.into_inner();
    let conn = req.connection_info();
    let server_base_url = format!("{}://{}",
        conn.scheme(),
        conn.host(),
    );

    
    // // -- JWT Authorization ------------------------------------------------------------------------- // 
    // if settings.jwt_auth {
    //     if let Some(token) = &params.token {
    //         validate_token(token, &settings).context("Autorization error")?;
    //     } else {
    //         return Err(HttpError::new_http_err(actix_web::error::ErrorUnauthorized("Token parameter was not found in the url")));
    //     }
    // }

        
    // -- Cornerstone ------------------------------------------------------------------------- // 
    if params.access_type.eq("cornerstone.json") {
        let manifest = models::cornerstone::build_manifest(&pool, &params, &settings, server_base_url).await?;
        Ok(HttpResponse::Ok().json(manifest))
    }

    // -- Weasis ------------------------------------------------------------------------- // 
    else if params.access_type.eq("weasis.xml") {
        let manifest = models::weasis::build_manifest(&pool, &params, &settings, server_base_url).await?;

        Ok(HttpResponse::Ok()
            .content_type("application/octet-stream")
            .append_header(("Content-Disposition", format!("attachment; filename=weasis-manifest.xml")))
            .body(manifest.into_inner()))
    }

    // -- Dicom ZIP ------------------------------------------------------------------------- // 
    else if params.access_type.eq("dicom.zip") {

        let stream = models::dicomzip::streamzip(&pool, &params, &settings)
            .await?
            .build();

        Ok(HttpResponse::Ok()
            .content_type("application/octet-stream")
            .append_header(("Content-Disposition", "attachment; filename=dicom.zip".to_string()))
            .streaming(stream))
    }

    else if params.access_type.eq("ohif") {
        let manifest = models::ohif::build_manifest(&pool, &params, &settings, server_base_url).await?;
        Ok(HttpResponse::Ok().json(manifest))
    }
    
    // -- Others ------------------------------------------------------------------------- // 
    else {
        Ok(HttpResponse::MethodNotAllowed().body(format!("Unsupported acccess_type: `{}`",params.access_type)))
    }     

}