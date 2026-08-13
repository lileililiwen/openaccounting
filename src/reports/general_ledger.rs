use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;

use super::AccountTotal;

#[derive(Clone, Debug)]
pub struct GeneralLedgerEntry {
    pub txn_id: Uuid,
    pub txn_date: NaiveDate,
    pub description: String,
    pub payee: String,
    pub account_id: Uuid,
    pub account_name: String,
    pub direction: String,
    pub amount: Decimal,
    pub memo: String,
}

pub async fn build_general_ledger(
    pool: &PgPool,
    ledger_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
    account_filter: Option<Uuid>,
) -> AppResult<(
    Vec<GeneralLedgerEntry>,
    std::collections::HashMap<Uuid, Decimal>,
)> {
    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            NaiveDate,
            String,
            Option<String>,
            Uuid,
            String,
            String,
            Decimal,
            Option<String>,
        ),
    >(
        r#"
        SELECT t.id, t.txn_date, t.description, t.payee,
               a.id, a.name, p.direction, p.amount, p.memo
        FROM postings p
        JOIN transactions t ON t.id = p.transaction_id
        JOIN accounts     a ON a.id = p.account_id
        WHERE t.ledger_id = $1
          AND t.txn_date BETWEEN $2 AND $3
          AND ($4::uuid IS NULL OR p.account_id = $4)
        ORDER BY t.txn_date, t.created_at, p.id
        "#,
    )
    .bind(ledger_id)
    .bind(from)
    .bind(to)
    .bind(account_filter)
    .fetch_all(pool)
    .await?;

    let entries: Vec<GeneralLedgerEntry> = rows
        .into_iter()
        .map(
            |(
                txn_id,
                txn_date,
                description,
                payee,
                account_id,
                account_name,
                direction,
                amount,
                memo,
            )| {
                GeneralLedgerEntry {
                    txn_id,
                    txn_date,
                    description,
                    payee: payee.unwrap_or_default(),
                    account_id,
                    account_name,
                    direction,
                    amount,
                    memo: memo.unwrap_or_default(),
                }
            },
        )
        .collect();

    // Per-account running balance up to and including the period end.
    // For debit-normal accounts (ASSET, EXPENSE): balance = sum(debits) - sum(credits)
    // For credit-normal accounts (LIABILITY, EQUITY, INCOME): balance = sum(credits) - sum(debits)
    let bal_rows = sqlx::query_as::<_, (Uuid, String, Decimal)>(
        r#"
        SELECT a.id, a.type,
               COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE -p.amount END), 0)
             + CASE WHEN a.type IN ('ASSET','EXPENSE') THEN 0 ELSE 0 END
               AS net
        FROM accounts a
        LEFT JOIN postings p ON p.account_id = a.id
        LEFT JOIN transactions t ON t.id = p.transaction_id AND t.txn_date <= $2
        WHERE a.ledger_id = $1
        GROUP BY a.id, a.type
        "#,
    )
    .bind(ledger_id)
    .bind(to)
    .fetch_all(pool)
    .await?;

    let mut balances = std::collections::HashMap::new();
    for (id, ty, signed) in bal_rows {
        // For credit-normal accounts, flip the sign of the SUM result.
        let val = match ty.as_str() {
            "ASSET" | "EXPENSE" => signed,
            _ => -signed,
        };
        balances.insert(id, val);
    }

    Ok((entries, balances))
}

#[allow(dead_code)]
pub fn type_for_total(_t: AccountTotal) {}
