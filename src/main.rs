use std::error::Error as StdError;
use std::fmt::Write as _;

use tracing::error;

pub mod bootstrap;
pub mod shared;
pub mod features;
pub mod pacs;

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

    // Start the application, exiting with an error code if it fails to start.
    if let Err(err) = crate::bootstrap::app::run().await {
        log_startup_error(&err);
        std::process::exit(1);
    }
}

fn log_startup_error(err: &(dyn StdError + 'static)) {
    error!("{}", format_anyhow_style_error(err));
}

fn format_anyhow_style_error(err: &(dyn StdError + 'static)) -> String {
    let mut out = format!("Error: {err}");
    let mut source = err.source();

    if source.is_some() {
        out.push_str("\nCaused by:");
    }

    let mut index = 0usize;
    while let Some(cause) = source {
        let _ = write!(&mut out, "\n    {index}: {cause}");
        source = cause.source();
        index += 1;
    }

    out
}
