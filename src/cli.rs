//! CLI subcommands (`d3-plaintext-export`).
//!
//! `openaccounting export|import` thin wrappers over the library so
//! scripted round-trips work without the web UI. Both commands need
//! `DATABASE_URL` (from the environment or `.env`) and connect with
//! the same pool options the server uses.

use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::config::Config;
use crate::export::LedgerSnapshot;
use crate::import::pta::{self, PtaFormat};

/// Shared pool: small, because a CLI command is short-lived.
async fn connect(cfg: &Config) -> anyhow::Result<sqlx::PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&cfg.database_url)
        .await?;
    Ok(pool)
}

/// `openaccounting export --ledger=<id> --format=beancount`
///
/// Writes the ledger to stdout in the requested format.
pub async fn cmd_export(cfg: &Config, ledger_id: Uuid, format: PtaFormat) -> anyhow::Result<()> {
    let pool = connect(cfg).await?;
    let snapshot = LedgerSnapshot::load(&pool, ledger_id).await?;
    match format {
        PtaFormat::Beancount => print!("{}", crate::export::beancount::render(&snapshot)),
        PtaFormat::HledgerCsv => print!("{}", crate::export::hledger::render(&snapshot)),
    }
    Ok(())
}

/// `openaccounting import --ledger=<id> --format=beancount [--dry-run]`
///
/// Reads plain text from stdin and inserts the transactions. Prints
/// a per-row report (insert/skip/error) to stdout; `--dry-run`
/// prints the plan without writing anything.
pub async fn cmd_import(
    cfg: &Config,
    ledger_id: Uuid,
    format: PtaFormat,
    input: String,
    dry_run: bool,
) -> anyhow::Result<()> {
    let pool = connect(cfg).await?;
    // The CLI runs as the ledger owner; record that user as the
    // actor on imported transactions and audit entries.
    let actor_id: Option<Uuid> = sqlx::query_scalar("SELECT owner_id FROM ledgers WHERE id = $1")
        .bind(ledger_id)
        .fetch_optional(&pool)
        .await?;
    let Some(actor_id) = actor_id else {
        anyhow::bail!("ledger {ledger_id} not found");
    };

    let report = pta::import(&pool, ledger_id, actor_id, format, &input, dry_run).await?;

    for line in &report.planned {
        println!("{line}");
    }
    for err in &report.errors {
        eprintln!("error: {err}");
    }
    println!(
        "{}: {} inserted, {} skipped, {} errors ({} input)",
        if dry_run { "plan" } else { "result" },
        report.inserted,
        report.skipped,
        report.errors.len(),
        format.as_str(),
    );
    Ok(())
}
