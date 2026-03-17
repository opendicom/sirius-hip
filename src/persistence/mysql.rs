use anyhow::Ok;
use async_trait::async_trait;
use sqlx::MySqlPool;

use crate::settings::DBVersion;
use super::JwtRepository;

pub struct MySqlJwtRepository {
     pool: MySqlPool,
}

impl MySqlJwtRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl JwtRepository for MySqlJwtRepository {

    // Initialize necessary tables for JWT repository
    // depending on the database version
    // Here we create two tables: HIP_sessions and HIP_session_files
    // The field type for file_id in HIP_session_files depends on the DB version
    async fn initialize_tables(&self, db_version: &DBVersion) -> anyhow::Result<()> {   
        log::debug!("Initializing JWT repository tables for MySQL");
        
        let (pacs_file_table_name, pacs_file_table_pk_field, pacs_file_table_pk_field_type) = match db_version {
            DBVersion::dcm4chee2183 => ("files", "pk", "BIGINT(20)"),
            DBVersion::dcm4chee440 => ("file_ref", "pk", "BIGINT(20)"),
        };

        let mut transaction = self.pool.begin().await?;

        log::debug!("Creating table HIP_sessions if not exists");
        sqlx::query(format!("
            CREATE TABLE IF NOT EXISTS HIP_sessions (
                id            BINARY(16) PRIMARY KEY NOT NULL,
                expires_at    INT UNSIGNED NOT NULL,
                bitset        BLOB NOT NULL,
                total_files   INTEGER NOT NULL,
                created_at    TIMESTAMP NOT NULL DEFAULT NOW()
            );").as_str()
        ).execute(&mut *transaction)
        .await?;

        log::debug!("Creating table HIP_session_files if not exists");
        sqlx::query(format!("
            CREATE TABLE IF NOT EXISTS HIP_session_files (
                session_id          BINARY(16) NOT NULL,
                file_index          INTEGER NOT NULL,
                file_id             {pacs_file_table_pk_field_type} NOT NULL,

                PRIMARY KEY (session_id, file_index),
                FOREIGN KEY (session_id) REFERENCES HIP_sessions(id),
                FOREIGN KEY (file_id) REFERENCES {pacs_file_table_name}({pacs_file_table_pk_field})
            );").as_str()
        ).execute(&mut *transaction)
        .await?;

        transaction.commit().await?;    

        Ok(())
    }

    async fn delete(&self, _id: i64) -> () {
        // TODO: implement delete logic here
        ()
    }
}
