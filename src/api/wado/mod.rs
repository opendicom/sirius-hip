use actix_web::{HttpRequest, web, HttpResponse,dev::PeerAddr};
use reqwest::Method;
use serde::{Serialize, Deserialize};

use futures::StreamExt as _;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{error::HttpError, settings::Settings};

#[derive(Debug, Serialize, Deserialize)]
pub struct Params {
    pub token: Option<String>
}

// -- End Points --------------------------------------------------------------------------------------- //
pub async fn endpoint(_params: web::Query<Params>, req: HttpRequest, settings: web::Data<Settings>,peer_addr: Option<PeerAddr>, mut payload: web::Payload,) -> Result<HttpResponse, HttpError> {
    
    // JWT Payload (Claims)
    #[derive(Debug, Serialize, Deserialize)]
    struct Claims {
        aud: String,
        exp: usize,          
    }

    // -- JWT Authorization ------------------------------------------------------------------------- // 
    // if settings.jwt_auth != JwtAuthMethod::None {
    //     if let Some(token) = &params.token {
    //         validate_token(token, &settings).context("Autorization error")?;
    //     } else {
    //         return Err(HttpError::new_http_err(actix_web::error::ErrorUnauthorized("Token parameter was not found in the url")));
    //     }
    // }
    
    let client = reqwest::Client::default();

    let wadourl = format!("{}?{}",settings.dicomarchive.wadouri,req.query_string());
    log::debug!("Request: {wadourl}");

    let (tx, rx) = mpsc::unbounded_channel();

    actix_web::rt::spawn(async move {
        while let Some(chunk) = payload.next().await {
            tx.send(chunk).unwrap();
        }
    });

    let forwarded_req = client
        .request(Method::GET, wadourl)
        .body(reqwest::Body::wrap_stream(UnboundedReceiverStream::new(rx)));

    // TODO: This forwarded implementation is incomplete as it only handles the unofficial
    // X-Forwarded-For header but not the official Forwarded one.
    let forwarded_req = match peer_addr {
        Some(PeerAddr(addr)) => forwarded_req.header("x-forwarded-for", addr.ip().to_string()),
        None => forwarded_req,
    };

    let res = forwarded_req
        .send()
        .await
        .map_err(|e| HttpError::new_http_err(actix_web::error::ErrorInternalServerError(e)))?;

    let mut client_resp = HttpResponse::build(res.status());
    // Remove `Connection` as per
    // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection#Directives
    for (header_name, header_value) in res.headers().iter().filter(|(h, _)| *h != "connection") {
        client_resp.insert_header((header_name.clone(), header_value.clone()));
    }

    Ok(client_resp.streaming(res.bytes_stream()))


    // let wadourl = format!("{}?{}",settings.dicomarchive.wadouri,req.query_string());
    // log::debug!("Request: {wadourl}");

    // // -- Request to wado 
    // let stream = reqwest::get(&wadourl)
    //     .await
    //     .context("Failed to get response from: `{url}`")?
    //     .bytes_stream();
    
    // // -- Stream wado response
    // Ok(HttpResponse::Ok()
    //     .content_type("application/octet-stream")
    //     .streaming(stream))

    
}
