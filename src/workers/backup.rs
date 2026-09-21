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

use crate::storage::SharedStorage;
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

/// Default S3 key prefix for the S3 backup target.
pub const DEFAULT_BACKUP_S3_PREFIX: &str = "backups";

/// How often the worker checks the schedule. Coarser than one
/// minute so that we don't busy-loop on the cron parser.
const TICK_INTERVAL: Duration = Duration::from_secs(30);

/// Where backup tarballs are written (`ops-hardening`).
#[derive(Debug, Clone, Default)]
pub enum BackupTargetKind {
    /// Local directory (original behavior).
    #[default]
    Dir,
    /// Object storage via the [`Storage`] trait (S3 backend).
    /// `prefix` scopes backup keys (default `"backups"`).
    Store { prefix: String },
}

/// A resolved backup target: the config plus the store handle for
/// the `Store` variant.
#[derive(Clone)]
pub enum ResolvedTarget {
    Dir(PathBuf),
    Store {
        store: SharedStorage,
        prefix: String,
    },
}

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
    /// Backup destination: local dir or object storage.
    pub target: BackupTargetKind,
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
        let target = match std::env::var("BACKUP_TARGET")
            .ok()
            .map(|s| s.to_lowercase())
            .as_deref()
        {
            Some("s3") | Some("store") => BackupTargetKind::Store {
                prefix: std::env::var("BACKUP_S3_PREFIX")
                    .ok()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| DEFAULT_BACKUP_S3_PREFIX.to_string()),
            },
            _ => BackupTargetKind::Dir,
        };
        Self {
            cron_expr,
            keep,
            dir,
            pg_dump_bin,
            target,
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

    let target = match resolve_target(&cfg, &state.storage) {
        Ok(t) => t,
        Err(e) => {
            error!(error = %e, "backup target misconfigured; scheduled backups DISABLED");
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

                match run_backup_with_target(&state.pool, &cfg, &target, run_id, next).await {
                    Ok(filename) => {
                        info!(run = %run_id, filename, "scheduled backup complete");
                    }
                    Err(e) => {
                        warn!(run = %run_id, error = %e, "scheduled backup failed");
                        let _ = record_run_failure(
                            &state.pool,
                            run_id,
                            &sanitize_error(&e.to_string()),
                        )
                        .await;
                    }
                }

                if let Err(e) = prune_target(&state.pool, &target, cfg.keep).await {
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

/// Resolve the configured target against the app's storage
/// backend. An S3 target with a non-S3 store fails fast so a
/// misconfiguration never silently writes backups to the wrong
/// place.
pub fn resolve_target(
    cfg: &BackupConfig,
    store: &SharedStorage,
) -> Result<ResolvedTarget, BackupError> {
    match &cfg.target {
        BackupTargetKind::Dir => Ok(ResolvedTarget::Dir(cfg.dir.clone())),
        BackupTargetKind::Store { prefix } => {
            if store.backend_label() != "s3" {
                return Err(BackupError::Internal(
                    "BACKUP_TARGET=s3 requires STORAGE_BACKEND=s3".to_string(),
                ));
            }
            Ok(ResolvedTarget::Store {
                store: store.clone(),
                prefix: prefix.clone(),
            })
        }
    }
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
    let target = resolve_target(cfg, &state.storage)?;
    run_backup_with_target(&state.pool, cfg, &target, run_id, scheduled_for).await
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
    run_backup_with_target(
        pool,
        cfg,
        &ResolvedTarget::Dir(cfg.dir.clone()),
        run_id,
        scheduled_for,
    )
    .await
}

/// Target-explicit backup core shared by [`run_backup`] and
/// [`run_backup_with_pool`].
pub async fn run_backup_with_target(
    pool: &PgPool,
    cfg: &BackupConfig,
    target: &ResolvedTarget,
    run_id: Uuid,
    scheduled_for: DateTime<Utc>,
) -> Result<String, BackupError> {
    if let ResolvedTarget::Dir(dir) = target {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(BackupError::Dir)?;
    }

    let stamp = scheduled_for.format("%Y%m%d_%H%M%S");
    let filename = format!("openaccounting_{stamp}.tar.gz");

    let documents_dir =
        std::env::var("DOCUMENTS_DIR").unwrap_or_else(|_| "./data/documents".to_string());

    let tmp = tempfile::tempdir().map_err(BackupError::Tmp)?;
    let docs_tar = tmp.path().join("documents.tar");

    // Database payload: pg_dump for PostgreSQL, file snapshot
    // for SQLite deployments (`ops-hardening` parity).
    let db_entry = database_payload(pool, &cfg.pg_dump_bin, tmp.path())
        .await
        .map_err(BackupError::PgDump)?;
    tar_directory(&documents_dir, &docs_tar)
        .await
        .map_err(BackupError::TarDocs)?;

    let staged = tmp.path().join(&filename);
    build_tarball_with(&staged, &db_entry, &docs_tar)
        .await
        .map_err(BackupError::Tar)?;

    let bytes = tokio::fs::read(&staged).await.map_err(BackupError::Stat)?;
    let size_bytes = bytes.len() as i64;
    persist_bytes(target, &filename, &bytes).await?;

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

/// Write finished tarball bytes to the resolved target.
/// Returns the byte length for the `backups` row.
async fn persist_bytes(
    target: &ResolvedTarget,
    filename: &str,
    bytes: &[u8],
) -> Result<(), BackupError> {
    match target {
        ResolvedTarget::Dir(dir) => {
            tokio::fs::write(dir.join(filename), bytes)
                .await
                .map_err(BackupError::Stat)?;
            Ok(())
        }
        ResolvedTarget::Store { store, prefix } => {
            let key = store
                .backup_key(prefix, filename)
                .map_err(|e| BackupError::Store(format!("backup key: {e}")))?;
            store
                .write(&key, bytes)
                .await
                .map_err(|e| BackupError::Store(format!("backup write: {e}")))?;
            Ok(())
        }
    }
}

/// Produce the database payload for the tarball: `pg_dump` output
/// for PostgreSQL, a file snapshot for SQLite deployments.
/// Returns the payload path inside `staging` (`db.sql` or
/// `db.sqlite`).
async fn database_payload(
    pool: &PgPool,
    pg_dump_bin: &str,
    staging: &Path,
) -> Result<PathBuf, String> {
    let url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set".to_string())?;
    if is_sqlite_url(&url) {
        let out = staging.join("db.sqlite");
        snapshot_sqlite(&url, &out).await?;
        return Ok(out);
    }
    let out = staging.join("db.sql");
    dump_database(pool, pg_dump_bin, &out).await?;
    Ok(out)
}

/// True for `sqlite://…` database URLs (file snapshot applies).
pub fn is_sqlite_url(url: &str) -> bool {
    url.starts_with("sqlite://") || url.starts_with("sqlite:")
}

/// Copy the SQLite database file to `out`. The URL may carry
/// query parameters (`?mode=ro`); they are stripped before
/// resolving the filesystem path.
pub async fn snapshot_sqlite(url: &str, out: &Path) -> Result<(), String> {
    let path = sqlite_path(url)
        .ok_or_else(|| "cannot resolve sqlite file path from DATABASE_URL".to_string())?;
    tokio::fs::copy(&path, out)
        .await
        .map_err(|e| format!("copying sqlite file {}: {e}", path.display()))?;
    Ok(())
}

/// Filesystem path of the SQLite database file, if the URL is a
/// file-backed sqlite URL (`:memory:` returns `None`).
pub fn sqlite_path(url: &str) -> Option<PathBuf> {
    let rest = url
        .strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))?;
    let path_part = rest.split('?').next().unwrap_or(rest);
    if path_part.is_empty() || path_part == ":memory:" {
        return None;
    }
    Some(PathBuf::from(path_part))
}

/// Strip any password from a database URL before it is stored in
/// `backup_runs.error` or returned to callers (log-redaction
/// policy, `docs/threat-model.md`).
pub fn sanitize_error(msg: &str) -> String {
    let mut out = msg.to_string();
    // Redact `scheme://user:password@` credentials.
    let mut start = 0;
    while let Some(scheme) = out[start..].find("://") {
        let abs = start + scheme;
        if let Some(at) = out[abs..].find('@') {
            let abs_at = abs + at;
            // Only redact when there is a userinfo part (a `:`
            // or content between :// and @ on one line).
            let userinfo = &out[abs + 3..abs_at];
            if !userinfo.is_empty() && !userinfo.contains('/') && !userinfo.contains(' ') {
                out.replace_range(abs + 3..abs_at, "***");
                start = abs + 6;
                continue;
            }
        }
        start = abs + 3;
    }
    out
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
    let version_num: i32 =
        sqlx::query_scalar("SELECT current_setting('server_version_num')::integer")
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

/// Combine the database payload (`db.sql` or `db.sqlite`) and
/// `documents.tar` into a single `tar.gz` at `out`. Uses the `tar`
/// binary for compression — `flate2::write::GzEncoder` over a
/// `tar::Builder` requires the `tar` crate as a dep, which we
/// don't need elsewhere.
async fn build_tarball_with(out: &Path, db_entry: &Path, docs_tar: &Path) -> Result<(), String> {
    let staging = out.with_extension("staging.tar");
    let db_name = db_entry
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
        .arg(db_entry.parent().unwrap_or(db_entry))
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

/// Trim a `Store` target to `keep` newest tarballs, then mirror
/// the deletion in the `backup_runs`/`backups` tables. Unit-testable
/// against any [`Storage`] implementation (task 1.2).
pub async fn prune_target(
    pool: &PgPool,
    target: &ResolvedTarget,
    keep: usize,
) -> Result<(), String> {
    let (store, prefix) = match target {
        ResolvedTarget::Store { store, prefix } => (store, prefix),
        ResolvedTarget::Dir(dir) => {
            return prune_dir(pool, dir, keep).await;
        }
    };
    let mut objects = store
        .list(prefix)
        .await
        .map_err(|e| format!("list backups: {e}"))?;
    // Newest first (the backends already sort, but enforce it).
    objects.sort_by_key(|o| std::cmp::Reverse(o.modified_secs.unwrap_or(0)));
    for obj in objects.iter().skip(keep) {
        if let Err(e) = store.delete(&obj.key).await {
            warn!(object = %obj.name, error = %e, "failed to remove old backup");
            continue;
        }
        delete_backup_rows(pool, &obj.name).await;
        info!(object = %obj.name, "pruned old backup");
    }
    Ok(())
}

async fn delete_backup_rows(pool: &PgPool, filename: &str) {
    let _ = sqlx::query("DELETE FROM backup_runs WHERE filename = $1")
        .bind(filename)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM backups WHERE filename = $1")
        .bind(filename)
        .execute(pool)
        .await;
}

async fn prune_dir(pool: &PgPool, dir: &Path, keep: usize) -> Result<(), String> {
    let mut entries = tokio::fs::read_dir(dir)
        .await
        .map_err(|e| format!("read_dir({}): {e}", dir.display()))?;
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
        if i >= keep {
            let path = dir.join(name);
            if let Err(e) = tokio::fs::remove_file(&path).await {
                warn!(file = %name, error = %e, "failed to remove old backup");
                continue;
            }
            delete_backup_rows(pool, name).await;
            info!(file = %name, "pruned old backup");
        }
    }
    Ok(())
}

/// Restore statistics returned by [`restore`].
#[derive(Debug, Clone)]
pub struct RestoreStats {
    pub db_bytes: u64,
    pub doc_files: u64,
    pub elapsed_secs: u64,
}

/// Restore a backup tarball into `target_db_url` + `documents_dir`
/// (`ops-hardening` drill path).
///
/// - Extracts the tarball to a temp dir.
/// - `db.sql` is loaded with `psql`; `db.sqlite` is copied over the
///   target file for SQLite deployments.
/// - `documents.tar` is extracted into `documents_dir`.
/// - Database URLs are redacted from every error string.
pub async fn restore(
    tarball: &Path,
    target_db_url: &str,
    documents_dir: &Path,
) -> Result<RestoreStats, BackupError> {
    let started = std::time::Instant::now();
    let tmp = tempfile::tempdir().map_err(BackupError::Tmp)?;
    let status = tokio::process::Command::new("tar")
        .arg("-xzf")
        .arg(tarball)
        .arg("-C")
        .arg(tmp.path())
        .status()
        .await
        .map_err(|e| BackupError::Restore(format!("extracting tarball: {e}")))?;
    if !status.success() {
        return Err(BackupError::Restore(format!("tar extract exited {status}")));
    }

    let db_bytes: u64;
    let db_sql = tmp.path().join("db.sql");
    let db_sqlite = tmp.path().join("db.sqlite");
    if db_sql.exists() {
        db_bytes = tokio::fs::metadata(&db_sql)
            .await
            .map_err(BackupError::Stat)?
            .len();
        load_sql_dump(&db_sql, target_db_url).await?;
    } else if db_sqlite.exists() {
        db_bytes = tokio::fs::metadata(&db_sqlite)
            .await
            .map_err(BackupError::Stat)?
            .len();
        let target_path = sqlite_path(target_db_url).ok_or_else(|| {
            BackupError::Restore("sqlite backup needs a file-backed sqlite target URL".to_string())
        })?;
        if let Some(parent) = target_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(BackupError::Dir)?;
        }
        tokio::fs::copy(&db_sqlite, &target_path)
            .await
            .map_err(|e| BackupError::Restore(format!("restoring sqlite file: {e}")))?;
    } else {
        return Err(BackupError::Restore(
            "tarball contains neither db.sql nor db.sqlite".to_string(),
        ));
    }

    let docs_tar = tmp.path().join("documents.tar");
    let mut doc_files = 0u64;
    if docs_tar.exists() {
        tokio::fs::create_dir_all(documents_dir)
            .await
            .map_err(BackupError::Dir)?;
        let status = tokio::process::Command::new("tar")
            .arg("-xf")
            .arg(&docs_tar)
            .arg("-C")
            .arg(documents_dir)
            .status()
            .await
            .map_err(|e| BackupError::Restore(format!("extracting documents: {e}")))?;
        if !status.success() {
            return Err(BackupError::Restore(format!(
                "documents extract exited {status}"
            )));
        }
        doc_files = count_files(documents_dir).await;
    }

    Ok(RestoreStats {
        db_bytes,
        doc_files,
        elapsed_secs: started.elapsed().as_secs(),
    })
}

async fn load_sql_dump(dump: &Path, target_db_url: &str) -> Result<(), BackupError> {
    let psql = resolve_psql();
    let out = tokio::process::Command::new(&psql)
        .arg("--quiet")
        .arg("-v")
        .arg("ON_ERROR_STOP=1")
        .arg("-d")
        .arg(target_db_url)
        .arg("-f")
        .arg(dump)
        .output()
        .await
        .map_err(|e| BackupError::Restore(format!("spawning {psql}: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(BackupError::Restore(sanitize_error(&format!(
            "psql restore exited {}: {stderr}",
            out.status
        ))));
    }
    Ok(())
}

/// Pick a `psql` binary: `psql` on PATH, else the newest
/// version-matched client under `/usr/lib/postgresql/*/bin/`.
fn resolve_psql() -> String {
    if which_psql_exists("psql") {
        return "psql".to_string();
    }
    let mut best: Option<(u32, PathBuf)> = None;
    if let Ok(dir) = std::fs::read_dir("/usr/lib/postgresql") {
        for entry in dir.filter_map(|e| e.ok()) {
            let major: u32 = match entry.file_name().to_str().and_then(|s| s.parse().ok()) {
                Some(m) => m,
                None => continue,
            };
            let candidate = entry.path().join("bin").join("psql");
            if candidate.exists() && best.as_ref().is_none_or(|(m, _)| major > *m) {
                best = Some((major, candidate));
            }
        }
    }
    best.map(|(_, p)| p.display().to_string())
        .unwrap_or_else(|| "psql".to_string())
}

fn which_psql_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|d| d.join(name).exists()))
        .unwrap_or(false)
}

