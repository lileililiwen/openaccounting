use crate::templates::render_response;
use axum::extract::{Path, RawForm, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_login::AuthSession;
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::Backend,
    audit,
    domain::{Account, Direction, Transaction, TxnLineInput},
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::transactions::{
        TransactionFilter, TransactionForm, TransactionFormLine, TransactionList, TransactionNew,
        TransactionRow, TransactionShow, TransactionShowLine,
    },
    AppState,
};

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let filter = TransactionFilter {
        from: q.get("from").map(|s| s.to_string()).unwrap_or_default(),
        to: q.get("to").map(|s| s.to_string()).unwrap_or_default(),
        account_id: q
            .get("account_id")
            .map(|s| s.to_string())
            .unwrap_or_default(),
        q: q.get("q").map(|s| s.to_string()).unwrap_or_default(),
    };

    let limit: i64 = q.get("limit").and_then(|s| s.parse().ok()).unwrap_or(100);

    let from_date = NaiveDate::parse_from_str(&filter.from, "%Y-%m-%d").ok();
    let to_date = NaiveDate::parse_from_str(&filter.to, "%Y-%m-%d").ok();
    let _account_uuid = Uuid::parse_str(&filter.account_id).ok();
    let q_pattern = if filter.q.is_empty() {
        None
    } else {
        Some(filter.q.as_str())
    };

    let rows = sqlx::query_as::<_, TransactionRow>(
        r#"
        SELECT t.id, t.txn_date AS date, t.description, COALESCE(t.payee, '') AS payee, t.currency, t.kind,
               COALESCE((SELECT SUM(p.amount) FROM postings p WHERE p.transaction_id=t.id AND p.direction='DEBIT'), 0) AS total,
               (SELECT COUNT(*) FROM documents d WHERE d.transaction_id = t.id) AS doc_count,
               COALESCE((SELECT array_agg(tg.name)
                         FROM transaction_tags tt
                         JOIN tags tg ON tg.id = tt.tag_id
                         WHERE tt.transaction_id = t.id), ARRAY[]::text[]) AS tags
        FROM transactions t
        WHERE t.ledger_id = $1
          AND ($2::date IS NULL OR t.txn_date >= $2)
          AND ($3::date IS NULL OR t.txn_date <= $3)
          AND ($4::text IS NULL OR t.description ILIKE '%' || $4 || '%' OR COALESCE(t.payee,'') ILIKE '%' || $4 || '%')
        ORDER BY t.txn_date DESC, t.created_at DESC
        LIMIT $5
        "#,
    )
    .bind(ledger_id)
    .bind(from_date)
    .bind(to_date)
    .bind(q_pattern)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(TransactionList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        transactions: rows,
        filter,
    }))
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let accounts = sqlx::query_as::<_, Account>(
        r#"SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
           FROM accounts WHERE ledger_id = $1 AND is_archived = FALSE
           ORDER BY type, code NULLS LAST, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(render_response(TransactionNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        currency: ledger.base_currency.clone(),
        accounts,
        error: String::new(),
        form: TransactionForm::empty(),
    }))
}

#[derive(Deserialize)]
pub struct NewTxnForm {
    pub date: String,
    pub description: String,
    pub payee: Option<String>,
    pub reference: Option<String>,
    /// Form fields for postings arrive as
    /// `lines[N][account_id]`, `lines[N][direction]`, `lines[N][amount]`,
    /// `lines[N][memo]`. Axum's Form extractor uses serde_urlencoded, which
    /// does not flatten bracketed keys into a nested struct. We catch-all
    /// into a `HashMap<String, String>` and parse manually below.
    #[serde(flatten, default)]
    pub extra: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug, Default)]
struct ParsedLine {
    account_id: String,
    direction: String,
    amount: String,
    memo: String,
}

