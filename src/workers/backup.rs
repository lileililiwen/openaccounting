//! Scheduled-backup worker (`o2-scheduled-backups`).
//!
//! A tokio task spawned at startup. Every 30 s it asks the cron
//! `Schedule` for the next tick. When the wall clock crosses that
//! tick, a backup run is recorded and executed.
//!
//! A backup is a single `tar.gz` containing:
//!
//! ```text
//! openaccounting_<UTC-timestamp>.tar.gz
//! ├── db.sql            (pg_dump output)
//! └── documents/        (the DOCUMENTS_DIR tree)
//! ```
//!
//! After the file is written, the `backup_runs` row is updated
//! with the filename, size, and status. Retention (`BACKUP_KEEP`,
//! default 7) then trims the oldest files and `backup_runs` rows.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use sqlx::PgPool;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::AppState;

/// Default cron expression: 02:00:00 every day.
///
/// Note: the `cron` crate (v0.12) requires seven fields:
/// `second minute hour day-of-month month day-of-week year`.
/// The five-field `0 2 * * *` form used by classic crontabs is
/// rejected by this parser.
pub const DEFAULT_BACKUP_CRON: &str = "0 0 2 * * * *";

/// Default retention: keep the 7 most recent backups.
pub const DEFAULT_BACKUP_KEEP: usize = 7;

/// Default destination directory.
pub const DEFAULT_BACKUP_DIR: &str = "./data/backups";

/// How often the worker checks the schedule. Coarser than one
/// minute so that we don't busy-loop on the cron parser.
const TICK_INTERVAL: Duration = Duration::from_secs(30);

/// Configuration for the backup worker.
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// Cron expression in 5-field standard format (min hour dom mon dow).
    pub cron_expr: String,
    /// Maximum number of backups to retain on disk.
    pub keep: usize,
    /// Destination directory for tarball files.
    pub dir: PathBuf,
    /// Path to the `pg_dump` binary (defaults to `pg_dump` on PATH).
    pub pg_dump_bin: String,
}

impl BackupConfig {
    /// Build from env vars. Defaults are applied when an env var
    /// is missing or unparseable.
    pub fn from_env() -> Self {
        let cron_expr = std::env::var("BACKUP_CRON")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_BACKUP_CRON.to_string());
        let keep: usize = std::env::var("BACKUP_KEEP")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_BACKUP_KEEP);
        let dir = PathBuf::from(
            std::env::var("BACKUP_DIR")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_BACKUP_DIR.to_string()),
        );
        let pg_dump_bin = std::env::var("PG_DUMP_BIN")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "pg_dump".to_string());
        Self {
            cron_expr,
            keep,
            dir,
            pg_dump_bin,
        }
    }

    /// Parse the cron expression or return an error. The
    /// `o2-scheduled-backups` spec requires the worker to fail
    /// fast on a bad expression.
    pub fn schedule(&self) -> Result<Schedule, cron::error::Error> {
        Schedule::from_str(&self.cron_expr)
    }
}

/// Spawn the worker. The returned `JoinHandle` is dropped when
/// the process exits; the task itself never returns.
pub fn spawn_backup_worker(state: AppState, cfg: BackupConfig) {
    let schedule = match cfg.schedule() {
        Ok(s) => s,
        Err(e) => {
            error!(
                cron = %cfg.cron_expr,
                error = %e,
                "BACKUP_CRON is not a valid cron expression; scheduled backups DISABLED"
            );
            return;
        }
    };

    let next_tick = match schedule.upcoming(Utc).next() {
        Some(t) => t,
        None => {
            error!(cron = %cfg.cron_expr, "cron expression produced no upcoming tick");
            return;
        }
    };

    info!(
        cron = %cfg.cron_expr,
        keep = cfg.keep,
        dir = %cfg.dir.display(),
        next_tick = %next_tick,
        "scheduled backup worker started"
    );

    tokio::spawn(async move {
        let mut next = next_tick;
        loop {
            let now = Utc::now();
            if now >= next {
                // Record the run and fire it.
                let run_id = match record_run_start(&state.pool, next).await {
                    Ok(id) => id,
                    Err(e) => {
                        error!(error = %e, "failed to record backup run");
                        // Skip ahead so we don't busy-loop on a
                        // persistent DB error.
                        next = schedule
                            .after(&next)
                            .next()
                            .unwrap_or(next + chrono::Duration::hours(1));
                        continue;
                    }
                };

                match run_backup(&state, &cfg, run_id, next).await {
                    Ok(filename) => {
                        info!(run = %run_id, filename, "scheduled backup complete");
                    }
                    Err(e) => {
                        warn!(run = %run_id, error = %e, "scheduled backup failed");
                        let _ = record_run_failure(&state.pool, run_id, &e.to_string()).await;
                    }
                }

                if let Err(e) = prune(&state.pool, &cfg).await {
                    warn!(error = %e, "backup retention prune failed");
                }

                next = match schedule.after(&next).next() {
                    Some(t) => t,
                    None => {
                        error!("cron produced no further ticks; worker exiting");
                        return;
                    }
                };
            }

            tokio::time::sleep(TICK_INTERVAL).await;
        }
    });
}

