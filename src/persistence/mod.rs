use std::{str::FromStr, sync::Arc};
use anyhow::Context;
use reqwest::Url;
use async_trait::async_trait;

use crate::{persistence::mysql::MySqlJwtRepository, settings::DBVersion, utils::db_url_hide_password};

mod mysql;

#[async_trait]
pub trait JwtRepository: Send + Sync {

    /// Delete a WZ session by its ID
    async fn delete(&self, id: i64) -> ();

    /// Initialize necessary tables for JWT repository
    /// depending on the database version
    /// Here we create two tables: HIP_sessions and HIP_session_files
    /// The field type for file_id in HIP_session_files depends on the DB version
    async fn initialize_tables(&self, db_version: &DBVersion) -> anyhow::Result<()>;
}


pub async fn build_jwt_repository(
    db_url: &str,
    db_max_conn: u32,
) -> anyhow::Result<Arc<dyn JwtRepository>> {
    let url = Url::parse(db_url)?;

    match url.scheme() {
        "mysql" => {
            let options = sqlx::mysql::MySqlConnectOptions::from_str(db_url)
                .with_context(|| format!("Failed to parse MySQL connection url {}", db_url_hide_password(db_url)))?;

            let pool = sqlx::mysql::MySqlPoolOptions::new()
                .max_connections(db_max_conn)
                .acquire_timeout(std::time::Duration::from_secs(6))
                .connect_with(options)
                .await
                .with_context(|| format!("Failed to create application mysql connection pool to {}", db_url_hide_password(db_url)))?;
            Ok(Arc::new(
                MySqlJwtRepository::new(pool),
            ))
        }

        scheme => anyhow::bail!("Unsupported database scheme: {}", scheme),
    }
}