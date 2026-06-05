use std::path::PathBuf;

use actix_web::{web, App, HttpServer};
use thiserror::Error;
use ::time::macros::format_description;
use tracing_subscriber::{EnvFilter, fmt::time};
use tracing::{info, error};
use clap::{arg, Command, crate_version};

use super::router;
use crate::shared::config::{load_settings, ConfigError};
use crate::bootstrap::dependencies::{DependencyBuildError, build_dependencies};
use crate::bootstrap::state::AppState;


// -- Application Run Error implementations -------------------------------------------------------------------------------- //

#[derive(Debug, Error)]
pub enum AppRunError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to build application dependencies")]
    Dependency(#[from] DependencyBuildError),
}


// -- Application Run implementations -------------------------------------------------------------------------------- //


/// Run the application, initializing tracing, loading configuration, building dependencies, and starting the server.
pub async fn run() -> Result<(), AppRunError> {
    
    
    // -- Initialize tracing ----------------------------------------------------------------------------- //
    init_tracing();

    info!("Starting Sirius HIP v{}", crate_version!());


    // -- Command line arguments  --------------------------------------------------------------------- //
    let matches = Command::new("Sirius HIP")
        .about("The integration platform for Sirius PACS")
        .after_help("HIP = Health Integration Platform")
        .version(crate_version!())
        .author("Opendicom")
        .arg(arg!(-c --config <file> "Filepath to the configuration file")
            .default_value("sirius-hip.toml"))
        .get_matches();

    
    // -- Load settings  from configuration file ----------------------------------------------------------------- //
    let conf_file = PathBuf::from(matches.get_one::<String>("config").unwrap());
    info!(config_file = %conf_file.display(), "Using configuration file");
    let settings = load_settings(conf_file)?;
    
    
    // -- Build dependencies -------------------------------------------------------------------------------------- //
    info!("Building application dependencies");
    let dependencies = build_dependencies(&settings).await?;

    
    // -- Set up application state  ------------------------------------------------------------------------------- //
    let state = AppState::from_parts(settings, dependencies);
    let bind = state.settings.server.bind.clone();
    let app_state = web::Data::new(state);

    
    // -- Start the server ---------------------------------------------------------------------------------------- //
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            //.service(web::scope("/api")
            .configure(router::build_router)
    })
    .shutdown_timeout(60)
    .bind(&bind)?
    .run()
    .await
    .map_err(AppRunError::from)
}


/// Initialize tracing with an environment filter, defaulting to "info" if not set.
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let timer = time::LocalTime::new(format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"));
    
    if tracing_subscriber::fmt()
        .with_timer(timer)
        .with_env_filter(env_filter)
        .try_init()
        .is_err()
    {
        // Already initialized by tests or parent process.
    }
}