/// Insert a `backup_runs` row in `running` state. Returns the
/// generated id.
async fn record_run_start(pool: &PgPool, scheduled_for: DateTime<Utc>) -> sqlx::Result<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO backup_runs (kind, scheduled_for, status)
         VALUES ('scheduled', $1, 'running')
         RETURNING id",
    )
    .bind(scheduled_for)
    .fetch_one(pool)
    .await
}

/// Mark a run as failed with the given error message.
async fn record_run_failure(pool: &PgPool, run_id: Uuid, error_msg: &str) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE backup_runs
         SET status = 'failed', finished_at = now(), error = $2
         WHERE id = $1",
    )
    .bind(run_id)
    .bind(error_msg)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Run a single backup: write `pg_dump` to a temp file, tar the
/// documents dir into a temp file, then combine both into a
/// single `.tar.gz` on the destination. On success, record the
/// filename + size and return.
pub async fn run_backup(
    state: &AppState,
    cfg: &BackupConfig,
    run_id: Uuid,
    scheduled_for: DateTime<Utc>,
) -> Result<String, BackupError> {
    run_backup_with_pool(&state.pool, cfg, run_id, scheduled_for).await
}

/// Pool-only variant of [`run_backup`]. Used by tests and any
/// caller that already has a `PgPool` but does not need the
/// full `AppState`.
pub async fn run_backup_with_pool(
    pool: &PgPool,
    cfg: &BackupConfig,
    run_id: Uuid,
    scheduled_for: DateTime<Utc>,
) -> Result<String, BackupError> {
    tokio::fs::create_dir_all(&cfg.dir)
        .await
        .map_err(BackupError::Dir)?;

    let stamp = scheduled_for.format("%Y%m%d_%H%M%S");
    let filename = format!("openaccounting_{stamp}.tar.gz");
    let path = cfg.dir.join(&filename);

    let documents_dir =
        std::env::var("DOCUMENTS_DIR").unwrap_or_else(|_| "./data/documents".to_string());

    let tmp = tempfile::tempdir().map_err(BackupError::Tmp)?;
    let db_sql = tmp.path().join("db.sql");
    let docs_tar = tmp.path().join("documents.tar");

    dump_database(pool, &cfg.pg_dump_bin, &db_sql)
        .await
        .map_err(BackupError::PgDump)?;
    tar_directory(&documents_dir, &docs_tar)
        .await
        .map_err(BackupError::TarDocs)?;

    build_tarball(&path, &db_sql, &docs_tar)
        .await
        .map_err(BackupError::Tar)?;

    let size_bytes = tokio::fs::metadata(&path)
        .await
        .map_err(BackupError::Stat)?
        .len() as i64;

    sqlx::query(
        "INSERT INTO backups (filename, size_bytes, created_by, kind)
         VALUES ($1, $2, NULL, 'scheduled')",
    )
    .bind(&filename)
    .bind(size_bytes)
    .execute(pool)
    .await
    .map_err(|e| BackupError::Internal(format!("insert backup row: {e}")))?;

    sqlx::query(
        "UPDATE backup_runs
         SET status = 'success', finished_at = now(),
             filename = $2, size_bytes = $3, error = NULL
         WHERE id = $1",
    )
    .bind(run_id)
    .bind(&filename)
    .bind(size_bytes)
    .execute(pool)
    .await
    .map_err(|e| BackupError::Internal(format!("update backup_runs: {e}")))?;

    crate::observability::metrics::backup_completed();
    Ok(filename)
}

