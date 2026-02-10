use sqlx::{Acquire, MySqlPool, Row};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicI64, Ordering};
use chrono::{DateTime, Utc};

use crate::src2::application::repositories::DownloadSessionRepository;
use crate::src2::application::models::{
    DownloadSession,
    DownloadSessionFile,
};
use crate::src2::errors::AppError;

// Opportunistic cleanup throttle for HIP_one_time_tokens.
// We want bounded growth without paying a DELETE on every request.
static LAST_ONE_TIME_TOKEN_CLEANUP_TS: AtomicI64 = AtomicI64::new(0);

#[derive(Debug, Clone, Copy)]
pub struct CleanupConfig {
    pub session_batch: usize,
    pub max_batches: usize,
    pub token_delete_limit: usize,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            session_batch: 200,
            max_batches: 20,
            token_delete_limit: 5000,
        }
    }
}

pub struct MySqlDownloadSessionRepository {
    pool: MySqlPool,
    cleanup_cfg: CleanupConfig,
}

impl MySqlDownloadSessionRepository {
    pub async fn new(pool: MySqlPool) -> Result<Self, AppError> {
        Self::new_with_config(pool, CleanupConfig::default()).await
    }

    pub async fn new_with_config(pool: MySqlPool, cleanup_cfg: CleanupConfig) -> Result<Self, AppError> {

        let mut tx = pool.begin().await?;

        // Create tables if missing (app DB is separate from PACS DB).
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS HIP_download_sessions (
                session_id        VARCHAR(64) PRIMARY KEY NOT NULL,
                expires_at        DATETIME NOT NULL,
                total_files       INT NOT NULL,
                created_at        DATETIME NOT NULL
            );
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Index for efficient cleanup and expired-session checks.
        // MySQL 5.7 doesn't support CREATE INDEX IF NOT EXISTS, so ignore duplicate-name errors.
        let _ = sqlx::query(
            r#"
            CREATE INDEX idx_hip_download_sessions_expires_at
            ON HIP_download_sessions (expires_at)
            "#,
        )
        .execute(&mut *tx)
        .await;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS HIP_download_session_files (
                session_id      VARCHAR(64) NOT NULL,
                file_index      INT NOT NULL,
                instance_uid    VARCHAR(250) NOT NULL,
                study_uid       VARCHAR(250) NOT NULL,
                series_uid      VARCHAR(250) NOT NULL,
                use_wado        BOOLEAN NOT NULL,
                filesystem_fk   INT NULL,
                relative_file_path TEXT NULL,

                PRIMARY KEY (session_id, file_index),
                FOREIGN KEY (session_id) REFERENCES HIP_download_sessions(session_id)
            );
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Claim table: small and hot. Enforces OneTime semantics via unique PK.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS HIP_download_session_claims (
                session_id VARCHAR(64) NOT NULL,
                file_index INT NOT NULL,
                claimed_at DATETIME NOT NULL,

                PRIMARY KEY (session_id, file_index),
                FOREIGN KEY (session_id, file_index)
                  REFERENCES HIP_download_session_files(session_id, file_index)
                  ON DELETE CASCADE
            );
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // One-time studyToken JWT claims table.
        // We store a SHA-256 hash of the raw token string as a fixed-size primary key.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS HIP_one_time_tokens (
                token_hash  BINARY(32) PRIMARY KEY NOT NULL,
                expires_at  DATETIME NOT NULL,
                consumed_at DATETIME NOT NULL,

                INDEX idx_hip_one_time_tokens_expires_at (expires_at)
            );
            "#,
        )
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;

        Ok(Self { pool, cleanup_cfg })
    }
}



#[async_trait]
impl DownloadSessionRepository for MySqlDownloadSessionRepository {
    
    async fn create_session(&self, session: &DownloadSession) -> Result<(), AppError> {

        sqlx::query(
            r#"
            INSERT INTO HIP_download_sessions (
                session_id,
                expires_at,
                total_files,
                created_at
            )
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&session.session_id)
        .bind(session.expires_at)
        .bind(session.total_files as i64)
        .bind(session.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }


    async fn add_files(&self, files: &[DownloadSessionFile]) -> Result<(), AppError> {
        if files.is_empty() {
            return Ok(());
        }

        // Chunk inserts to avoid building a single enormous SQL statement
        // (important for sessions with thousands of files).
        const CHUNK_SIZE: usize = 500;

        for chunk in files.chunks(CHUNK_SIZE) {
            let mut query_builder = sqlx::QueryBuilder::new(
                "INSERT INTO HIP_download_session_files (session_id, file_index, instance_uid, study_uid, series_uid, use_wado, filesystem_fk, relative_file_path) ",
            );

            query_builder.push_values(chunk.iter(), |mut b, f| {
                b.push_bind(&f.session_id)
                    .push_bind(f.file_index as i64)
                    .push_bind(&f.instance_uid)
                    .push_bind(&f.study_uid)
                    .push_bind(&f.series_uid)
                    .push_bind(f.use_wado)
                    .push_bind(f.filesystem_fk)
                    .push_bind(&f.relative_file_path);
            });

            query_builder.build().execute(&self.pool).await?;
        }
        Ok(())
    }

    async fn get_file(&self, session_id: &str, file_index: u32) -> Result<DownloadSessionFile, AppError> {
        let row = sqlx::query(
            r#"
            SELECT session_id, file_index, instance_uid, study_uid, series_uid, use_wado, filesystem_fk, relative_file_path
            FROM HIP_download_session_files
            WHERE session_id = ? AND file_index = ?
            "#,
        )
        .bind(session_id)
        .bind(file_index as i64)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => AppError::FileIndexNotFound(file_index),
            other => AppError::Database(other),
        })?;

