//! Bank feeds — provider-pluggable live transaction sync.
//!
//! The `Provider` trait is the single abstraction all adapters implement.
//! Available providers: plaid, gocardless, enable_banking, salt_edge, simplefin, manual.

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use thiserror::Error;

pub mod crypto;
pub mod enable_banking;
pub mod gocardless;
pub mod manual;
pub mod plaid;
pub mod salt_edge;
pub mod simplefin;

// ─── Shared types ──────────────────────────────────────────────────────────

/// A single transaction fetched from the bank provider.
#[derive(Debug, Clone)]
pub struct RemoteTransaction {
    /// Provider-assigned unique ID (used for deduplication).
    pub provider_txn_id: String,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub description: String,
    pub payee: Option<String>,
}

/// Opaque cursor used to resume incremental sync.
pub type Cursor = Option<String>;

#[derive(Debug, Error)]
pub enum BankFeedError {
    #[error("provider HTTP error: {0}")]
    Http(String),
    #[error("token decryption failed")]
    TokenDecryption,
    #[error("provider credential not configured: {0}")]
    MissingCredential(String),
    #[error("provider returned invalid data: {0}")]
    InvalidData(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

// ─── Provider trait ────────────────────────────────────────────────────────

/// Abstraction over all bank-data providers.
///
/// Each impl lives in its own submodule. The `manual` stub is always
/// available and requires no credentials.
///
/// ## EU open-banking aggregator shape (`statement-reconciliation`)
///
/// EU aggregators (GoCardless Bank Account Data, Enable Banking) are
/// normalized to `(date, amount, payee, reference)` at the provider
/// boundary with the provider tag preserved:
///
/// | Provider       | date field                | amount field                  | payee field                        | reference field                  | txn id                          |
/// |----------------|---------------------------|-------------------------------|----------------------------------|----------------------------------|-----------------------------------|
/// | GoCardless     | `bookingDate` (YYYY-MM-DD)| `transactionAmount.amount`    | — (remittance only)              | `remittanceInformationUnstructured` | `transactionId`                |
/// | Enable Banking | `booking_date`/`value_date`| `amount` (signed string)      | `creditor_name`/`debtor_name`    | `remittance`                     | `enable_banking:{entry_id}`       |
///
/// Live OAuth credentials cannot be tested in CI, so EU adapters expose
/// a documented mapping (`enable_banking::map_transaction`) plus a
/// test-shaped fetch; see `enable_banking.rs`.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Fetch new transactions since `cursor`.
    ///
    /// Returns `(transactions, new_cursor)`. The cursor is provider-specific
    /// (pagination token, timestamp, etc.). Pass `None` for the first call.
    async fn fetch_transactions(
        &self,
        access_token: &str,
        cursor: Cursor,
    ) -> Result<(Vec<RemoteTransaction>, Cursor), BankFeedError>;

    /// Exchange a one-time public token (Plaid link flow) for a persistent
    /// access token. Providers that do not use this pattern return the
    /// `public_token` unchanged.
    async fn exchange_token(&self, public_token: &str) -> Result<String, BankFeedError>;
}

/// Resolve a provider instance by name string.
///
/// Returns `None` for unknown names. The caller decides how to handle that
/// (config validation runs at startup so unknown names never reach here
/// in production).
pub fn resolve(name: &str) -> Option<Box<dyn Provider>> {
    match name {
        "plaid" => Some(Box::new(plaid::PlaidProvider)),
        "gocardless" => Some(Box::new(gocardless::GoCardlessProvider)),
        "enable_banking" => Some(Box::new(enable_banking::EnableBankingProvider)),
        "salt_edge" => Some(Box::new(salt_edge::SaltEdgeProvider)),
        "simplefin" => Some(Box::new(simplefin::SimplefinProvider)),
        _ => Some(Box::new(manual::ManualProvider)),
    }
}
