use actix_web::{App, HttpResponse, HttpServer, middleware, web};
use actix_web::ResponseError;
use actix_web::middleware::Logger;
use actix_cors::Cors;
use anyhow::Context;
use clap::{Command, arg, crate_version};
use serde_querystring_actix::{QueryStringConfig, ParseMode};
use sqlx::{mysql::MySqlPoolOptions,
           mysql::MySqlConnectOptions};
use std::io::Write;
use std::str::FromStr;
use std::sync::Arc;
use log::{LevelFilter, Level};
use env_logger::{WriteStyle, fmt::Color};
use std::env;
use std::fs;
use once_cell::sync::Lazy;
use std::time::Duration;

use crate::settings::Settings;
use crate::constants::QIDO_STUDY_INCLUDEFIELD_DIC;
use crate::pacs::repositories::factory::StudyRepositoryFactory;
use crate::state2::{AppState2, PacsState2};
use crate::errors::app_error::AppError;

mod settings;
mod constants;
mod auth;
mod utils;
mod pacs;
mod errors;
mod api;
mod state2;
mod application;
mod init;
mod dicomzip;


#[actix_web::main]
async fn main() {
    // Thanks to https://www.asciiart.eu/text-to-ascii-art for the logo.
    // Larry 3D
    println!(r"
         ____                                         __  __  ______   ____    
        /\  _`\   __         __                      /\ \/\ \/\__  _\ /\  _`\  
        \ \,\L\_\/\_\  _ __ /\_\  __  __    ____     \ \ \_\ \/_/\ \/ \ \ \L\ \
         \/_\__ \\/\ \/\`'__\/\ \/\ \/\ \  /',__\     \ \  _  \ \ \ \  \ \ ,__/
           /\ \L\ \ \ \ \ \/ \ \ \ \ \_\ \/\__, `\     \ \ \ \ \ \_\ \__\ \ \/ 
           \ `\____\ \_\ \_\  \ \_\ \____/\/\____/      \ \_\ \_\/\_____\\ \_\ 
            \/_____/\/_/\/_/   \/_/\/___/  \/___/        \/_/\/_/\/_____/ \/_/ 
    
    ");

    std::env::set_var("RUST_LOG", "debug");

    // Global error handling: log the error
    if let Err(e) = run().await {
        log::error!("{:?}", e);
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {

    // -- Command line arguments  --------------------------------------------------------------------- //
    let matches = Command::new("Sirius HIP")
        .about("The integration platform for Sirius PACS")
        .after_help("HIP = Health Integration Platform")
        .version(crate_version!())
        .author("Opendicom")
        .arg(arg!(-b --bind <socket> "IP_ADDRESS:PORT to bind xdsproy server")
                .default_value("0.0.0.0:5001"))
        .arg(arg!(-c --config <file> "Filepath to the configuration file")
            .default_value("./sirius-hip.toml"))
        .get_matches();

    // -- Get bind socket ----------------------------------------------------------------------------- //
    let bind = match matches.get_one::<String>("bind") {
        Some(val) => val,
        None => anyhow::bail!("Failed to get `bind ip address` from the command line arguments"),
    };

    // -- Load main settings configuration from file ------------------------------------------------------- //
    let conf_file = match matches.get_one::<String>("config") {
        Some(val) => val,
        None => anyhow::bail!("Failed to get `config file` from the command line arguments")
    };
    dotenv::from_path(conf_file).ok();

    let settings = Arc::new( {
        let content = fs::read_to_string(&conf_file)
            .context("Failed to load configuration file.")?;
        toml::from_str::<Settings>(&content)
            .context("Failed to parse configuration file.")?
    });

    // -- Validate settings --------------------------------------------------------------------------------- //
    settings.validate()?;

    Lazy::force(&QIDO_STUDY_INCLUDEFIELD_DIC);
    
     // -- Logging  -------------------------------------------------------------------------------------------//
    env_logger::Builder::new()
    .filter_level(LevelFilter::Info)
    .parse_env("loglevel")
    .format(|buf, record| {
        let mut style = buf.style();
        
        match record.level() {
            Level::Info => style.set_color(Color::Green),
            Level::Debug => style.set_color(Color::Blue),
            Level::Error => style.set_color(Color::Red),
            Level::Trace => style.set_color(Color::Magenta),
            Level::Warn => style.set_color(Color::Yellow),
        };

        writeln!(buf,
            "[{} {} {}] {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            style.value(record.level()),
            record.target(),
            record.args()
        )
    })
    .write_style(WriteStyle::Always)
    .init();

    log::info!("Starting Sirius HIP v{}", crate_version!());
    log::info!("Using configuration from {}",conf_file);


    // -- PACS Database ------------------------------------------------------------------------------------- //    
    let options = MySqlConnectOptions::from_str(&settings.dicomarchive.database_url)
        .context("Failed to get PACS Database connection.")?;
        // Disable query logging because bad display
        // They are logged manually with database::prettysql(...) function
        //.disable_statement_logging();

    let pool = MySqlPoolOptions::new()
        .max_connections(settings.dicomarchive.database_max_connections)
        .acquire_timeout(std::time::Duration::from_secs(6))
        .connect_with(options)
        .await
        .context("Failed to get PACS Database connection pool")?;



    // TODO:
    // Tengo que crear una funcion init_pacs_study_repo similar a init_download_session_repo
    // osea crear un solo pool y pasarselo a los  repositorios del pacs study, instance, series, file...
    let study_repo = StudyRepositoryFactory::create(&settings)
        .await
        .context("Failed to create Database Study Repository")?;


    let download_session_repo = crate::init::init_download_session_repo(&settings)
        .await
        .context("Failed to create Database Download Session Repository")?;

    // Background cleanup for OneTime persistence (MySQL app-DB).
    // Multi-instance safe: the MySQL repo uses GET_LOCK so only one instance cleans.
    if settings.onetime_cleanup.enabled
        && settings.app_database_url.starts_with("mysql://")
    {
        let cleanup_repo = download_session_repo.clone();
        let cleanup_cfg = settings.onetime_cleanup.clone();

        tokio::spawn(async move {
            // Small jitter to reduce simultaneous wakeups across instances.
            let jitter_max = cleanup_cfg.initial_jitter_max_secs.max(1);
            let jitter = (std::process::id() as u64) % jitter_max;
            tokio::time::sleep(Duration::from_secs(jitter)).await;

            let interval_secs = cleanup_cfg.interval_secs.clamp(30, 24 * 60 * 60);
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            let retention_hours = cleanup_cfg.retention_hours.clamp(0, 24 * 30);

            loop {
                interval.tick().await;
                // Calculate cutoff time for expired sessions.
                let cutoff = chrono::Utc::now() - chrono::Duration::hours(retention_hours);
                if let Err(e) = cleanup_repo.cleanup_expired(cutoff).await {
                    log::warn!("OneTime cleanup failed: {:?}", e);
                } else {
                    log::info!("OneTime cleanup completed successfully");
                }
            }
        });
    }

    let app_state = web::Data::new(AppState2 {
        download_session_repo,
        pacs: PacsState2 {
            study_repo,
        },
        tmp_pool: pool,
        settings: settings.clone(),
        http_client: reqwest::Client::builder()
            // Keep-alive + pooling improves throughput for WADO proxying.
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            // Fail fast on unreachable PACS.
            .connect_timeout(Duration::from_secs(5))
            // Do NOT set a global request timeout: downloads may legitimately stream for a long time.
            .build()
            .context("Failed to build HTTP client")?,
    });
    

    let settings_data = settings.clone();


    //QIDO Query String Extractor configuration
    let qido_qs = QueryStringConfig::default()
        .parse_mode(ParseMode::Duplicate) // <- choose the parsing mode
        .error_handler(|err, _req| {  // <- create custom error response
            // Keep client-facing errors sanitized and consistent.
            actix_web::error::InternalError::from_response(
                err,
                AppError::bad_request("invalid query parameters").error_response(),
            )
            .into()
        });



    // -- HttpServer  --------------------------------------------------------------------------------- //
    log::info!("Listening on: {}",bind);
    HttpServer::new(move || {

        // CORS
        let sets = settings_data.clone();
        let cors = Cors::default()
            .allowed_headers(vec!["accept"])
            .allowed_methods(vec!["GET"])
            .allowed_origin_fn(move|origin, _req|{
                sets.cors_whitelist.iter().any(|value| value.as_bytes() == origin.as_bytes())
            });

        let app = App::new()
            .wrap(cors)
            .wrap(Logger::new(
                "%a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T %{X-Error-Id}o",
            ))
            //.wrap(Logger::new("%a %{User-Agent}i"))
            .wrap(middleware::Compress::default())
            .app_data(app_state.clone())
            .route("/echo", web::get().to(api::echo_handler))
            .route("/settings", web::get().to(api::settings_handler))

            // Download endpoint.
            // - JwtAuthMethod::OneTime: denies re-downloads (bitset enforcement)
            // - JwtAuthMethod::{Standard,None}: streams/proxies without one-time enforcement
            .route(
                "/files/{session_id}/{file_index}",
                web::get().to(api::download_file_handler),
            )

            .service(web::scope("/studyToken")
                .app_data(web::QueryConfig::default().error_handler(|err, _req| {
                    // Keep client-facing errors sanitized and consistent.
                    // AppError logs the detail (we only emit a generic 400).
                    actix_web::error::InternalError::from_response(
                        err,
                        AppError::bad_request("invalid query parameters").error_response(),
                    )
                    .into()
                }))
                .service(web::resource("")
                    .route(actix_web::web::get().to(api::study_token_handler)))
            )
            
           
            // -- WADO PROXY------------------------------------------------------------------------------------ //
            .route("/wado", web::get().to(api::wado_handler))
            

            // -- QIDO ------------------------------------------------------------------------------------------ //
            // SearchForStudies
            .service(web::scope("/qido")
                .service(
                    web::resource("/studies")
                        .app_data(qido_qs.clone())
                        .route(web::get().to(api::qido_studies_handler))
                )
                // SearchForSeries
                .service(
                    web::resource("/studies/{StudyInstanceUID}/series")
                        .app_data(qido_qs.clone())
                        .route(web::get().to(|| async { HttpResponse::NotImplemented().finish() }))
                )
                .service(
                    web::resource("/series")
                        .app_data(qido_qs.clone())
                        .route(web::get().to(|| async { HttpResponse::NotImplemented().finish() }))
                )
                // SearchForInstances
                .service(
                    web::resource("/studies/{StudyInstanceUID}/series/{SeriesInstanceUID}/instances")
                        .app_data(qido_qs.clone())
                        .route(web::get().to(|| async { HttpResponse::NotImplemented().finish() }))
                )
                .service(
                    web::resource("/studies/{StudyInstanceUID}/instances")
                        .app_data(qido_qs.clone())
                        .route(web::get().to(|| async { HttpResponse::NotImplemented().finish() }))
                )
                .service(
                    web::resource("/instances")
                        .app_data(qido_qs.clone())
                        .route(web::get().to(|| async { HttpResponse::NotImplemented().finish() }))
                ) 
            );
            app
    })
    .bind(bind)
    .context("Failed to bind server")?
    .run()
    .await
    .context("Server error")?;
    
    Ok(())
}
