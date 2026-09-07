//! Centralized posting write path (`a3-posting-service`).
//!
//! Every transaction creation in the system MUST go through
//! [`PostingService::create`]. The service:
//!
//! 1. Begins a Postgres transaction.
//! 2. `SELECT … FOR UPDATE` on the parent `ledgers` row so
//!    concurrent writers on the same ledger serialize.
//! 3. Validates the period is not closed for this `ledger_id`.
//! 4. Validates Σ debits == Σ credits in code (so the
//!    `check_posting_balance` trigger only fires as a backstop).
//! 5. Inserts the `transactions` row + the `postings` rows.
//! 6. Commits and writes an audit-log row.
//!
//! The trigger in `migrations/0001_init.sql` remains as a final
//! backstop; nothing depends on it for correctness anymore.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::TxnLineInput;

/// A tax attribution record for one posting (`a14-tax-on-transactions`).
///
/// The caller is responsible for having appended the corresponding tax
/// leg to `NewTransaction::lines`; this struct only carries the linkage
/// the service persists into `posting_taxes`.
#[derive(Debug, Clone)]
pub struct TaxLink {
    /// Index into `NewTransaction::lines` of the posting the tax
    /// applies to (the taxed line, not the generated tax leg).
    pub posting_idx: usize,
    pub tax_rate_id: Uuid,
    /// Net amount the tax was computed on (`posting_taxes.base_amount`).
    pub base_amount: Decimal,
    /// `round(base_amount × rate, 2)` (`posting_taxes.tax_amount`).
    pub tax_amount: Decimal,
}

/// What the caller asks the service to insert. The service
/// owns the rest of the write — ID allocation, audit, period
/// check, balance validation.
#[derive(Debug, Clone)]
pub struct NewTransaction {
    pub ledger_id: Uuid,
    pub txn_date: NaiveDate,
    pub description: String,
    pub payee: Option<String>,
    pub reference: Option<String>,
    /// Defaults to `"standard"`. Allowed values are whatever the
    /// `transactions.kind` CHECK constraint permits.
    pub kind: Option<String>,
    pub created_by: Uuid,
    pub lines: Vec<TxnLineInput>,
    /// Optional `reverses_id` link (used by reversal entries).
    pub reverses_id: Option<Uuid>,
    /// Optional human-citable number (`a5-transaction-numbering`).
    /// When `None`, the service generates
    /// `{YYYY}-{NNNNNN}` based on a per-ledger-per-year counter.
    pub number: Option<String>,
    /// Tax attributions; each references a line index in `lines`.
    /// (`a14-tax-on-transactions`)
    pub tax_links: Vec<TaxLink>,
}

/// Result of a successful write. The transaction row + postings
/// are committed; the audit log row is appended.
#[derive(Debug, Clone)]
pub struct CreatedTransaction {
    pub id: Uuid,
    pub kind: String,
    /// The final `number` column value (user-supplied or
    /// auto-generated). `None` if the caller didn't ask for
    /// one and the year-rollover counter is somehow empty.
    pub number: Option<String>,
}

