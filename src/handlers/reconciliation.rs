use crate::templates::render_response;
use axum::body::Bytes;
use axum::extract::{Multipart, Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::reconciliation::{ReconHistory, ReconPage, ReconStatementLine, ReconTxn},
    upload, AppState,
};

#[derive(Deserialize)]
pub struct MatchForm {
    pub line_id: Uuid,
    pub transaction_id: Uuid,
}

pub async fn page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let unmatched_lines = sqlx::query_as::<_, ReconStatementLine>(
        r#"SELECT id, statement_date, description, amount, COALESCE(check_number, '') AS check_number
           FROM bank_statement_lines
           WHERE ledger_id = $1 AND account_id = $2 AND status = 'unmatched'
           ORDER BY statement_date DESC, created_at DESC
           LIMIT 100"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .fetch_all(&state.pool)
    .await?;

    let candidate_txns = sqlx::query_as::<_, ReconTxn>(
        r#"SELECT t.id, t.txn_date, t.description, COALESCE(t.payee, '') AS payee, p.amount
           FROM transactions t
           JOIN postings p ON p.transaction_id = t.id
           WHERE t.ledger_id = $1 AND p.account_id = $2 AND t.is_reconciled = FALSE
           ORDER BY t.txn_date DESC, t.created_at DESC
           LIMIT 100"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .fetch_all(&state.pool)
    .await?;

    let statement_balance: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(amount), 0) FROM bank_statement_lines
           WHERE ledger_id = $1 AND account_id = $2 AND status IN ('unmatched', 'matched')"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .fetch_one(&state.pool)
    .await?;

    let ledger_balance: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(p.amount), 0) FROM postings p
           JOIN transactions t ON t.id = p.transaction_id
           WHERE t.ledger_id = $1 AND p.account_id = $2 AND p.direction = 'DEBIT'"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .fetch_one(&state.pool)
    .await?;

    let difference = statement_balance - ledger_balance;

    Ok(render_response(ReconPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        account_id,
        unmatched_lines,
        candidate_txns,
        statement_balance,
        ledger_balance,
        difference,
        flash: String::new(),
    }))
}