fn parse_lines(raw: &std::collections::HashMap<String, String>) -> Vec<ParsedLine> {
    // Group fields by the integer in `lines[N][field]`.
    let mut by_index: std::collections::BTreeMap<usize, ParsedLine> =
        std::collections::BTreeMap::new();
    for (key, value) in raw {
        // key is e.g. "lines[0][account_id]"
        if !key.starts_with("lines[") {
            continue;
        }
        let after = &key[6..]; // "0][account_id]"
        let Some(close) = after.find("][") else {
            continue;
        };
        let idx_str = &after[..close];
        let Ok(idx) = idx_str.parse::<usize>() else {
            continue;
        };
        let field = &after[close + 2..];
        let field = field.trim_end_matches(']');
        let entry = by_index.entry(idx).or_default();
        match field {
            "account_id" => entry.account_id = value.clone(),
            "direction" => entry.direction = value.clone(),
            "amount" => entry.amount = value.clone(),
            "memo" => entry.memo = value.clone(),
            _ => {}
        }
    }
    by_index.into_values().collect()
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    RawForm(body): RawForm,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let accounts = sqlx::query_as::<_, Account>(
        r#"SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
           FROM accounts WHERE ledger_id = $1 AND is_archived = FALSE ORDER BY type, code, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    // Parse the raw form body ourselves: `lines[N][field]=value` is not
    // representable as a nested serde struct, so we read the bytes, split on
    // '&', and group by the integer in the key.
    let raw: std::collections::HashMap<String, String> = form_urlencoded::parse(&body)
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let form = NewTxnForm {
        date: raw.get("date").cloned().unwrap_or_default(),
        description: raw.get("description").cloned().unwrap_or_default(),
        payee: raw.get("payee").cloned(),
        reference: raw.get("reference").cloned(),
        extra: raw,
    };

    let make_error = |msg: String| TransactionNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name.clone(),
        currency: ledger.base_currency.clone(),
        accounts: accounts.clone(),
        error: msg,
        form: TransactionForm {
            date: form.date.clone(),
            description: form.description.clone(),
            payee: form.payee.clone().unwrap_or_default(),
            reference: form.reference.clone().unwrap_or_default(),
            lines: parse_lines(&form.extra)
                .into_iter()
                .map(|l| TransactionFormLine {
                    account_id: l.account_id,
                    amount: l.amount,
                    direction: l.direction,
                    memo: l.memo,
                })
                .collect(),
        },
    };

    if form.extra.is_empty() {
        return Ok(render_response(make_error(
            "A transaction needs at least two postings.".into(),
        )));
    }
    let parsed = parse_lines(&form.extra);
    if parsed.len() < 2 {
        return Ok(render_response(make_error(
            "A transaction needs at least two postings.".into(),
        )));
    }
    let date = match NaiveDate::parse_from_str(&form.date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            return Ok(render_response(make_error(
                "Invalid date format (YYYY-MM-DD)".into(),
            )))
        }
    };
    let description = form.description.trim();
    if description.is_empty() {
        return Ok(render_response(make_error(
            "Description is required".into(),
        )));
    }

    let mut inputs: Vec<TxnLineInput> = Vec::new();
    for l in &parsed {
        let account_id = match Uuid::parse_str(&l.account_id) {
            Ok(id) => id,
            Err(_) => {
                return Ok(render_response(make_error(
                    "Invalid account on a posting".into(),
                )))
            }
        };
        let amount: Decimal = match l.amount.trim().parse() {
            Ok(a) if a > Decimal::ZERO => a,
            _ => {
                return Ok(render_response(make_error(
                    "Each posting must have a positive amount".into(),
                )))
            }
        };
        let direction = match l.direction.as_str() {
            "DEBIT" => Direction::Debit,
            "CREDIT" => Direction::Credit,
            _ => {
                return Ok(render_response(make_error(
                    "Each posting must be DEBIT or CREDIT".into(),
                )))
            }
        };
        inputs.push(TxnLineInput {
            account_id,
            signed_amount: match direction {
                Direction::Debit => amount,
                Direction::Credit => -amount,
            },
            memo: if l.memo.is_empty() {
                None
            } else {
                Some(l.memo.clone())
            },
        });
    }

    // Validate balance in code before insert (DB trigger will also enforce).
    let total: Decimal = inputs.iter().map(|i| i.signed_amount).sum();
    if total != Decimal::ZERO {
        return Ok(render_response(make_error(format!(
            "Postings do not balance: net is {} (debits must equal credits).",
            total
        ))));
    }

    let mut tx = state.pool.begin().await?;

    // Check if the period is closed
    let is_closed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM closed_periods WHERE ledger_id = $1 AND period_year = $2)",
    )
    .bind(ledger_id)
    .bind(date.year())
    .fetch_one(&mut *tx)
    .await?;
    if is_closed {
        return Err(AppError::Validation(format!(
            "Period {} is closed. Cannot post transactions to closed periods.",
            date.year()
        )));
    }

    let txn = sqlx::query_as::<_, Transaction>(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, payee, reference, currency, kind, created_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id, ledger_id, txn_date, description, payee, reference, currency, kind, contact_id, invoice_id, created_by, created_at, updated_at"#,
    )
    .bind(ledger_id)
    .bind(date)
    .bind(description)
    .bind(form.payee.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(form.reference.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(&ledger.base_currency)
    .bind("standard")
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;

    for l in &inputs {
        let dir = if l.signed_amount > Decimal::ZERO {
            Direction::Debit
        } else {
            Direction::Credit
        };
        let amt = l.signed_amount.abs();
        sqlx::query(
            r#"INSERT INTO postings (transaction_id, account_id, amount, direction, memo)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(txn.id)
        .bind(l.account_id)
        .bind(amt)
        .bind(dir.as_str())
        .bind(l.memo.as_deref())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    // Audit log
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "transaction",
        Some(txn.id),
        None,
        Some(serde_json::json!({
            "description": description,
            "date": date,
            "kind": "standard"
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/transactions/{}", ledger_id, txn.id)).into_response())
}

pub async fn show(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, txn_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let txn = sqlx::query_as::<_, Transaction>(
        r#"SELECT id, ledger_id, txn_date, description, payee, reference, currency, kind, contact_id, invoice_id, created_by, created_at, updated_at
           FROM transactions WHERE id = $1 AND ledger_id = $2"#,
    )
    .bind(txn_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let lines = sqlx::query_as::<_, (String, String, String, Decimal, Option<String>)>(
        r#"SELECT a.name, a.type, p.direction, p.amount, p.memo
           FROM postings p JOIN accounts a ON a.id = p.account_id
           WHERE p.transaction_id = $1
           ORDER BY p.direction, a.name"#,
    )
    .bind(txn_id)
    .fetch_all(&state.pool)
    .await?;
    let lines: Vec<TransactionShowLine> = lines
        .into_iter()
        .map(|(n, t, d, a, m)| TransactionShowLine {
            account_name: n,
            account_type: t,
            direction: d,
            amount: a,
            memo: m.unwrap_or_default(),
        })
        .collect();

    let documents = sqlx::query_as::<_, crate::domain::Document>(
        r#"SELECT id, transaction_id, filename, stored_filename, mime_type, size_bytes, uploaded_by, uploaded_at
           FROM documents WHERE transaction_id = $1 ORDER BY uploaded_at"#,
    )
    .bind(txn_id)
    .fetch_all(&state.pool)
    .await?;

    let tags: Vec<String> = sqlx::query_scalar(
        r#"SELECT tg.name FROM transaction_tags tt
           JOIN tags tg ON tg.id = tt.tag_id WHERE tt.transaction_id = $1 ORDER BY tg.name"#,
    )
    .bind(txn_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(TransactionShow {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        txn_id: txn.id,
        date: txn.txn_date,
        description: txn.description,
        payee: txn.payee.unwrap_or_default(),
        reference: txn.reference.unwrap_or_default(),
        currency: txn.currency,
        lines,
        documents,
        tags,
        flash: String::new(),
    }))
}
