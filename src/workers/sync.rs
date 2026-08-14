//! Background sync worker for bank feeds.
//!
//! Spawned once at startup. Loops forever, sleeping between runs.
//! Each run fetches all active bank feed links and syncs transactions.

use std::time::Duration;

use tracing::{info, warn};
use uuid::Uuid;

use crate::bank_feeds::{crypto::TokenCipher, resolve, RemoteTransaction};
use crate::AppState;

/// Row from `bank_feed_links` — only what the sync loop needs.
#[derive(sqlx::FromRow)]
pub struct LinkRow {
    pub id: Uuid,
    pub ledger_id: Uuid,
    pub provider: String,
    pub access_token_encrypted: Option<String>,
    pub cursor: Option<String>,
    pub account_id_in_ledger: Option<Uuid>,
}

/// Entry point: run the sync loop forever.
///
/// `interval_hours` controls how often we run (default: 6 hours).
/// The loop starts with an initial sleep so the server can finish booting.
pub async fn run_sync_loop(state: AppState, interval_hours: u64) {
    let interval = Duration::from_secs(interval_hours * 3600);
    info!(
        "bank feed sync worker started (interval = {}h)",
        interval_hours
    );
    loop {
        tokio::time::sleep(interval).await;
        if let Err(e) = run_once(&state).await {
            warn!("bank feed sync run failed: {e}");
        }
    }
}

/// Run a single sync pass over all active links.
pub async fn run_once(state: &AppState) -> Result<(), sqlx::Error> {
    let links: Vec<LinkRow> = sqlx::query_as(
        r#"SELECT id, ledger_id, provider, access_token_encrypted, cursor,
                  account_id_in_ledger
           FROM bank_feed_links WHERE status = 'active'"#,
    )
    .fetch_all(&state.pool)
    .await?;

    for link in links {
        if let Err(e) = sync_one(state, &link).await {
            warn!("sync failed for link {}: {}", link.id, e);
            let _ = sqlx::query(
                "UPDATE bank_feed_links SET status = 'error', error_message = $1, updated_at = now() WHERE id = $2",
            )
            .bind(e.to_string())
            .bind(link.id)
            .execute(&state.pool)
            .await;
        }
    }
    Ok(())
}