pub async fn upload_csv(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id)): Path<(Uuid, Uuid)>,
    mut multipart: Multipart,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let mut csv_data: Option<(String, String, Bytes)> = None; // (declared_mime, filename, body)
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
    {
        if field.name() == Some("file") {
            let declared = field
                .content_type()
                .map(|m| m.to_string())
                .unwrap_or_else(|| "text/csv".to_string());
            let filename = field.file_name().unwrap_or("statement.csv").to_string();
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            csv_data = Some((declared, filename, bytes));
        }
    }

    let (declared, filename, bytes) =
        csv_data.ok_or(AppError::Validation("No file uploaded".into()))?;

    // `s10-upload-validation`: bank-statement CSVs are sniffed
    // with the same policy as document uploads — CSV is
    // accepted on declaration, anything else is rejected if
    // sniff disagrees.
    let _mime = upload::validate(&declared, Some(&filename), &bytes)
        .map_err(|e| AppError::Validation(e.message()))?;

    let text = std::str::from_utf8(&bytes)
        .map_err(|_| AppError::Validation("Invalid UTF-8 in statement file".into()))?
        .to_string();

    // `data-interchange`: content sniffing picks the format — CSV,
    // OFX/QFX, QIF, CAMT.052/053, or MT940. Extensions are ignored.
    let account_currency: String =
        sqlx::query_scalar("SELECT currency FROM accounts WHERE id = $1")
            .bind(account_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(AppError::NotFound)?;

    enum Line {
        Csv(csv::StringRecord),
        Stmt(crate::import::statement::StatementLine),
    }
    let lines: Vec<Line> = match crate::import::statement::sniff_format(&bytes) {
        Some(format) => {
            let qif_order = crate::import::statement::qif::DateOrder::Us;
            let parsed = crate::import::statement::parse(format, &text, qif_order)
                .map_err(AppError::Validation)?;
            parsed.into_iter().map(Line::Stmt).collect()
        }
        None => {
            let mut rdr = csv::ReaderBuilder::new()
                .has_headers(true)
                .from_reader(text.as_bytes());
            rdr.records()
                .map(|r| r.map(Line::Csv))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| AppError::Internal(e.to_string()))?
        }
    };

    let mut tx = state.pool.begin().await?;
    let mut count = 0i64;
    let mut duplicates = 0i64;
    let mut row_errors: Vec<String> = Vec::new();
    for line in lines {
        let (date, description, amount, check_number, external_id, line_currency) = match line {
            Line::Csv(record) => {
                if record.len() < 3 {
                    continue;
                }
                let date = match NaiveDate::parse_from_str(&record[0], "%Y-%m-%d") {
                    Ok(d) => d,
                    Err(_) => {
                        row_errors.push(format!("bad CSV date '{}'", &record[0]));
                        continue;
                    }
                };
                let amount: Decimal = match record[2].parse() {
                    Ok(a) => a,
                    Err(_) => {
                        row_errors.push(format!("bad CSV amount '{}'", &record[2]));
                        continue;
                    }
                };
                let desc = record.get(1).unwrap_or("").to_string();
                let check = if record.len() > 3 {
                    let c = record.get(3).unwrap_or("").to_string();
                    if c.is_empty() {
                        None
                    } else {
                        Some(c)
                    }
                } else {
                    None
                };
                (date, desc, amount, check, None, None)
            }
            Line::Stmt(l) => (
                l.date,
                l.payee.clone(),
                l.amount,
                l.memo,
                l.external_id,
                l.currency,
            ),
        };

        // Currency mismatch fails loudly per row (`data-interchange`):
        // FX conversion belongs to multi-currency-fx, not the importer.
        if let Some(ccy) = &line_currency {
            if !ccy.eq_ignore_ascii_case(&account_currency) {
                row_errors.push(format!("{ccy} vs account {account_currency}"));
                continue;
            }
        }

        // Cross-format duplicate detection: stable external ids use the
        // unique index; fingerprint fallback (QIF) checks
        // (date, amount, normalized payee).
        if let Some(ext) = &external_id {
            let exists: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM bank_statement_lines WHERE account_id = $1 AND external_id = $2",
            )
            .bind(account_id)
            .bind(ext)
            .fetch_optional(&mut *tx)
            .await?;
            if exists.is_some() {
                duplicates += 1;
                continue;
            }
        } else {
            let norm = crate::import::statement::normalize_payee(&description);
            let exists: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM bank_statement_lines
                 WHERE account_id = $1 AND statement_date = $2 AND amount = $3
                       AND LOWER(description) = LOWER($4)",
            )
            .bind(account_id)
            .bind(date)
            .bind(amount)
            .bind(&norm)
            .fetch_optional(&mut *tx)
            .await?;
            if exists.is_some() {
                duplicates += 1;
                continue;
            }
        }

        sqlx::query(
            r#"INSERT INTO bank_statement_lines (ledger_id, account_id, statement_date, description, amount, check_number, external_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(ledger_id)
        .bind(account_id)
        .bind(date)
        .bind(&description)
        .bind(amount)
        .bind(&check_number)
        .bind(&external_id)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }
    tx.commit().await?;

    if !row_errors.is_empty() {
        return Err(AppError::Validation(format!(
            "imported {count} lines; {} row(s) rejected: {}",
            row_errors.len(),
            row_errors
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }
    if duplicates > 0 {
        tracing::info!(%ledger_id, %account_id, duplicates, "statement import skipped duplicates");
    }

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "import",
        "bank_statement",
        None,
        None,
        Some(serde_json::json!({
            "account_id": account_id.to_string(),
            "lines_imported": count,
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/reconcile/{}", ledger_id, account_id)).into_response())
}

pub async fn match_line(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<MatchForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let mut tx = state.pool.begin().await?;

    sqlx::query("UPDATE bank_statement_lines SET status = 'matched', matched_transaction_id = $1 WHERE id = $2")
        .bind(form.transaction_id)
        .bind(form.line_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("UPDATE transactions SET is_reconciled = TRUE WHERE id = $1")
        .bind(form.transaction_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    // `payee-learning`: confirmed matches teach the alias store. The
    // contra account (non-cash side of the matched transaction) is the
    // suggested category next time.
    {
        let stmt_desc: Option<String> =
            sqlx::query_scalar("SELECT description FROM bank_statement_lines WHERE id = $1")
                .bind(form.line_id)
                .fetch_optional(&state.pool)
                .await?
                .flatten();
        let txn_payee: Option<Option<String>> =
            sqlx::query_scalar("SELECT payee FROM transactions WHERE id = $1")
                .bind(form.transaction_id)
                .fetch_optional(&state.pool)
                .await?;
        let contra: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT p.account_id FROM postings p
               JOIN transactions t ON t.id = p.transaction_id
               WHERE t.id = $1 AND p.account_id <> $2
               LIMIT 1"#,
        )
        .bind(form.transaction_id)
        .bind(account_id)
        .fetch_optional(&state.pool)
        .await?
        .flatten();
        if let (Some(desc), Some(Some(payee))) = (stmt_desc, txn_payee) {
            crate::domain::payee_learning::record_from_match(
                &state.pool,
                ledger_id,
                &desc,
                &payee,
                contra,
            )
            .await;
        }
    }

    Ok(Redirect::to(&format!("/ledgers/{}/reconcile/{}", ledger_id, account_id)).into_response())
}

pub async fn exclude_line(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id, line_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    sqlx::query("UPDATE bank_statement_lines SET status = 'excluded' WHERE id = $1")
        .bind(line_id)
        .execute(&state.pool)
        .await?;

    Ok(Redirect::to(&format!("/ledgers/{}/reconcile/{}", ledger_id, account_id)).into_response())
}

pub async fn complete(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let statement_balance: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(amount), 0) FROM bank_statement_lines
           WHERE ledger_id = $1 AND account_id = $2 AND status IN ('matched', 'excluded')"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .fetch_one(&state.pool)
    .await?;

    let ledger_balance: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(p.amount), 0) FROM postings p
           JOIN transactions t ON t.id = p.transaction_id
           WHERE t.ledger_id = $1 AND p.account_id = $2 AND p.direction = 'DEBIT' AND t.is_reconciled = TRUE"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .fetch_one(&state.pool)
    .await?;

    let difference = statement_balance - ledger_balance;
    let today = chrono::Utc::now().date_naive();

    sqlx::query(
        r#"INSERT INTO reconciliations (ledger_id, account_id, statement_date, statement_balance, ledger_balance, difference, completed_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .bind(today)
    .bind(statement_balance)
    .bind(ledger_balance)
    .bind(difference)
    .bind(user.id)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "reconcile",
        "bank_account",
        Some(account_id),
        None,
        Some(serde_json::json!({
            "statement_balance": statement_balance.to_string(),
            "ledger_balance": ledger_balance.to_string(),
            "difference": difference.to_string(),
        })),
    )
    .await;

    // Domain metric (`o4-metrics-endpoint`).
    crate::observability::metrics::reconciliation_completed();

    Ok(Redirect::to(&format!(
        "/ledgers/{}/reconcile/{}/history",
        ledger_id, account_id
    ))
    .into_response())
}

pub async fn history(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let rows = sqlx::query_as::<
        _,
        (
            chrono::NaiveDate,
            Decimal,
            Decimal,
            Decimal,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"SELECT statement_date, statement_balance, ledger_balance, difference, completed_at
           FROM reconciliations
           WHERE ledger_id = $1 AND account_id = $2
           ORDER BY statement_date DESC, completed_at DESC"#,
    )
    .bind(ledger_id)
    .bind(account_id)
    .fetch_all(&state.pool)
    .await?;

    let history: Vec<(
        NaiveDate,
        Decimal,
        Decimal,
        Decimal,
        chrono::DateTime<chrono::Utc>,
    )> = rows;

    Ok(render_response(ReconHistory {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        account_id,
        history: history.iter().map(|r| (r.0, r.1, r.2, r.3, r.4)).collect(),
    }))
}