/// Shell out to `pg_dump` and capture the SQL into `out`.
///
/// The binary is resolved via [`resolve_pg_dump`] so it matches the
/// server's major version.
async fn dump_database(pool: &PgPool, configured: &str, out: &Path) -> Result<(), String> {
    let url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set".to_string())?;
    let pg_dump = resolve_pg_dump(pool, configured)
        .await
        .map_err(|e| format!("resolving pg_dump: {e}"))?;
    let mut child = tokio::process::Command::new(&pg_dump)
        .arg("--no-owner")
        .arg("--no-privileges")
        .arg(&url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawning {pg_dump}: {e}"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "pg_dump stdout was not captured".to_string())?;
    let mut file = tokio::fs::File::create(out)
        .await
        .map_err(|e| format!("opening db.sql for write: {e}"))?;
    tokio::io::copy(&mut stdout, &mut file)
        .await
        .map_err(|e| format!("streaming pg_dump output: {e}"))?;
    let status = child
        .wait()
        .await
        .map_err(|e| format!("waiting on pg_dump: {e}"))?;
    if !status.success() {
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| "pg_dump stderr was not captured".to_string())?;
        let mut err_buf = String::new();
        use tokio::io::AsyncReadExt;
        let _ = stderr.read_to_string(&mut err_buf).await;
        return Err(format!("pg_dump exited with status {status}: {err_buf}"));
    }
    Ok(())
}

/// Major version of the PostgreSQL server behind `pool` (e.g. `16` for
/// 16.14). Uses `server_version_num`, which is `major * 10000 + minor`.
async fn server_major_version(pool: &PgPool) -> sqlx::Result<u32> {
    let version_num: i32 = sqlx::query_scalar(
        "SELECT current_setting('server_version_num')::integer",
    )
    .fetch_one(pool)
    .await?;
    Ok((version_num / 10000) as u32)
}

/// Pick a `pg_dump` binary whose major version matches the server.
///
/// `pg_dump` refuses to dump a server with a different major version, so on
/// hosts where an older client is first on PATH the backup fails with a
/// version-mismatch error. When `PG_DUMP_BIN` was explicitly configured it is
/// used verbatim; otherwise we query the server's major version and prefer the
/// matching client from the standard Debian/Ubuntu PGDG layout
/// (`/usr/lib/postgresql/<major>/bin/pg_dump`), falling back to `pg_dump` on
/// PATH when no version-matched client is installed.
async fn resolve_pg_dump(pool: &PgPool, configured: &str) -> Result<String, String> {
    if !configured.is_empty() && configured != "pg_dump" {
        return Ok(configured.to_string());
    }

    let major = match server_major_version(pool).await {
        Ok(m) => m,
        Err(_) => return Ok(configured.to_string()),
    };

    let candidates = [
        format!("/usr/lib/postgresql/{major}/bin/pg_dump"),
        "/usr/local/bin/pg_dump".to_string(),
    ];
    for candidate in candidates {
        if Path::new(&candidate).exists() {
            return Ok(candidate);
        }
    }
    Ok(configured.to_string())
}

/// Tar the documents directory into `out`. If the documents dir
/// does not exist, write an empty tar (so the resulting tarball
/// still has a `documents/` entry — the spec mandates the
/// structure even when empty).
async fn tar_directory(src: &str, out: &Path) -> Result<(), String> {
    let src_path = Path::new(src);
    if !tokio::fs::metadata(src_path).await.is_ok() {
        // Empty tar — just the header.
        let mut f = tokio::fs::File::create(out)
            .await
            .map_err(|e| format!("opening documents.tar for write: {e}"))?;
        let header = make_empty_tar_header();
        tokio::io::AsyncWriteExt::write_all(&mut f, &header)
            .await
            .map_err(|e| format!("writing empty tar: {e}"))?;
        // Two zero blocks to terminate.
        let zero_block = [0u8; 1024];
        tokio::io::AsyncWriteExt::write_all(&mut f, &zero_block)
            .await
            .map_err(|e| format!("writing tar terminator: {e}"))?;
        tokio::io::AsyncWriteExt::write_all(&mut f, &zero_block)
            .await
            .map_err(|e| format!("writing tar terminator: {e}"))?;
        return Ok(());
    }

    let status = tokio::process::Command::new("tar")
        .arg("-cf")
        .arg(out)
        .arg("-C")
        .arg(src_path.parent().unwrap_or(src_path))
        .arg(
            src_path
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new(".")),
        )
        .status()
        .await
        .map_err(|e| format!("spawning tar: {e}"))?;
    if !status.success() {
        return Err(format!("tar exited with status {status}"));
    }
    Ok(())
}

