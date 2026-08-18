//! Daily worker that anchors the audit chain's tail hash to an
//! append-only log file (`d1-audit-chain`, optional companion).
//!
//! Every 24 h it appends one line `ISO-8601 <hex-tail-hash>` to
//! `AUDIT_ANCHOR_FILE` (default `./data/audit-anchor.log`) and
//! fsyncs. The file is write-only evidence: if the DB is later
//! tampered with, the last written anchor cannot be retroactively
//! altered, so anyone comparing it with the current chain tail
//! detects the tamper even when the attacker rewrites the whole
//! chain.
//!
//! [`run_anchor_loop`] spawns the repeating task; [`run_once`]
//! does a single append (used by tests).

use std::time::Duration;

use sqlx::PgPool;
use tracing::{info, warn};

/// How often the anchor loop writes a new tail hash.
pub const ANCHOR_INTERVAL_HOURS: u64 = 24;

/// Spawn a tokio task that appends the tail hash daily.
pub fn run_anchor_loop(pool: PgPool) {
    tokio::spawn(async move {
        let interval = Duration::from_secs(ANCHOR_INTERVAL_HOURS * 3600);
        info!("audit-chain anchor worker started (interval = {ANCHOR_INTERVAL_HOURS}h)");
        loop {
            tokio::time::sleep(interval).await;
            match run_once(&pool).await {
                Ok(Some(hash)) => info!("audit anchor written: {hash}"),
                Ok(None) => {}
                Err(e) => warn!("audit anchor append failed: {e}"),
            }
        }
    });
}

/// Append the current tail hash to the anchor file. Returns the
/// hex tail hash, or `None` when the audit log is empty.
pub async fn run_once(pool: &PgPool) -> Result<Option<String>, std::io::Error> {
    let tail = crate::audit::chain::latest_hash_hex(pool)
        .await
        .map_err(std::io::Error::other)?;
    let Some(hash) = tail else {
        return Ok(None);
    };

    let path = std::env::var("AUDIT_ANCHOR_FILE")
        .unwrap_or_else(|_| "./data/audit-anchor.log".to_string());
    if let Some(parent) = std::path::Path::new(&path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let line = format!("{} {hash}\n", chrono::Utc::now().to_rfc3339());
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await?;
    use tokio::io::AsyncWriteExt;
    file.write_all(line.as_bytes()).await?;
    file.sync_all().await?;
    Ok(Some(hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_is_twenty_four_hours() {
        assert_eq!(ANCHOR_INTERVAL_HOURS, 24);
    }
}
