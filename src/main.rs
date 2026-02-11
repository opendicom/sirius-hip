use actix_web::{web, App, HttpServer, middleware, guard,};
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
use crate::src2::pacs::repositories::factory::StudyRepositoryFactory;
use crate::src2::state2::{AppState2, PacsState2};
use crate::src2::errors::app_error::AppError;

mod settings;
mod api;
mod error;
mod constants;
mod models;
mod database;
mod auth;
mod persistence;
mod utils;
mod state;

mod src2;


#[actix_web::main]
async fn main() -> anyhow::Result<()> {

    std::env::set_var("RUST_LOG", "debug");

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
    
    dbg!("Settings loaded: {:?}", &settings);

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
    let study_repo = StudyRepositoryFactory::create(&settings).await?;


    let download_session_repo = crate::src2::init::init_download_session_repo(&settings).await?;

    // Background cleanup for OneTime persistence (MySQL app-DB).
    // Multi-instance safe: the MySQL repo uses GET_LOCK so only one instance cleans.
    if settings.onetime_cleanup.enabled
        && settings
            .app_database_url
            .as_deref()
            .map(|u| u.starts_with("mysql://"))
            .unwrap_or(false)
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
            actix_web::error::ErrorBadRequest(err)
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

        let mut app = App::new()
            .wrap(cors)
            .wrap(Logger::default())
            //.wrap(Logger::new("%a %{User-Agent}i"))
            .wrap(middleware::Compress::default())
            .app_data(app_state.clone())
            .route("/echo", web::get().to(api::echo::endpoint))
            .route("/settings", web::get().to(api::setting::endpoint))

            // Download endpoint.
            // - JwtAuthMethod::OneTime: denies re-downloads (bitset enforcement)
            // - JwtAuthMethod::{Standard,None}: streams/proxies without one-time enforcement
            .route(
                "/files/{token}",
                web::get().to(src2::api::download_token_handler),
            )
            .route(
                "/files/{session_id}/{file_index}",
                web::get().to(src2::api::download_file_handler),
            )

            .service(web::scope("/studyToken")
                .app_data(web::QueryConfig::default().error_handler(|err, _req| {
                    // Keep client-facing errors sanitized and consistent.
                    // AppError logs the detail (we only emit a generic 400).
                    actix_web::error::InternalError::from_response(
                        err,
                        AppError::BadRequest.error_response(),
                    )
                    .into()
                }))
                .service(web::resource("")
                    .route(actix_web::web::get().to(src2::api::study_token_handler)))
            )
            
           
           // -- WADO PROXY----------------------------------------------------------------------------------------//
            .route("/wado", web::get().to(api::wado::endpoint))
            

            // -- QIDO ------------------------------------------------------------------------------------------ //
            // SearchForStudies
            .service(web::scope("/qido")
                .service(
                    web::resource("/studies")
                        .app_data(qido_qs.clone())
                        .guard(guard::Header("content-type","application/json"))
                        .route(web::get().to(src2::api::qido_studies_handler))
                )
                // SearchForSeries
                .service(
                    web::resource("/studies/{StudyInstanceUID}/series")
                        .app_data(qido_qs.clone())
                        .guard(guard::Header("content-type","application/json"))
                        .route(web::get().to(api::qido::series))
                )
                .service(
                    web::resource("/series")
                        .app_data(qido_qs.clone())
                        .guard(guard::Header("content-type","application/json"))
                        .route(web::get().to(api::qido::series))
                )
                // SearchForInstances
                .service(
                    web::resource("/studies/{StudyInstanceUID}/series/{SeriesInstanceUID}/instances")
                        .app_data(qido_qs.clone())
                        .guard(guard::Header("content-type","application/json"))
                        .route(web::get().to(api::qido::instances))
                )
                .service(
                    web::resource("/studies/{StudyInstanceUID}/instances")
                        .app_data(qido_qs.clone())
                        .guard(guard::Header("content-type","application/json"))
                        .route(web::get().to(api::qido::instances))
                )
                .service(
                    web::resource("/instances")
                        .app_data(qido_qs.clone())
                        .guard(guard::Header("content-type","application/json"))
                        .route(web::get().to(api::qido::instances))
                ) 
            );

            if settings.dicomarchive.custodianoid.is_some() {
                app = app.service(
                web::scope("/custodians")
                        .service(api::settings::oids)
                        .service(api::settings::aeis)
                );
            }
            if settings.dicomarchive.custodianoid.is_some() {
                app = app.service(
                    web::scope("/pacs")
                        .service(api::settings::wadouri)
                        .service(api::settings::stow)
                        .service(api::settings::qido)
                );
            }
            app
    })
    .bind(bind)
    .context("Failed to bind server")?
    .run()
    .await
    .context("Server error")?;
    
    Ok(())
}
