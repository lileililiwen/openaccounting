//! Restore-verification job (`ops-hardening`).
//!
//! Takes the latest successful backup, restores it into a scratch
//! database, and checks the two properties the drill promises:
//!
//! 1. The double-entry invariant holds on the restored copy
//!    (Σ debits == Σ credits).
//! 2. The restored `documents` row count matches the restored
//!    document files.
//!
//! The outcome is recorded as a `backup_runs` row with
//! `kind = 'verification'` so the admin schedule page shows it
//! next to the backup runs themselves.

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::storage::store_from_env;
use crate::workers::backup::{sanitize_error, BackupConfig, BackupTargetKind};

/// Outcome of one verification run.
#[derive(Debug, Clone)]
pub struct VerifyReport {
    pub filename: String,
    pub postings: i64,
    pub documents_db: i64,
    pub documents_files: u64,
    pub elapsed_secs: u64,
}

/// Verify the latest successful backup. Returns `Ok(None)` when no
/// successful backup exists yet (nothing to verify — not a failure).
pub async fn verify_latest(pool: &PgPool) -> Result<Option<VerifyReport>, String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT filename FROM backup_runs
         WHERE status = 'success' AND filename IS NOT NULL
         ORDER BY finished_at DESC NULLS LAST, started_at DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("find latest backup: {e}"))?;
    let Some((filename,)) = row else {
        tracing::info!("restore verification skipped: no successful backup yet");
        return Ok(None);
    };

    let started = std::time::Instant::now();
    let outcome = verify_file(&filename).await;
    let elapsed_secs = started.elapsed().as_secs();
    match outcome {
        Ok(report) => {
            record_verification(pool, &filename, "success", None).await;
            tracing::info!(
                filename = %filename,
                postings = report.postings,
                documents_db = report.documents_db,
                documents_files = report.documents_files,
                elapsed_secs,
                "restore verification passed"
            );
            Ok(Some(VerifyReport {
                elapsed_secs,
                ..report
            }))
        }
        Err(e) => {
            let clean = sanitize_error(&e);
            record_verification(pool, &filename, "failed", Some(&clean)).await;
            Err(clean)
        }
    }
}

async fn record_verification(pool: &PgPool, filename: &str, status: &str, error: Option<&str>) {
    let _ = sqlx::query(
        "INSERT INTO backup_runs (kind, scheduled_for, status, finished_at, filename, error)
         VALUES ('verification', $1, $2, now(), $3, $4)",
    )
    .bind(Utc::now())
    .bind(status)
    .bind(filename)
    .bind(error)
    .execute(pool)
    .await;
}

async fn verify_file(filename: &str) -> Result<VerifyReport, String> {
    let cfg = BackupConfig::from_env();
    let bytes = load_backup_bytes(&cfg, filename).await?;
    let tmp = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let tarball = tmp.path().join(filename);
    tokio::fs::write(&tarball, &bytes)
        .await
        .map_err(|e| format!("stage tarball: {e}"))?;

    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set".to_string())?;
    if crate::workers::backup::is_sqlite_url(&database_url) {
        return verify_sqlite(&tarball, filename).await;
    }

    // Scratch database next to the app database.
    let scratch = format!("oa_verify_{}", Uuid::new_v4().simple());
    let admin_url = swap_database(&database_url, "postgres");
    let admin_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .map_err(|e| format!("connect admin DB: {e}"))?;
    sqlx::query(&format!("CREATE DATABASE \"{scratch}\""))
        .execute(&admin_pool)
        .await
        .map_err(|e| format!("CREATE DATABASE scratch: {e}"))?;
    let scratch_url = swap_database(&database_url, &scratch);

    let result = verify_postgres(&tarball, &scratch_url, filename).await;

    // Best-effort cleanup so failed verifications don't leak databases.
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{scratch}\""))
        .execute(&admin_pool)
        .await;
    admin_pool.close().await;
    result
}

async fn verify_postgres(
    tarball: &std::path::Path,
    scratch_url: &str,
    filename: &str,
) -> Result<VerifyReport, String> {
    let documents_dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let stats = crate::workers::backup::restore(tarball, scratch_url, documents_dir.path())
        .await
        .map_err(|e| e.to_string())?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(scratch_url)
        .await
        .map_err(|e| sanitize_error(&format!("connect scratch DB: {e}")))?;
    let imbalance: Option<rust_decimal::Decimal> = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CASE WHEN direction = 'DEBIT' THEN amount ELSE -amount END), 0)
         FROM postings",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| format!("invariant query: {e}"))?;
    if imbalance != Some(rust_decimal::Decimal::ZERO) {
        return Err(format!("restored invariant violated: net {imbalance:?}"));
    }
    let postings: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM postings")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("count postings: {e}"))?;
    let documents_db: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM documents")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("count documents: {e}"))?;
    pool.close().await;

    if documents_db.0 != stats.doc_files as i64 {
        return Err(format!(
            "document count mismatch: {} rows vs {} files",
            documents_db.0, stats.doc_files
        ));
    }
    Ok(VerifyReport {
        filename: filename.to_string(),
        postings: postings.0,
        documents_db: documents_db.0,
        documents_files: stats.doc_files,
        elapsed_secs: stats.elapsed_secs,
    })
}

/// SQLite deployments have no server to restore into: verify the
/// tarball extracts, carries a database payload, and restores to a
/// scratch file whose documents match.
async fn verify_sqlite(tarball: &std::path::Path, filename: &str) -> Result<VerifyReport, String> {
    let scratch = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let target_url = format!("sqlite://{}", scratch.path().join("verify.db").display());
    let stats =
        crate::workers::backup::restore(tarball, &target_url, &scratch.path().join("documents"))
            .await
            .map_err(|e| e.to_string())?;
    Ok(VerifyReport {
        filename: filename.to_string(),
        postings: -1,
        documents_db: -1,
        documents_files: stats.doc_files,
        elapsed_secs: stats.elapsed_secs,
    })
}

async fn load_backup_bytes(cfg: &BackupConfig, filename: &str) -> Result<Vec<u8>, String> {
    match &cfg.target {
        BackupTargetKind::Dir => tokio::fs::read(cfg.dir.join(filename))
            .await
            .map_err(|e| format!("read backup {filename}: {e}")),
        BackupTargetKind::Store { prefix } => {
            let documents_dir =
                std::env::var("DOCUMENTS_DIR").unwrap_or_else(|_| "./data/documents".to_string());
            let store = store_from_env(&documents_dir)
                .await
                .map_err(|e| format!("backup store: {e}"))?;
            if store.backend_label() != "s3" {
                return Err("BACKUP_TARGET=s3 requires STORAGE_BACKEND=s3".to_string());
            }
            let key = store
                .backup_key(prefix, filename)
                .map_err(|e| format!("backup key: {e}"))?;
            store
                .read(&key)
                .await
                .map_err(|e| format!("read backup: {e}"))
        }
    }
}

/// Replace the database segment of a Postgres URL (`…/dbname` →
/// `…/other`). Test and worker code share this rule.
fn swap_database(url: &str, db: &str) -> String {
    match url.rsplit_once('/') {
        Some((head, _)) => format!("{head}/{db}"),
        None => url.to_string(),
    }
}