        Ok(DownloadSessionFile {
            session_id: row.try_get::<String, _>("session_id")?,
            file_index: (row.try_get::<i64, _>("file_index")? as u32),
            instance_uid: row.try_get("instance_uid")?,
            study_uid: row.try_get("study_uid")?,
            series_uid: row.try_get("series_uid")?,
            use_wado: row.try_get("use_wado")?,
            filesystem_fk: row.try_get("filesystem_fk")?,
            relative_file_path: row.try_get("relative_file_path")?,
        })
    }

    async fn consume_session(&self, session_id: &str) -> Result<(), AppError> {
        // Validate session first to return correct errors.
        let row = sqlx::query(
            r#"
            SELECT expires_at
            FROM HIP_download_sessions
            WHERE session_id = ?
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Err(AppError::DownloadSessionNotFound);
        };

        let expires_at: chrono::DateTime<chrono::Utc> = row.try_get("expires_at")?;
        if expires_at < chrono::Utc::now() {
            return Err(AppError::DownloadSessionExpired);
        }

        // Consume all files by inserting claims. IGNORE makes this idempotent.
        sqlx::query(
            r#"
            INSERT IGNORE INTO HIP_download_session_claims (session_id, file_index, claimed_at)
            SELECT f.session_id, f.file_index, UTC_TIMESTAMP()
            FROM HIP_download_session_files f
            WHERE f.session_id = ?
            "#,
        )
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn claim_file(&self, session_id: &str, file_index: u32) -> Result<DownloadSessionFile, AppError> {
        // Fast-path: claim via INSERT into the small claim table.
        // This only succeeds when:
        // - session exists and is not expired
        // - file exists
        // - file was not already claimed
        let insert_res = sqlx::query(
            r#"
                                                INSERT INTO HIP_download_session_claims (session_id, file_index, claimed_at)
            SELECT f.session_id, f.file_index, UTC_TIMESTAMP()
                                                FROM HIP_download_session_files f
                                                JOIN HIP_download_sessions s ON s.session_id = f.session_id
            WHERE f.session_id = ?
              AND f.file_index = ?
              AND s.expires_at >= UTC_TIMESTAMP()
            "#,
        )
        .bind(session_id)
        .bind(file_index as i64)
        .execute(&self.pool)
        .await;

        match insert_res {
            Ok(r) => {
                if r.rows_affected() == 1 {
                    return self.get_file(session_id, file_index).await;
                }
            }
            Err(e) => {
                // ER_DUP_ENTRY = #(1062) 23000
                // Duplicate entry means the file was already claimed.
                let duplicate = match &e {
                    sqlx::Error::Database(db) => db.code().as_deref() == Some("23000"),
                    _ => false,
                };
                if duplicate {
                    return Err(AppError::FileAlreadyDownloaded(file_index));
                }
                return Err(AppError::Database(e));
            }
        }

        // Slow-path: determine why the claim failed.
        let row = sqlx::query(
            r#"
            SELECT
                s.expires_at AS expires_at,
                f.session_id IS NOT NULL AS file_exists,
                c.session_id IS NOT NULL AS claimed
            FROM HIP_download_sessions s
            LEFT JOIN HIP_download_session_files f
              ON f.session_id = s.session_id
             AND f.file_index = ?
            LEFT JOIN HIP_download_session_claims c
              ON c.session_id = s.session_id
             AND c.file_index = ?
            WHERE s.session_id = ?
            "#,
        )
        .bind(file_index as i64)
        .bind(file_index as i64)
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Err(AppError::DownloadSessionNotFound);
        };

        let expires_at: chrono::DateTime<chrono::Utc> = row.try_get("expires_at")?;
        if expires_at < chrono::Utc::now() {
            return Err(AppError::DownloadSessionExpired);
        }

        let claimed: bool = row.try_get("claimed")?;
        if claimed {
            return Err(AppError::FileAlreadyDownloaded(file_index));
        }

        let file_exists: bool = row.try_get("file_exists")?;
        if !file_exists {
            return Err(AppError::FileIndexNotFound(file_index));
        }

        Err(AppError::FileIndexNotFound(file_index))
    }

    async fn claim_one_time_token(&self, token: &str, exp: usize) -> Result<(), AppError> {
        // Opportunistic cleanup: keep the table bounded without requiring external cron.
        // Throttle to at most once every 10 minutes per process.
        // Using LIMIT avoids large delete bursts under high churn.
        let now_ts = chrono::Utc::now().timestamp();
        let last_ts = LAST_ONE_TIME_TOKEN_CLEANUP_TS.load(Ordering::Relaxed);
        if now_ts > last_ts && (now_ts - last_ts) >= 10 * 60 {
            if LAST_ONE_TIME_TOKEN_CLEANUP_TS
                .compare_exchange(last_ts, now_ts, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                let _ = sqlx::query(
                    r#"
                    DELETE FROM HIP_one_time_tokens
                    WHERE expires_at < UTC_TIMESTAMP()
                    LIMIT 1000
                    "#,
                )
                .execute(&self.pool)
                .await;
            }
        }

        let expires_at = chrono::DateTime::<chrono::Utc>::from_timestamp(exp as i64, 0)
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Invalid expiration timestamp")))?;

        let token_hash: Vec<u8> = Sha256::digest(token.as_bytes()).to_vec();

        let insert_res = sqlx::query(
            r#"
            INSERT INTO HIP_one_time_tokens (token_hash, expires_at, consumed_at)
            VALUES (?, ?, UTC_TIMESTAMP())
            "#,
        )
        .bind(token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await;

        match insert_res {
            Ok(_) => Ok(()),
            Err(e) => {
                // ER_DUP_ENTRY = #(1062) 23000
                // Duplicate entry means the token was already consumed.
                let duplicate = match &e {
                    sqlx::Error::Database(db) => db.code().as_deref() == Some("23000"),
                    _ => false,
                };
                if duplicate {
                    return Err(AppError::TokenAlreadyUsed);
                }
                Err(AppError::Database(e))
            }
        }
    }

    async fn cleanup_expired(&self, cutoff: DateTime<Utc>) -> Result<(), AppError> {
        // Multi-instance safe: only one process should clean at a time.
        // MySQL GET_LOCK is connection-scoped, cheap, and works on 5.7.
        const LOCK_NAME: &str = "hip_cleanup_onetime_v1";
        let session_batch = self.cleanup_cfg.session_batch.max(1).min(5000);
        let max_batches = self.cleanup_cfg.max_batches.max(1).min(500);
        let token_delete_limit = self.cleanup_cfg.token_delete_limit.max(1).min(200_000);

        // IMPORTANT: GET_LOCK is scoped to the DB connection, not the pool.
        // Acquire one connection and run all cleanup queries on it.
        let mut conn = self.pool.acquire().await?;

        let got_lock: i64 = sqlx::query_scalar("SELECT GET_LOCK(?, 0)")
            .bind(LOCK_NAME)
            .fetch_one(&mut *conn)
            .await
            .unwrap_or(0);

        if got_lock != 1 {
            return Ok(());
        }

        let cleanup_res: Result<(), AppError> = async {
            // 1) Purge old one-time token hashes (safe after cutoff).
            // Keep this bounded in case there is high churn.
            let _ = sqlx::query(
                r#"
                DELETE FROM HIP_one_time_tokens
                WHERE expires_at < ?
                LIMIT ?
                "#,
            )
            .bind(cutoff)
            .bind(token_delete_limit as i64)
            .execute(&mut *conn)
            .await;

            // 2) Purge expired download sessions in batches.
            for _ in 0..max_batches {
                let session_ids: Vec<String> = sqlx::query_scalar(
                    r#"
                    SELECT session_id
                    FROM HIP_download_sessions
                    WHERE expires_at < ?
                    ORDER BY expires_at ASC
                    LIMIT ?
                    "#,
                )
                .bind(cutoff)
                .bind(session_batch as i64)
                .fetch_all(&mut *conn)
                .await?;

                if session_ids.is_empty() {
                    break;
                }

                let mut tx = conn.begin().await?;

                // Delete files first (claims cascade from files). Sessions have an FK from files.
                {
                    let mut qb = sqlx::QueryBuilder::new(
                        "DELETE FROM HIP_download_session_files WHERE session_id IN (",
                    );
                    let mut separated = qb.separated(",");
                    for id in &session_ids {
                        separated.push_bind(id);
                    }
                    separated.push_unseparated(")");
                    qb.build().execute(&mut *tx).await?;
                }

                {
                    let mut qb = sqlx::QueryBuilder::new(
                        "DELETE FROM HIP_download_sessions WHERE session_id IN (",
                    );
                    let mut separated = qb.separated(",");
                    for id in &session_ids {
                        separated.push_bind(id);
                    }
                    separated.push_unseparated(")");
                    qb.build().execute(&mut *tx).await?;
                }

                tx.commit().await?;
            }

            Ok(())
        }
        .await;

        // Always release lock (best-effort).
        let _ = sqlx::query_scalar::<_, i64>("SELECT RELEASE_LOCK(?)")
            .bind(LOCK_NAME)
            .fetch_one(&mut *conn)
            .await;

        cleanup_res
    }


}

