//! Health and readiness endpoints (`o5-health-endpoint`).
//!
//! Two public probes for load balancers / Kubernetes:
//!
//! * `GET /healthz` — liveness. Returns 200 as long as the
//!   process is alive. No I/O.
//! * `GET /readyz` — readiness. Pings Postgres and verifies
//!   the documents directory is writable, each with a 1 s
//!   timeout. 200 when both succeed; 503 with a JSON
//!   `{ "status": "not_ready", "reason": "db" | "disk" }`
//!   otherwise.
//!
//! Both endpoints are mounted BEFORE the
//! `login_required!` middleware so they're reachable without
//! a session cookie.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::time::Duration;

/// Probe timeout per subsystem.
const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

/// GET /healthz — process-alive only.
pub async fn healthz() -> Response {
    (
        StatusCode::OK,
        [(
            header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        )],
        Json(json!({ "status": "ok" })),
    )
        .into_response()
}

/// GET /readyz — Postgres ping + documents dir writability,
/// each with a 1 s timeout.
pub async fn readyz(State(state): State<crate::AppState>) -> Response {
    let (status, reason) = match check_readiness(&state).await {
        Ok(()) => (StatusCode::OK, None),
        Err(r) => (StatusCode::SERVICE_UNAVAILABLE, Some(r)),
    };
    let body = match reason {
        Some(r) => json!({ "status": "not_ready", "reason": r }),
        None => json!({ "status": "ready" }),
    };
    (
        status,
        [(
            header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        )],
        Json(body),
    )
        .into_response()
}

/// Probe Postgres with a 1 s timeout. Returns Err("db") on
/// any failure.
async fn check_readiness(state: &crate::AppState) -> Result<(), &'static str> {
    // Postgres ping: the SELECT 1 round-trip exercises the pool.
    let ping = tokio::time::timeout(
        PROBE_TIMEOUT,
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.pool),
    )
    .await;
    match ping {
        Ok(Ok(_)) => {}
        Ok(Err(_)) | Err(_) => return Err("db"),
    }

    // Documents-dir writability. We open the directory in
    // append mode and write a one-byte probe; the FS removes
    // any rights to drop the probe before unlink. For non-FS
    // backends (S3) we just attempt a tiny `put_object` to
    // verify network reachability.
    let backend = state.storage.backend_label();
    let write: Result<(), &'static str> = if backend == "filesystem" {
        // The FS implementation has a `root` accessor via
        // `key_from_stored`. We probe a sentinel file in the
        // root by allocating a key for an empty string and
        // checking the parent dir is writable.
        let key = state
            .storage
            .key_from_stored(".healthz-probe")
            .map_err(|_| "storage")?;
        let probe_path = match key {
            crate::storage::StorageKey::Filesystem(p) => p,
            _ => return Err("db"),
        };
        let result = tokio::time::timeout(PROBE_TIMEOUT, async {
            use tokio::io::AsyncWriteExt;
            let mut f = tokio::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&probe_path)
                .await
                .map_err(|_| "storage")?;
            f.write_all(b".").await.map_err(|_| "storage")?;
            f.sync_all().await.map_err(|_| "storage")?;
            Ok::<_, &'static str>(())
        })
        .await;
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("storage"),
        }
    } else {
        // For S3: attempt a tiny upload as a reachability probe.
        // Failure means the bucket is unreachable.
        let key = state
            .storage
            .key_from_stored(".healthz-probe")
            .map_err(|_| "storage")?;
        let result = tokio::time::timeout(PROBE_TIMEOUT, state.storage.write(&key, b".")).await;
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) | Err(_) => Err("storage"),
        }
    };
    if write.is_err() {
        return Err("storage");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_timeout_is_one_second() {
        assert_eq!(PROBE_TIMEOUT, Duration::from_secs(1));
    }
}