/// Sync a single link by its ID (used by the manual sync trigger and webhook handler).
pub async fn run_for_link(
    state: &AppState,
    link_id: Uuid,
) -> Result<(), crate::bank_feeds::BankFeedError> {
    let link: Option<LinkRow> = sqlx::query_as(
        r#"SELECT id, ledger_id, provider, access_token_encrypted, cursor,
                  account_id_in_ledger
           FROM bank_feed_links WHERE id = $1"#,
    )
    .bind(link_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(crate::bank_feeds::BankFeedError::Db)?;

    match link {
        Some(l) => sync_one(state, &l).await,
        None => Ok(()),
    }
}

/// Public alias used by the handler layer.
pub async fn sync_link(
    state: &AppState,
    link: &LinkRow,
) -> Result<(), crate::bank_feeds::BankFeedError> {
    sync_one(state, link).await
}

async fn sync_one(
    state: &AppState,
    link: &LinkRow,
) -> Result<(), crate::bank_feeds::BankFeedError> {
    // Decrypt access token.
    let cipher = get_cipher()?;
    let access_token = match &link.access_token_encrypted {
        Some(enc) => cipher
            .open(enc)
            .ok_or(crate::bank_feeds::BankFeedError::TokenDecryption)?,
        None => String::new(),
    };

    let provider = resolve(&link.provider)
        .unwrap_or_else(|| Box::new(crate::bank_feeds::manual::ManualProvider));

    let (txns, new_cursor) = provider
        .fetch_transactions(&access_token, link.cursor.clone())
        .await?;

    for t in &txns {
        import_transaction(state, link, t).await?;
    }

    // Update cursor and last_synced_at.
    sqlx::query(
        "UPDATE bank_feed_links SET cursor = $1, last_synced_at = now(), error_message = NULL, updated_at = now() WHERE id = $2",
    )
    .bind(&new_cursor)
    .bind(link.id)
    .execute(&state.pool)
    .await
    .map_err(crate::bank_feeds::BankFeedError::Db)?;

    info!("synced {} transactions for link {}", txns.len(), link.id);
    Ok(())
}

async fn import_transaction(
    state: &AppState,
    link: &LinkRow,
    t: &RemoteTransaction,
) -> Result<(), crate::bank_feeds::BankFeedError> {
    // Dedup: skip if provider_txn_id already imported for this link.
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM bank_feed_transactions WHERE link_id = $1 AND provider_txn_id = $2)"
    )
    .bind(link.id)
    .bind(&t.provider_txn_id)
    .fetch_one(&state.pool)
    .await
    .map_err(crate::bank_feeds::BankFeedError::Db)?;

    if exists {
        return Ok(());
    }

    // Look up a system user for the transaction created_by field.
    let created_by: Uuid = sqlx::query_scalar("SELECT id FROM users LIMIT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(crate::bank_feeds::BankFeedError::Db)?;

    // Look up the ledger's base currency.
    let currency: String = sqlx::query_scalar("SELECT base_currency FROM ledgers WHERE id = $1")
        .bind(link.ledger_id)
        .fetch_one(&state.pool)
        .await
        .map_err(crate::bank_feeds::BankFeedError::Db)?;

    // Create the transaction + two postings inside a DB transaction.
    let mut db_tx = state
        .pool
        .begin()
        .await
        .map_err(crate::bank_feeds::BankFeedError::Db)?;

    let txn_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, payee, currency, created_by)
           VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"#,
    )
    .bind(link.ledger_id)
    .bind(t.date)
    .bind(&t.description)
    .bind(&t.payee)
    .bind(&currency)
    .bind(created_by)
    .fetch_one(&mut *db_tx)
    .await
    .map_err(crate::bank_feeds::BankFeedError::Db)?;

    // If a ledger account is configured for this link, post DR that account.
    if let Some(account_id) = link.account_id_in_ledger {
        let direction = if t.amount >= rust_decimal::Decimal::ZERO {
            "DEBIT"
        } else {
            "CREDIT"
        };
        let abs_amount = t.amount.abs();
        sqlx::query(
            "INSERT INTO postings (transaction_id, account_id, direction, amount) VALUES ($1,$2,$3,$4)"
        )
        .bind(txn_id)
        .bind(account_id)
        .bind(direction)
        .bind(abs_amount)
        .execute(&mut *db_tx)
        .await
        .map_err(crate::bank_feeds::BankFeedError::Db)?;
    }

    // Record the provider txn id for dedup.
    sqlx::query(
        "INSERT INTO bank_feed_transactions (link_id, provider_txn_id, transaction_id) VALUES ($1,$2,$3)"
    )
    .bind(link.id)
    .bind(&t.provider_txn_id)
    .bind(txn_id)
    .execute(&mut *db_tx)
    .await
    .map_err(crate::bank_feeds::BankFeedError::Db)?;

    db_tx
        .commit()
        .await
        .map_err(crate::bank_feeds::BankFeedError::Db)?;

    Ok(())
}

/// Load the cipher from env. Returns an error if the key is not set.
fn get_cipher() -> Result<TokenCipher, crate::bank_feeds::BankFeedError> {
    let key_b64 = std::env::var("BANK_FEEDS_ENCRYPTION_KEY").map_err(|_| {
        crate::bank_feeds::BankFeedError::MissingCredential("BANK_FEEDS_ENCRYPTION_KEY".into())
    })?;
    TokenCipher::from_base64(&key_b64)
        .map_err(|e| crate::bank_feeds::BankFeedError::Internal(format!("bad encryption key: {e}")))
}