/// A minimal POSIX ustar header for a directory entry named
/// `documents/`. Used when the documents dir does not exist yet
/// so the tarball structure is invariant.
fn make_empty_tar_header() -> [u8; 512] {
    let mut h = [0u8; 512];
    // Name field is 9 bytes; the trailing null is already there
    // because the rest of the array starts as 0.
    h[0..9].copy_from_slice(b"documents");
    h[100..107].copy_from_slice(b"0000755"); // mode: dir, 0755
    h[108..115].copy_from_slice(b"0000000"); // uid
    h[116..123].copy_from_slice(b"0000000"); // gid
    h[124..135].copy_from_slice(b"00000000000"); // size (0)
    h[135] = 0; // mtime high
                // Checksum placeholder — the canonical tar convention is to
                // treat these 8 bytes as ASCII spaces during the checksum.
    h[148..156].copy_from_slice(b"        ");
    h[156] = b'5'; // typeflag: directory
    h[257..262].copy_from_slice(b"ustar");
    h[262] = 0; // version
                // Compute checksum: sum of all 512 bytes (with the 8
                // checksum bytes themselves counted as spaces).
    let sum: u32 = h.iter().map(|b| *b as u32).sum();
    let s = format!("{:06o}\0 ", sum);
    h[148..156].copy_from_slice(s.as_bytes());
    h
}

/// Combine `db.sql` and `documents.tar` into a single
/// `tar.gz` at `out`. Uses the `tar` binary for compression —
/// `flate2::write::GzEncoder` over a `tar::Builder` requires the
/// `tar` crate as a dep, which we don't need elsewhere.
async fn build_tarball(out: &Path, db_sql: &Path, docs_tar: &Path) -> Result<(), String> {
    let staging = out.with_extension("staging.tar");
    let db_name = db_sql
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("db.sql");
    let docs_name = docs_tar
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("documents.tar");

    let status = tokio::process::Command::new("tar")
        .arg("-cf")
        .arg(&staging)
        .arg("-C")
        .arg(db_sql.parent().unwrap_or(db_sql))
        .arg(db_name)
        .arg("-C")
        .arg(docs_tar.parent().unwrap_or(docs_tar))
        .arg(docs_name)
        .status()
        .await
        .map_err(|e| format!("spawning tar (stage): {e}"))?;
    if !status.success() {
        return Err(format!("tar stage exited with status {status}"));
    }

    // gzip the staged tar.
    let status = tokio::process::Command::new("gzip")
        .arg("-n")
        .arg("-f")
        .arg(&staging)
        .status()
        .await
        .map_err(|e| format!("spawning gzip: {e}"))?;
    if !status.success() {
        return Err(format!("gzip exited with status {status}"));
    }

    // `gzip -f` writes `<staging>.gz` — rename to `out`.
    tokio::fs::rename(format!("{}.gz", staging.display()), out)
        .await
        .map_err(|e| format!("renaming staged tarball: {e}"))?;
    Ok(())
}

/// Trim the on-disk backup directory to `cfg.keep` files, then
/// mirror the deletion in the `backup_runs` table.
pub async fn prune_for_test(pool: &PgPool, cfg: &BackupConfig) -> Result<(), String> {
    prune(pool, cfg).await
}

async fn prune(pool: &PgPool, cfg: &BackupConfig) -> Result<(), String> {
    let mut entries = tokio::fs::read_dir(&cfg.dir)
        .await
        .map_err(|e| format!("read_dir({}): {e}", cfg.dir.display()))?;
    let mut files: Vec<(String, std::time::SystemTime)> = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| format!("dir entry: {e}"))?
    {
        if let Ok(meta) = entry.metadata().await {
            if let Ok(modified) = meta.modified() {
                if let Some(name) = entry.file_name().to_str() {
                    files.push((name.to_string(), modified));
                }
            }
        }
    }
    // Newest first.
    files.sort_by_key(|f| std::cmp::Reverse(f.1));

    for (i, (name, _)) in files.iter().enumerate() {
        if i >= cfg.keep {
            let path = cfg.dir.join(name);
            if let Err(e) = tokio::fs::remove_file(&path).await {
                warn!(file = %name, error = %e, "failed to remove old backup");
                continue;
            }
            let _ = sqlx::query("DELETE FROM backup_runs WHERE filename = $1")
                .bind(name)
                .execute(pool)
                .await;
            let _ = sqlx::query("DELETE FROM backups WHERE filename = $1")
                .bind(name)
                .execute(pool)
                .await;
            info!(file = %name, "pruned old backup");
        }
    }
    Ok(())
}

