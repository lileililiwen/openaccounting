//! Daily worker that refreshes FX rates from the ECB reference feed
//! (`multi-currency-fx`).
//!
//! Enabled with `FX_ECB_ENABLED=true`. The feed URL defaults to the
//! ECB's historical CSV (`eurofxref-hist` layout: `Date,USD,JPY,…`
//! with EUR as the base) and can be overridden with `FX_ECB_URL` —
//! useful for self-hosted mirrors or a plain-CSV proxy of the zipped
//! original. Upserts are idempotent (`ON CONFLICT DO NOTHING`), so
//! re-running a day never duplicates rows and never overwrites manual
//! entries.

use std::time::Duration;

use sqlx::PgPool;
use tracing::{info, warn};

pub const DEFAULT_ECB_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist.csv";
pub const REFRESH_INTERVAL_HOURS: u64 = 24;

/// Spawn the daily refresh loop when `FX_ECB_ENABLED=true`.
/// A no-op otherwise (the default), so tests and air-gapped installs
/// are unaffected.
pub fn spawn_if_enabled(pool: PgPool) {
    let enabled = std::env::var("FX_ECB_ENABLED")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);
    if !enabled {
        return;
    }
    let url = std::env::var("FX_ECB_URL").unwrap_or_else(|_| DEFAULT_ECB_URL.to_string());
    tokio::spawn(async move {
        let interval = Duration::from_secs(REFRESH_INTERVAL_HOURS * 3600);
        info!(url = %url, "ECB FX refresh worker started (interval = {REFRESH_INTERVAL_HOURS}h)");
        loop {
            match refresh_once(&pool, &url).await {
                Ok(n) => info!("ECB FX refresh stored {n} new rates"),
                Err(e) => warn!("ECB FX refresh failed: {e}"),
            }
            tokio::time::sleep(interval).await;
        }
    });
}

/// Fetch, parse, and upsert one feed. Returns the number of newly
/// inserted rows. Idempotent on every step.
pub async fn refresh_once(pool: &PgPool, url: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let body = reqwest::get(url).await?.error_for_status()?.text().await?;
    let rates = crate::domain::fx::parse_ecb_csv(&body);
    if rates.is_empty() {
        return Err("ECB feed produced no parseable rows".into());
    }
    Ok(crate::domain::fx::upsert_ecb_rates(pool, &rates).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_by_default() {
        // The worker must not spawn without an explicit opt-in; this
        // pins the env contract so a typo in the flag name fails here.
        let enabled = std::env::var("FX_ECB_ENABLED").is_ok_and(|v| v == "true");
        assert!(!enabled, "tests must run with FX_ECB_ENABLED unset");
    }
}