async fn count_files(dir: &Path) -> u64 {
    let mut count = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let mut entries = match tokio::fs::read_dir(&current).await {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
            match entry.file_type().await {
                Ok(ft) if ft.is_dir() => stack.push(entry.path()),
                Ok(ft) if ft.is_file() => count += 1,
                _ => {}
            }
        }
    }
    count
}

/// Trim the on-disk backup directory to `cfg.keep` files, then
/// mirror the deletion in the `backup_runs` table.
pub async fn prune_for_test(pool: &PgPool, cfg: &BackupConfig) -> Result<(), String> {
    prune_dir(pool, &cfg.dir, cfg.keep).await
}

/// Errors from the backup pipeline. Each variant is rendered into
/// the `backup_runs.error` column on failure (URLs redacted via
/// [`sanitize_error`]).
#[derive(Debug)]
pub enum BackupError {
    Dir(std::io::Error),
    Tmp(std::io::Error),
    PgDump(String),
    TarDocs(String),
    Tar(String),
    Stat(std::io::Error),
    Store(String),
    Restore(String),
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
            Self::Store(e) => write!(f, "backup storage: {e}"),
            Self::Restore(e) => write!(f, "restore: {e}"),
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
            target: BackupTargetKind::Dir,
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
            target: BackupTargetKind::Dir,
        };
        assert!(cfg.schedule().is_err());
    }

    /// In-memory [`Storage`] fake: S3-shaped keys, controllable
    /// modification times, no network.
    struct FakeStore {
        objects: std::sync::Mutex<std::collections::HashMap<String, (Vec<u8>, i64)>>,
    }

    impl FakeStore {
        fn new() -> Self {
            Self {
                objects: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
    }

    use crate::storage::{StorageKey, StoredObject};

    #[async_trait::async_trait]
    impl crate::storage::Storage for FakeStore {
        async fn allocate_path(
            &self,
            _transaction_id: Uuid,
            _original: &str,
        ) -> Result<StorageKey, crate::storage::StorageError> {
            Err(crate::storage::StorageError::NotFound)
        }

        fn key_from_stored(
            &self,
            _stored: &str,
        ) -> Result<StorageKey, crate::storage::StorageError> {
            Err(crate::storage::StorageError::NotFound)
        }

        fn root_for(&self) -> PathBuf {
            PathBuf::new()
        }

        async fn read(&self, key: &StorageKey) -> Result<Vec<u8>, crate::storage::StorageError> {
            let name = match key {
                StorageKey::S3 { key, .. } => key.clone(),
                _ => return Err(crate::storage::StorageError::NotFound),
            };
            self.objects
                .lock()
                .unwrap()
                .get(&name)
                .map(|(b, _)| b.clone())
                .ok_or(crate::storage::StorageError::NotFound)
        }

        async fn write(
            &self,
            key: &StorageKey,
            bytes: &[u8],
        ) -> Result<(), crate::storage::StorageError> {
            let name = match key {
                StorageKey::S3 { key, .. } => key.clone(),
                _ => return Err(crate::storage::StorageError::NotFound),
            };
            // Fake clock: each write is newer than the last.
            let seq = self.objects.lock().unwrap().len() as i64;
            self.objects
                .lock()
                .unwrap()
                .insert(name, (bytes.to_vec(), seq));
            Ok(())
        }

        async fn delete(&self, key: &StorageKey) -> Result<(), crate::storage::StorageError> {
            let name = match key {
                StorageKey::S3 { key, .. } => key.clone(),
                _ => return Err(crate::storage::StorageError::NotFound),
            };
            self.objects.lock().unwrap().remove(&name);
            Ok(())
        }

        async fn signed_url(
            &self,
            _key: &StorageKey,
            _ttl_secs: u32,
        ) -> Result<Option<String>, crate::storage::StorageError> {
            Ok(None)
        }

        fn backend_label(&self) -> &'static str {
            "s3"
        }

        fn backup_key(
            &self,
            prefix: &str,
            filename: &str,
        ) -> Result<StorageKey, crate::storage::StorageError> {
            Ok(StorageKey::S3 {
                bucket: "fake".to_string(),
                key: format!("{prefix}/{filename}"),
            })
        }

        async fn list(
            &self,
            prefix: &str,
        ) -> Result<Vec<StoredObject>, crate::storage::StorageError> {
            let mut out: Vec<StoredObject> = self
                .objects
                .lock()
                .unwrap()
                .iter()
                .filter(|(k, _)| k.starts_with(prefix))
                .map(|(k, (b, seq))| StoredObject {
                    key: StorageKey::S3 {
                        bucket: "fake".to_string(),
                        key: k.clone(),
                    },
                    name: k.clone(),
                    size_bytes: b.len() as u64,
                    modified_secs: Some(*seq),
                })
                .collect();
            out.sort_by_key(|o| std::cmp::Reverse(o.modified_secs.unwrap_or(0)));
            Ok(out)
        }
    }

    /// Task 1.2: the S3 target writes tarballs and prunes per the
    /// retention fixture (keep = 3 of 5).
    #[tokio::test]
    async fn s3_target_writes_and_prunes_per_retention() {
        let store: SharedStorage = std::sync::Arc::new(FakeStore::new());
        let target = ResolvedTarget::Store {
            store,
            prefix: "backups".to_string(),
        };
        for i in 0..5 {
            persist_bytes(
                &target,
                &format!("openaccounting_2026010{i}_120000.tar.gz"),
                b"fake",
            )
            .await
            .expect("write");
        }
        // Pool is unused by the Store prune path except for row
        // mirrors; a lazy pool is never connected.
        let pool = PgPool::connect_lazy("postgres://localhost/unused").expect("lazy pool");
        prune_target(&pool, &target, 3).await.expect("prune");
        let remaining = match &target {
            ResolvedTarget::Store { store, prefix } => store.list(prefix).await.expect("list"),
            ResolvedTarget::Dir(_) => unreachable!(),
        };
        assert_eq!(
            remaining.len(),
            3,
            "retention must keep 3, got {}",
            remaining.len()
        );
        let names: Vec<_> = remaining.iter().map(|o| o.name.clone()).collect();
        assert!(
            names.iter().any(|n| n.contains("20260104")),
            "newest must survive: {names:?}"
        );
        assert!(
            !names.iter().any(|n| n.contains("20260100")),
            "oldest must be pruned: {names:?}"
        );
    }

    #[test]
    fn sqlite_url_detection_and_path_resolution() {
        assert!(is_sqlite_url("sqlite:///var/lib/oa.db"));
        assert!(is_sqlite_url("sqlite:/var/lib/oa.db"));
        assert!(!is_sqlite_url("postgres://localhost/oa"));
        assert_eq!(
            sqlite_path("sqlite:///var/lib/oa.db?mode=ro"),
            Some(PathBuf::from("/var/lib/oa.db"))
        );
        assert_eq!(sqlite_path("sqlite://:memory:"), None);
    }

    #[test]
    fn sanitize_error_redacts_passwords() {
        let dirty = "psql restore exited 1: connection to postgres://bob:s3cret@db:5432/oa failed";
        let clean = sanitize_error(dirty);
        assert!(!clean.contains("s3cret"), "password must go: {clean}");
        assert!(!clean.contains("bob"), "username must go: {clean}");
        assert!(clean.contains("***"), "redaction marker must show: {clean}");
        let untouched = "tar extract exited 1";
        assert_eq!(sanitize_error(untouched), untouched);
    }
}