/// Errors from the backup pipeline. Each variant is rendered into
/// the `backup_runs.error` column on failure.
#[derive(Debug)]
pub enum BackupError {
    Dir(std::io::Error),
    Tmp(std::io::Error),
    PgDump(String),
    TarDocs(String),
    Tar(String),
    Stat(std::io::Error),
    Internal(String),
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dir(e) => write!(f, "create backup dir: {e}"),
            Self::Tmp(e) => write!(f, "create tempdir: {e}"),
            Self::PgDump(e) => write!(f, "pg_dump: {e}"),
            Self::TarDocs(e) => write!(f, "tar documents: {e}"),
            Self::Tar(e) => write!(f, "tar: {e}"),
            Self::Stat(e) => write!(f, "stat tarball: {e}"),
            Self::Internal(e) => write!(f, "internal: {e}"),
        }
    }
}

impl std::error::Error for BackupError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tar_header_is_well_formed() {
        let h = make_empty_tar_header();
        // Name field is 100 bytes, null-padded; first 9 bytes
        // spell `documents` followed by a null.
        assert_eq!(&h[0..9], b"documents");
        assert_eq!(h[9], 0, "name must be null-terminated");
        // Type flag at offset 156 is `5` (directory).
        assert_eq!(h[156], b'5');
        // Magic at offset 257 is `ustar`.
        assert_eq!(&h[257..262], b"ustar");
        // Checksum: 6 octal digits at the start of [148..156]
        // encoding the unsigned sum of all 512 header bytes
        // (computed with the checksum field treated as 8 spaces
        // — the canonical tar convention). We verify the octal
        // is well-formed and round-trips to a sensible integer;
        // the exact match against the post-format byte sum is
        // intentionally not asserted because writing the
        // octal representation of a sum changes the bytes being
        // summed (off-by-the-ascii-value-of-the-octal-digits),
        // and POSIX tar implementations handle this by
        // recomputing the checksum on read.
        let stored_str = std::str::from_utf8(&h[148..154]).unwrap();
        let stored: u32 = u32::from_str_radix(stored_str, 8).expect("checksum must be valid octal");
        assert!(stored > 0, "checksum of a non-empty header must be > 0");
        // Recompute the checksum the way readers do: treat the
        // 8 checksum bytes as spaces and sum.
        let mut recomputed = h;
        for b in &mut recomputed[148..156] {
            *b = b' ';
        }
        let sum_with_spaces: u32 = recomputed.iter().map(|b| *b as u32).sum();
        assert_eq!(
            stored, sum_with_spaces,
            "checksum must equal sum-of-header-bytes-with-checksum-treated-as-spaces"
        );
    }

    #[test]
    fn default_cron_is_valid() {
        let s = Schedule::from_str(DEFAULT_BACKUP_CRON).expect("default cron must parse");
        assert!(s.upcoming(Utc).next().is_some());
    }

    #[test]
    fn explicit_defaults_match_constants() {
        // The constant values are the source of truth. We just
        // sanity-check them against a freshly-built config.
        let cfg = BackupConfig {
            cron_expr: DEFAULT_BACKUP_CRON.to_string(),
            keep: DEFAULT_BACKUP_KEEP,
            dir: PathBuf::from(DEFAULT_BACKUP_DIR),
            pg_dump_bin: "pg_dump".to_string(),
        };
        assert_eq!(cfg.cron_expr, DEFAULT_BACKUP_CRON);
        assert_eq!(cfg.keep, DEFAULT_BACKUP_KEEP);
        assert_eq!(cfg.dir, PathBuf::from(DEFAULT_BACKUP_DIR));
        assert_eq!(cfg.pg_dump_bin, "pg_dump");
        assert!(cfg.schedule().is_ok());
    }

    #[test]
    fn invalid_cron_yields_error() {
        let cfg = BackupConfig {
            cron_expr: "not a cron".into(),
            keep: 1,
            dir: PathBuf::from("/tmp"),
            pg_dump_bin: "pg_dump".into(),
        };
        assert!(cfg.schedule().is_err());
    }
}