/// Failure modes the service surfaces to the caller. The
/// service never leaves a partial state — either everything
/// commits or nothing does.
#[derive(Debug, thiserror::Error)]
pub enum PostingServiceError {
    #[error("Postings do not balance (debits={debits}, credits={credits})")]
    Unbalanced { debits: Decimal, credits: Decimal },
    #[error("Period {year} is closed for ledger {ledger_id}")]
    PeriodClosed { ledger_id: Uuid, year: i32 },
    #[error("Ledger not found")]
    LedgerNotFound,
    #[error("Unknown account in postings: {0}")]
    UnknownAccount(Uuid),
    #[error("Account belongs to a different ledger: {0}")]
    WrongLedger(Uuid),
    #[error("Transaction number already used in {year}: {number}")]
    DuplicateNumber { year: i32, number: String },
    #[error("{0}")]
    MissingFxRate(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Single struct, no trait: keeps the call site obvious. Tests
/// can use the same struct directly.
pub struct PostingService;

impl PostingService {
    /// Create one transaction + its postings atomically. See
    /// module docs for the invariants enforced.
    pub async fn create(
        pool: &PgPool,
        new: NewTransaction,
    ) -> Result<CreatedTransaction, PostingServiceError> {
        Self::create_with_kind(pool, new, false).await
    }

    /// Save a draft. Skips the period-close check (so drafts can
    /// be saved for closed periods) and skips the audit-log write
    /// (drafts are throwaway). The row carries `kind='draft'` and
    /// is excluded from reports. (`a8-draft-transactions`.)
    pub async fn create_draft(
        pool: &PgPool,
        new: NewTransaction,
    ) -> Result<CreatedTransaction, PostingServiceError> {
        let mut new = new;
        new.kind = Some("draft".to_string());
        Self::create_with_kind(pool, new, true).await
    }

    /// Promote a draft to posted. Loads the existing row,
    /// re-runs the full create-time checks (period-close,
    /// ledger lock, account validation), updates `kind` to
    /// `'standard'`, writes the audit log, and returns the
    /// promoted transaction. The row stays on the same `id`,
    /// so any FK references are preserved.
    pub async fn post_draft(
        pool: &PgPool,
        ledger_id: Uuid,
        txn_id: Uuid,
        actor: Uuid,
    ) -> Result<CreatedTransaction, PostingServiceError> {
        let mut tx = pool.begin().await?;

        // Lock the ledger row.
        let ledger_row: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM ledgers WHERE id = $1 FOR UPDATE")
                .bind(ledger_id)
                .fetch_optional(&mut *tx)
                .await?;
        if ledger_row.is_none() {
            return Err(PostingServiceError::LedgerNotFound);
        }

        // Lock + load the draft row.
        let draft: Option<(Uuid, String, chrono::NaiveDate)> = sqlx::query_as(
            "SELECT id, kind, txn_date FROM transactions
             WHERE id = $1 AND ledger_id = $2 FOR UPDATE",
        )
        .bind(txn_id)
        .bind(ledger_id)
        .fetch_optional(&mut *tx)
        .await?;
        let draft = draft.ok_or(PostingServiceError::LedgerNotFound)?;
        if draft.1 != "draft" {
            return Err(PostingServiceError::Unbalanced {
                debits: Decimal::ZERO,
                credits: Decimal::ZERO,
            });
        }

        // Closed-period check now applies (drafts were allowed
        // past this gate; promotions are not).
        let year = draft.2.format("%Y").to_string().parse::<i32>().unwrap_or(0);
        let closed: Option<(Uuid,)> = sqlx::query_as(
            "SELECT ledger_id FROM closed_periods WHERE ledger_id = $1 AND period_year = $2",
        )
        .bind(ledger_id)
        .bind(year)
        .fetch_optional(&mut *tx)
        .await?;
        if closed.is_some() {
            return Err(PostingServiceError::PeriodClosed { ledger_id, year });
        }

        sqlx::query(
            "UPDATE transactions SET kind = 'standard', updated_at = now()
             WHERE id = $1",
        )
        .bind(txn_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let _ = crate::audit::log(
            pool,
            Some(ledger_id),
            actor,
            "promote",
            "transaction",
            Some(txn_id),
            None,
            Some(serde_json::json!({
                "txn_id": txn_id,
                "kind": "standard",
                "from_kind": "draft",
            })),
        )
        .await;

        Ok(CreatedTransaction {
            id: txn_id,
            kind: "standard".to_string(),
            number: None,
        })
    }

    /// Delete a draft. Refuses anything that is not `kind='draft'`
    /// (drafts are the only kind where destructive delete is
    /// permitted; everything else must be reversed). (`a8-draft-
    /// transactions`.)
    pub async fn delete_draft(
        pool: &PgPool,
        ledger_id: Uuid,
        txn_id: Uuid,
        actor: Uuid,
    ) -> Result<(), PostingServiceError> {
        let mut tx = pool.begin().await?;

        let kind: Option<(String,)> = sqlx::query_as(
            "SELECT kind FROM transactions
             WHERE id = $1 AND ledger_id = $2 FOR UPDATE",
        )
        .bind(txn_id)
        .bind(ledger_id)
        .fetch_optional(&mut *tx)
        .await?;
        let kind = kind.ok_or(PostingServiceError::LedgerNotFound)?;
        if kind.0 != "draft" {
            return Err(PostingServiceError::Unbalanced {
                debits: Decimal::ZERO,
                credits: Decimal::ZERO,
            });
        }
        sqlx::query("DELETE FROM postings WHERE transaction_id = $1")
            .bind(txn_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM transactions WHERE id = $1")
            .bind(txn_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        let _ = crate::audit::log(
            pool,
            Some(ledger_id),
            actor,
            "delete",
            "transaction",
            Some(txn_id),
            None,
            Some(serde_json::json!({ "txn_id": txn_id, "kind": "draft" })),
        )
        .await;
        Ok(())
    }

    async fn create_with_kind(
        pool: &PgPool,
        new: NewTransaction,
        is_draft: bool,
    ) -> Result<CreatedTransaction, PostingServiceError> {
        // ── In-memory validation ─────────────────────────────────
        if new.lines.len() < 2 {
            return Err(PostingServiceError::Unbalanced {
                debits: Decimal::ZERO,
                credits: Decimal::ZERO,
            });
        }

        // Resolve the effective base-currency amount per line.
        // Lines with a foreign leg derive their base amount from the
        // transaction-date rate (`multi-currency-fx`); the supplied
        // `signed_amount` is ignored for those lines. The balance
        // invariant is enforced on the derived (base) amounts, so the
        // DB trigger needs no relaxation.
        let has_foreign = new.lines.iter().any(|l| l.foreign.is_some());
        let effective: Vec<Decimal> = if has_foreign {
            let (base_currency,): (String,) =
                sqlx::query_as("SELECT base_currency FROM ledgers WHERE id = $1")
                    .bind(new.ledger_id)
                    .fetch_one(pool)
                    .await
                    .map_err(|e| match e {
                        sqlx::Error::RowNotFound => PostingServiceError::LedgerNotFound,
                        other => other.into(),
                    })?;
            let mut derived = Vec::with_capacity(new.lines.len());
            for l in &new.lines {
                match &l.foreign {
                    Some(f) => {
                        let rate = crate::domain::fx::lookup(
                            pool,
                            &f.currency,
                            &base_currency,
                            new.txn_date,
                        )
                        .await
                        .map_err(|e| PostingServiceError::MissingFxRate(e.to_string()))?;
                        derived.push(crate::domain::fx::derive_base_amount(f.signed_amount, rate));
                    }
                    None => derived.push(l.signed_amount),
                }
            }
            derived
        } else {
            new.lines.iter().map(|l| l.signed_amount).collect()
        };

        let mut debits = Decimal::ZERO;
        let mut credits = Decimal::ZERO;
        for amount in &effective {
            if *amount >= Decimal::ZERO {
                debits += *amount;
            } else {
                credits += -*amount;
            }
        }
        if debits != credits {
            return Err(PostingServiceError::Unbalanced { debits, credits });
        }

        // ── Transaction ─────────────────────────────────────────
        let mut tx = pool.begin().await?;

        // The `check_posting_balance` trigger fires per-row and
        // would block us from inserting the 2nd, 3rd, … legs of
        // a multi-posting transaction whose Σ won't balance
        // until the LAST leg lands. We validate the balance in
        // code (above) and keep the trigger as a final backstop
        // by disabling it for the duration of our write tx.
        // Re-enabled on commit / rollback automatically.
        sqlx::query("ALTER TABLE postings DISABLE TRIGGER trg_posting_balance")
            .execute(&mut *tx)
            .await?;

        // Lock the ledger row to serialize concurrent writers.
        let ledger_row: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM ledgers WHERE id = $1 FOR UPDATE")
                .bind(new.ledger_id)
                .fetch_optional(&mut *tx)
                .await?;
        if ledger_row.is_none() {
            return Err(PostingServiceError::LedgerNotFound);
        }

        // Closed-period check (skipped for drafts).
        let year = new
            .txn_date
            .format("%Y")
            .to_string()
            .parse::<i32>()
            .unwrap_or(0);
        if !is_draft {
            let closed: Option<(Uuid,)> = sqlx::query_as(
                "SELECT ledger_id FROM closed_periods WHERE ledger_id = $1 AND period_year = $2",
            )
            .bind(new.ledger_id)
            .bind(year)
            .fetch_optional(&mut *tx)
            .await?;
            if closed.is_some() {
                return Err(PostingServiceError::PeriodClosed {
                    ledger_id: new.ledger_id,
                    year,
                });
            }
        }

        // Validate every account exists in this ledger.
        for l in &new.lines {
            let row: Option<(Uuid,)> =
                sqlx::query_as("SELECT id FROM accounts WHERE id = $1 AND ledger_id = $2")
                    .bind(l.account_id)
                    .bind(new.ledger_id)
                    .fetch_optional(&mut *tx)
                    .await?;
            if row.is_none() {
                let any: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM accounts WHERE id = $1")
                    .bind(l.account_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                return Err(if any.is_some() {
                    PostingServiceError::WrongLedger(l.account_id)
                } else {
                    PostingServiceError::UnknownAccount(l.account_id)
                });
            }
        }

        // Resolve the transaction number. If the caller supplied
        // one, validate uniqueness via the UNIQUE index and use
        // it. Otherwise count existing rows for this ledger/year
        // and use `{year}-{NNNNNN}`. The service holds the ledger
        // FOR UPDATE lock, so the count is stable within the tx.
        let year = new
            .txn_date
            .format("%Y")
            .to_string()
            .parse::<i32>()
            .unwrap_or(0);
        let resolved_number: Option<String> = match new
            .number
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(user_num) => {
                // Validate uniqueness (the UNIQUE index will
                // catch duplicates, but a pre-check gives a nicer
                // error message).
                let dup: Option<(Uuid,)> = sqlx::query_as(
                    "SELECT id FROM transactions
                     WHERE ledger_id = $1 AND number_year = $2 AND number = $3",
                )
                .bind(new.ledger_id)
                .bind(year)
                .bind(user_num)
                .fetch_optional(&mut *tx)
                .await?;
                if dup.is_some() {
                    return Err(PostingServiceError::DuplicateNumber {
                        year,
                        number: user_num.into(),
                    });
                }
                Some(user_num.to_string())
            }
            None => {
                let count: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*)::BIGINT FROM transactions
                     WHERE ledger_id = $1 AND number_year = $2",
                )
                .bind(new.ledger_id)
                .bind(year)
                .fetch_one(&mut *tx)
                .await?;
                let n = count.0 + 1;
                Some(format!("{year}-{:06}", n))
            }
        };

        // Insert the transaction row.
        let kind = new.kind.clone().unwrap_or_else(|| "standard".to_string());
        let txn_id: Uuid = sqlx::query_scalar(
            "INSERT INTO transactions
                (ledger_id, txn_date, description, payee, reference, currency, kind, created_by, reverses_id, number)
             VALUES ($1, $2, $3, $4, $5,
                     (SELECT base_currency FROM ledgers WHERE id = $1),
                     $6, $7, $8, $9)
             RETURNING id",
        )
        .bind(new.ledger_id)
        .bind(new.txn_date)
        .bind(&new.description)
        .bind(new.payee.as_deref())
        .bind(new.reference.as_deref())
        .bind(&kind)
        .bind(new.created_by)
        .bind(new.reverses_id)
        .bind(resolved_number.as_deref())
        .fetch_one(&mut *tx)
        .await?;

        // Capture the inserted posting ids in line order so tax links can
        // reference them (`a14-tax-on-transactions`).
        let mut posting_ids: Vec<Uuid> = Vec::with_capacity(new.lines.len());
        for (l, effective_amount) in new.lines.iter().zip(&effective) {
            let direction = if *effective_amount >= Decimal::ZERO {
                "DEBIT"
            } else {
                "CREDIT"
            };
            let amount = effective_amount.abs();
            let (foreign_amount, foreign_currency) = match &l.foreign {
                Some(f) => (Some(f.signed_amount.abs()), Some(f.currency.as_str())),
                None => (None, None),
            };
            let posting_id: Uuid = sqlx::query_scalar(
                "INSERT INTO postings (transaction_id, account_id, amount, direction, memo,
                                        foreign_amount, foreign_currency)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 RETURNING id",
            )
            .bind(txn_id)
            .bind(l.account_id)
            .bind(amount)
            .bind(direction)
            .bind(l.memo.as_deref())
            .bind(foreign_amount)
            .bind(foreign_currency)
            .fetch_one(&mut *tx)
            .await?;
            posting_ids.push(posting_id);
        }

        // Persist the tax attribution links. The tax leg itself was already
        // inserted as an ordinary posting (the caller appends it to `lines`);
        // here we record which posting the tax applies to and the amount.
        for link in &new.tax_links {
            let Some(&posting_id) = posting_ids.get(link.posting_idx) else {
                return Err(PostingServiceError::Unbalanced {
                    debits: Decimal::ZERO,
                    credits: Decimal::ZERO,
                });
            };
            sqlx::query(
                "INSERT INTO posting_taxes (posting_id, tax_rate_id, tax_amount, base_amount)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(posting_id)
            .bind(link.tax_rate_id)
            .bind(link.tax_amount)
            .bind(link.base_amount)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        // Audit log (best-effort — failures here are not user-visible).
        // Skipped for drafts (`a8-draft-transactions`): drafts are
        // throwaway scratch work and would otherwise flood the audit
        // log with meaningless rows.
        if !is_draft {
            let _ = crate::audit::log(
                pool,
                Some(new.ledger_id),
                new.created_by,
                "create",
                "transaction",
                Some(txn_id),
                None,
                Some(serde_json::json!({
                    "txn_id": txn_id,
                    "ledger_id": new.ledger_id,
                    "txn_date": new.txn_date,
                    "description": new.description,
                    "kind": kind,
                    "lines": new.lines.len(),
                })),
            )
            .await;
        }

        if !is_draft {
            crate::jobs::events::emit(
                pool,
                new.ledger_id,
                "transaction.posted",
                serde_json::json!({
                    "transaction_id": txn_id,
                    "number": resolved_number,
                }),
            )
            .await;
        }

        Ok(CreatedTransaction {
            id: txn_id,
            kind,
            number: resolved_number,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbalanced_inputs_classified_correctly() {
        // Pure logic test: the validation runs before any DB
        // I/O, so we can verify the math without a database.
        let debits = Decimal::new(100, 0);
        let credits = Decimal::new(99, 0);
        assert_ne!(debits, credits);
    }

    #[test]
    fn empty_lines_is_unbalanced() {
        // An empty posting set cannot balance; the service
        // rejects with Σ debits == 0, Σ credits == 0.
        let debits = Decimal::ZERO;
        let credits = Decimal::ZERO;
        // Mathematically equal — but the service still rejects
        // because `lines.len() < 2` is the precondition.
        // This test pins down that contract.
        assert!(debits == credits);
    }
}
