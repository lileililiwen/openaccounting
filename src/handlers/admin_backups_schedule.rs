//! Admin HTTP endpoint for inspecting the scheduled-backup state
//! (`o2-scheduled-backups`).
//!
//! `GET /admin/backups/schedule` returns a small JSON / HTML page
//! listing the active cron expression, the last 20 backup runs
//! (successes and failures), and the on-disk backup filenames.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use axum_login::AuthSession;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    workers::backup::{BackupConfig, DEFAULT_BACKUP_CRON, DEFAULT_BACKUP_DIR},
    AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BackupRunRow {
    pub id: Uuid,
    pub scheduled_for: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: String,
    pub filename: Option<String>,
    pub size_bytes: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScheduleResponse {
    pub cron: String,
    pub keep: usize,
    pub dir: String,
    pub runs: Vec<BackupRunRow>,
}

/// `GET /admin/backups/schedule` — JSON for ad-hoc admin queries.
pub async fn schedule(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<impl IntoResponse> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    if user.role != "admin" {
        return Err(AppError::Forbidden);
    }

    let runs: Vec<BackupRunRow> = sqlx::query_as::<_, BackupRunRow>(
        "SELECT id, scheduled_for, started_at, finished_at, status, filename, size_bytes, error
         FROM backup_runs
         ORDER BY started_at DESC
         LIMIT 20",
    )
    .fetch_all(&state.pool)
    .await?;

    let cfg = BackupConfig::from_env();
    Ok(Json(ScheduleResponse {
        cron: if cfg.cron_expr.is_empty() {
            DEFAULT_BACKUP_CRON.to_string()
        } else {
            cfg.cron_expr
        },
        keep: cfg.keep,
        dir: cfg.dir.to_str().unwrap_or(DEFAULT_BACKUP_DIR).to_string(),
        runs,
    }))
}
