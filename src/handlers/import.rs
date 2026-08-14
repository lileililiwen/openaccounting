use crate::templates::render_response;
use axum::extract::{Multipart, Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_login::AuthSession;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::Backend,
    audit,
    domain::Account,
    error::{AppError, AppResult},
    handlers::ledgers,
    import::sniff,
    templates::import::{ImportPreview, ImportUpload},
    AppState,
};

#[derive(Clone, Debug, Default)]
pub struct CsvMapping {
    pub date_column: usize,
    pub description_column: usize,
    pub debit_column: Option<usize>,
    pub credit_column: Option<usize>,
    pub account_column: Option<usize>,
    pub payee_column: Option<usize>,
    pub reference_column: Option<usize>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ParsedRow {
    pub date: String,
    pub description: String,
    pub debit: String,
    pub credit: String,
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default)]
    pub payee: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub is_duplicate: bool,
}

pub async fn upload_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    Ok(render_response(ImportUpload {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        error: String::new(),
    }))
}

pub async fn upload(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let mut csv_content = String::new();
    let mut filename = String::new();

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Multipart(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" {
            continue;
        }
        filename = field.file_name().unwrap_or("upload.csv").to_string();
        let chunk = field
            .chunk()
            .await
            .map_err(|e| AppError::Multipart(e.to_string()))?
            .ok_or_else(|| AppError::Validation("Empty file".into()))?;
        csv_content = String::from_utf8(chunk.to_vec())
            .map_err(|_| AppError::Validation("Invalid UTF-8 in CSV file".into()))?;
    }

    if csv_content.is_empty() {
        return Ok(render_response(ImportUpload {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name,
            error: "No file provided".into(),
        }));
    }

    // Sniff the format and dispatch. The CSV path keeps the
    // existing 7-column contract. OFX / QIF / MT940 use the
    // dedicated parsers in `crate::import::`.
    let format = sniff::detect(&csv_content);
    let (headers, rows) = match format {
        sniff::Format::Csv => {
            let all_rows = crate::import::csv::parse(&csv_content);
            let headers = vec![
                "date".to_string(),
                "description".to_string(),
                "debit".to_string(),
                "credit".to_string(),
                "account".to_string(),
                "payee".to_string(),
                "reference".to_string(),
            ];
            // Preview cap: show the first 10 000 rows; the
            // commit handler re-parses the full file (or in
            // the current implementation, the rows the
            // preview sent back via the form).
            let rows = all_rows.into_iter().take(10_000).collect();
            (headers, rows)
        }
        sniff::Format::Ofx => {
            let rows = crate::import::ofx::parse(&csv_content);
            let headers = vec![
                "date".to_string(),
                "description".to_string(),
                "debit".to_string(),
                "credit".to_string(),
                "payee".to_string(),
                "reference".to_string(),
            ];
            (headers, rows)
        }
        sniff::Format::Qif => {
            let rows = crate::import::qif::parse(&csv_content);
            let headers = vec![
                "date".to_string(),
                "description".to_string(),
                "debit".to_string(),
                "credit".to_string(),
                "payee".to_string(),
                "reference".to_string(),
            ];
            (headers, rows)
        }
        sniff::Format::Mt940 => {
            let mt = crate::import::mt940::parse(&csv_content);
            let headers = vec![
                "date".to_string(),
                "description".to_string(),
                "debit".to_string(),
                "credit".to_string(),
                "payee".to_string(),
                "reference".to_string(),
            ];
            (headers, mt.rows)
        }
    };

    // Get accounts for mapping
    let accounts = sqlx::query_as::<_, Account>(
        r#"SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
           FROM accounts WHERE ledger_id = $1 AND is_archived = FALSE ORDER BY type, code, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(ImportPreview {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        filename,
        format: format.as_str().to_string(),
        headers,
        rows,
        accounts,
        mapping: CsvMapping::default(),
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct ImportConfirmForm {
    pub filename: String,
    #[serde(default)]
    pub date_column: usize,
    #[serde(default)]
    pub description_column: usize,
    #[serde(default)]
    pub debit_column: Option<usize>,
    #[serde(default)]
    pub credit_column: Option<usize>,
    pub account_id: Uuid,
    /// Default account the bank-statement importers post the
    /// cash leg to. The CSV path uses `account_id` for the
    /// same purpose.
    #[serde(default)]
    pub default_account_id: Uuid,
    #[serde(default)]
    pub skip_duplicates: bool,
    /// The pre-parsed rows from the platform-specific
    /// parsers. Each row carries its own date / description /
    /// debit / credit / payee / reference. When this field is
    /// present the legacy `*_column` indices are ignored.
    ///
    /// The form is sent as a single JSON string in the
    /// `rows` field (axum's `Form` deserializer doesn't
    /// natively understand nested arrays; we parse the JSON
    /// ourselves inside `confirm`).
    #[serde(default)]
    pub rows: String,
}

pub async fn confirm(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    form: axum::Form<ImportConfirmForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    // For the bank-statement importers (this change), we
    // require a `default_account_id` form field and a JSON-
    // encoded `rows` field; the CSV-only fields `date_column`
    // etc. are accepted but ignored (the format-specific
    // parsers already produce a canonical 7-column shape).
    let parsed_rows: Vec<ParsedRow> = if form.rows.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&form.rows).map_err(|e| {
            AppError::Validation(format!("could not parse rows: {e}"))
        })?
    };
    // Look up the ledger's default cash account once for the
    // whole batch. We pick the first ASSET account whose name
    // matches the cash heuristic (Cash / Bank); this is the
    // same one the cash-flow report uses.
    let cash_id: Uuid = sqlx::query_scalar(
        r#"SELECT id FROM accounts
           WHERE ledger_id = $1
             AND type = 'ASSET'
             AND (LOWER(name) LIKE '%cash%' OR LOWER(name) LIKE '%bank%')
           ORDER BY code NULLS LAST
           LIMIT 1"#,
    )
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or(form.default_account_id);
    let mut tx = state.pool.begin().await?;
    let mut created = 0u32;
    let mut failed = Vec::new();
    for row in &parsed_rows {
        if row.date.trim().is_empty() {
            failed.push(format!("missing date: {}", row.description));
            continue;
        }
        let debit = row.debit.trim();
        let credit = row.credit.trim();
        if debit.is_empty() && credit.is_empty() {
            failed.push(format!("no amount: {}", row.description));
            continue;
        }
        if !debit.is_empty() && !credit.is_empty() {
            failed.push(format!(
                "row '{}': exactly one of debit or credit must be set",
                row.description
            ));
            continue;
        }
        let amount = match (debit.is_empty(), credit.is_empty()) {
            (true, false) => credit
                .parse::<Decimal>()
                .map_err(|e| {
                    format!("row '{}': bad credit '{credit}': {e}", row.description)
                }),
            (false, true) => debit
                .parse::<Decimal>()
                .map_err(|e| {
                    format!("row '{}': bad debit '{debit}': {e}", row.description)
                }),
            _ => unreachable!(),
        };
        let amount = match amount {
            Ok(a) => a,
            Err(msg) => {
                failed.push(msg);
                continue;
            }
        };
        if amount <= Decimal::ZERO {
            failed.push(format!("row '{}': non-positive amount", row.description));
            continue;
        }
        let parsed_date = match chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(e) => {
                failed.push(format!(
                    "row '{}': bad date '{}': {e}",
                    row.description, row.date
                ));
                continue;
            }
        };
        let (txn_id,): (Uuid,) = sqlx::query_as(
            r#"INSERT INTO transactions
                  (ledger_id, txn_date, description, payee, reference,
                   currency, created_by)
               VALUES ($1, $2, $3, $4, $5, 'USD', $6) RETURNING id"#,
        )
        .bind(ledger_id)
        .bind(parsed_date)
        .bind(&row.description)
        .bind(row.payee.as_deref().unwrap_or(""))
        .bind(row.reference.as_deref().unwrap_or(""))
        .bind(user.id)
        .fetch_one(&mut *tx)
        .await?;
        let cash_id = {
            // Use the auto-detected cash account (the
            // batch-level `cash_id` looked up above).
            cash_id
        };
        let other_id = row
            .account
            .as_deref()
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or(form.default_account_id);
        let (debit_acct, credit_acct) = if !debit.is_empty() {
            // Cash out: DR row account, CR cash.
            (other_id, cash_id)
        } else {
            // Cash in: DR cash, CR row account.
            (cash_id, other_id)
        };
        for (acct, direction) in [
            (debit_acct, "DEBIT"),
            (credit_acct, "CREDIT"),
        ] {
            sqlx::query(
                r#"INSERT INTO postings (transaction_id, account_id, amount, direction)
                   VALUES ($1, $2, $3, $4)"#,
            )
            .bind(txn_id)
            .bind(acct)
            .bind(amount)
            .bind(direction)
            .execute(&mut *tx)
            .await?;
        }
        created += 1;
    }
    if !failed.is_empty() {
        // Atomicity: a single bad row rolls back the whole
        // batch.
        let _ = tx.rollback().await;
        return Err(AppError::Unprocessable(format!(
            "import rolled back: {} valid, {} failed: {}",
            created,
            failed.len(),
            failed.join("; ")
        )));
    }
    tx.commit().await?;
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "import",
        "transaction",
        None,
        None,
        Some(serde_json::json!({
            "filename": form.filename,
            "account_id": form.default_account_id,
            "rows_committed": created,
        })),
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{}/transactions", ledger_id)).into_response())
}
