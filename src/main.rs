#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use openaccounting::config::Config;
use openaccounting::import::PtaFormat;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "openaccounting", about = "OpenAccounting server and PTA CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve the web UI.
    Serve,
    /// Export a ledger to plain text on stdout.
    Export {
        /// Ledger UUID.
        #[arg(long)]
        ledger: String,
        /// Output format: `beancount` (default) or `hledger-csv`.
        #[arg(long, default_value = "beancount")]
        format: String,
    },
    /// Import plain text from stdin into a ledger.
    Import {
        /// Ledger UUID.
        #[arg(long)]
        ledger: String,
        /// Input format: `beancount` (default) or `hledger-csv`.
        #[arg(long, default_value = "beancount")]
        format: String,
        /// Print the diff without inserting anything.
        #[arg(long)]
        dry_run: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,openaccounting=debug,sqlx=warn".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Serve => openaccounting::run().await,
        Command::Export { ledger, format } => {
            let cfg = Config::from_env()?;
            let ledger_id = Uuid::parse_str(&ledger)
                .map_err(|_| anyhow::anyhow!("--ledger must be a valid UUID, got '{ledger}'"))?;
            let format = PtaFormat::from_arg(&format)?;
            openaccounting::cli::cmd_export(&cfg, ledger_id, format).await
        }
        Command::Import {
            ledger,
            format,
            dry_run,
        } => {
            let cfg = Config::from_env()?;
            let ledger_id = Uuid::parse_str(&ledger)
                .map_err(|_| anyhow::anyhow!("--ledger must be a valid UUID, got '{ledger}'"))?;
            let format = PtaFormat::from_arg(&format)?;
            let mut input = String::new();
            use std::io::Read as _;
            std::io::stdin().read_to_string(&mut input)?;
            openaccounting::cli::cmd_import(&cfg, ledger_id, format, input, dry_run).await
        }
    }
}
