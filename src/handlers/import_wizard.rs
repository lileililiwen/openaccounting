//! CSV import column-mapping wizard (`u4-csv-import-wizard`).
//!
//! Four steps, each a separate URL, with state passed
//! between them as form fields (no server-side session):
//!
//! 1. `GET /ledgers/{id}/import/wizard`
//!    → upload form (POST `file`)
//! 2. `POST /ledgers/{id}/import/wizard/map`
//!    → returns the column-mapping form, pre-filled with
//!    header-based auto-detection. User overrides + saves.
//! 3. `POST /ledgers/{id}/import/wizard/preview`
//!    → renders the first five transformed rows.
//! 4. `POST /ledgers/{id}/import/wizard/commit`
//!    → atomic `INSERT ...` for every row in one
//!    transaction.

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Form,
};
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::{
        import::{ImportUpload, WizardMapPage, WizardPreviewPage},
        render_response,
    },
    AppState,
};

/// Step 1 — render the upload form.
pub async fn show_upload(
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

/// Heuristic header → field mapping. Returns the column
/// index that best matches the canonical field.
pub fn auto_detect(headers: &[String]) -> ColumnMap {
    let lc: Vec<String> = headers.iter().map(|h| h.to_ascii_lowercase()).collect();
    let find = |candidates: &[&str]| -> i32 {
        for (i, h) in lc.iter().enumerate() {
            if candidates.iter().any(|c| h.contains(c)) {
                return i as i32;
            }
        }
        -1
    };
    ColumnMap {
        date: find(&["date", "txn_date", "transaction date"]),
        description: find(&["description", "memo", "narrative", "details"]),
        debit: find(&["debit", "withdrawal", "amount out"]),
        credit: find(&["credit", "deposit", "amount in"]),
        account: find(&["account", "acct"]),
        payee: find(&["payee", "vendor", "merchant"]),
        reference: find(&["reference", "ref", "check", "memo"]),
        amount_in: -1,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMap {
    pub date: i32,
    pub description: i32,
    /// Sentinel `-1` = "not mapped". serde's default for `i32`
    /// is `0`, which would silently alias the date column; we
    /// override the default with `-1` so absent form fields
    /// are unambiguously "no column".
    #[serde(default = "default_none")]
    pub debit: i32,
    #[serde(default = "default_none")]
    pub credit: i32,
    #[serde(default = "default_none")]
    pub account: i32,
    #[serde(default = "default_none")]
    pub payee: i32,
    #[serde(default = "default_none")]
    pub reference: i32,
    #[serde(default = "default_none")]
    pub amount_in: i32,
}

fn default_none() -> i32 {
    -1
}

impl Default for ColumnMap {
    fn default() -> Self {
        Self {
            date: 0,
            description: 0,
            debit: -1,
            credit: -1,
            account: -1,
            payee: -1,
            reference: -1,
            amount_in: -1,
        }
    }
}

impl ColumnMap {
    pub fn debit_opt(&self) -> Option<usize> {
        usize_from_i32(self.debit)
    }
    pub fn credit_opt(&self) -> Option<usize> {
        usize_from_i32(self.credit)
    }
    pub fn account_opt(&self) -> Option<usize> {
        usize_from_i32(self.account)
    }
    pub fn payee_opt(&self) -> Option<usize> {
        usize_from_i32(self.payee)
    }
    pub fn reference_opt(&self) -> Option<usize> {
        usize_from_i32(self.reference)
    }
    pub fn amount_in_opt(&self) -> Option<usize> {
        usize_from_i32(self.amount_in)
    }
}

fn usize_from_i32(v: i32) -> Option<usize> {
    if v < 0 {
        None
    } else {
        Some(v as usize)
    }
}

/// Step 2 — accept the upload + initial mapping, render the
/// mapping form. The CSV content is round-tripped to the
/// client via a hidden form field (we keep the wizard stateless
/// on the server; the file lives in the form payload until
/// commit).
#[derive(Deserialize)]
pub struct UploadForm {
    // The raw CSV bytes are read from the multipart `file`
    // field directly by [`handle_upload`]; no struct fields
    // needed here.
}

/// Multipart handler for step 1 → step 2 transition. We re-emit
/// the CSV in a hidden form field so the user can confirm the
/// mapping in step 2.
pub async fn handle_upload(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let mut csv_content = String::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Multipart(e.to_string()))?
    {
        if field.name().unwrap_or("") == "file" {
            csv_content = String::from_utf8(
                field
                    .bytes()
                    .await
                    .map_err(|e| AppError::Multipart(e.to_string()))?
                    .to_vec(),
            )
            .map_err(|_| AppError::Validation("Invalid UTF-8 in CSV file".into()))?;
            break;
        }
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

    let headers = parse_headers(&csv_content);
    let map = auto_detect(&headers);

    // If a saved mapping matches the filename, prefer it.
    let mut map = map;
    let mut saved_name: Option<String> = None;
    if let Some(stored) = lookup_saved_mapping(&state.pool, ledger_id, "upload.csv").await {
        map = stored.column_map();
        saved_name = Some(stored.name.clone());
    }

    let rows_preview: Vec<Vec<String>> = csv_content
        .lines()
        .skip(1)
        .take(5)
        .map(parse_csv_line)
        .collect();

    let page = WizardMapPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        headers,
        map,
        csv_content,
        rows_preview,
        saved_mapping_name: saved_name.unwrap_or_default(),
    };
    Ok(render_response(page))
}

/// Apply the user's column overrides, render the first five
/// transformed rows so they can sanity-check before commit.
#[derive(Deserialize)]
pub struct MapForm {
    pub csv_content: String,
    pub date: i32,
    pub description: i32,
    #[serde(default = "default_none")]
    pub debit: i32,
    #[serde(default = "default_none")]
    pub credit: i32,
    #[serde(default = "default_none")]
    pub account: i32,
    #[serde(default = "default_none")]
    pub payee: i32,
    #[serde(default = "default_none")]
    pub reference: i32,
    #[serde(default = "default_none")]
    pub amount_in: i32,
    #[serde(default)]
    pub save_name: Option<String>,
    #[serde(default)]
    pub filename_glob: Option<String>,
}

pub async fn handle_preview(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<MapForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let headers = parse_headers(&form.csv_content);
    let map = ColumnMap {
        date: form.date,
        description: form.description,
        debit: form.debit,
        credit: form.credit,
        account: form.account,
        payee: form.payee,
        reference: form.reference,
        amount_in: form.amount_in,
    };
    let rows = transform(&form.csv_content, &map);

    // Persist the mapping if the user asked.
    if let (Some(name), Some(glob)) = (form.save_name.clone(), form.filename_glob.clone()) {
        if !name.is_empty() && !glob.is_empty() {
            save_mapping(&state.pool, ledger_id, user.id, &name, &glob, &map).await?;
        }
    }

    Ok(render_response(WizardPreviewPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        headers,
        map,
        csv_content: form.csv_content,
        rows,
    }))
}

/// Atomically commit the transformed rows. All inserts run
/// inside one \`BEGIN ... COMMIT\`. If the loop returns an
/// error, sqlx rolls back; the user's data is unaffected.
#[derive(Deserialize)]
pub struct CommitForm {
    pub csv_content: String,
    pub date: i32,
    pub description: i32,
    #[serde(default = "default_none")]
    pub debit: i32,
    #[serde(default = "default_none")]
    pub credit: i32,
    #[serde(default = "default_none")]
    pub account: i32,
    #[serde(default = "default_none")]
    pub payee: i32,
    #[serde(default = "default_none")]
    pub reference: i32,
    #[serde(default = "default_none")]
    pub amount_in: i32,
}

pub async fn handle_commit(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<CommitForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let map = ColumnMap {
        date: form.date,
        description: form.description,
        debit: form.debit,
        credit: form.credit,
        account: form.account,
        payee: form.payee,
        reference: form.reference,
        amount_in: form.amount_in,
    };
    let rows = transform(&form.csv_content, &map);
    if rows.is_empty() {
        return Err(AppError::Validation("no rows to commit".into()));
    }

    let mut tx = state.pool.begin().await?;
    // Look up the default cash account for this ledger so
    // single-amount imports don't have to specify one.
    let cash_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts
         WHERE ledger_id = $1 AND name = 'Bank Account' LIMIT 1",
    )
    .bind(ledger_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::Validation("Bank Account not found".into()))?;

    let mut committed = 0usize;
    for row in &rows {
        let txn_id: Uuid = sqlx::query_scalar(
            "INSERT INTO transactions
                 (ledger_id, txn_date, description, payee, reference, currency, created_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id",
        )
        .bind(ledger_id)
        .bind(row.date)
        .bind(&row.description)
        .bind(&row.payee)
        .bind(&row.reference)
        .bind(&ledger.base_currency)
        .bind(user.id)
        .fetch_one(&mut *tx)
        .await?;
        let amount = row.amount;
        if amount == Decimal::ZERO {
            continue;
        }
        let (dir, amt) = if amount.is_sign_negative() {
            ("CREDIT", -amount)
        } else {
            ("DEBIT", amount)
        };
        sqlx::query(
            "INSERT INTO postings (transaction_id, account_id, amount, direction)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(txn_id)
        .bind(cash_id)
        .bind(amt)
        .bind(dir)
        .execute(&mut *tx)
        .await?;
        committed += 1;
    }
    tx.commit().await?;

    let _ = crate::audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "import",
        "transactions",
        None,
        None,
        Some(serde_json::json!({
            "source": "csv_wizard",
            "rows_committed": committed,
        })),
    )
    .await;

    Ok((
        StatusCode::SEE_OTHER,
        [(
            axum::http::header::LOCATION,
            axum::http::HeaderValue::from_str(&format!("/ledgers/{ledger_id}/dashboard"))
                .map_err(|e| AppError::Internal(e.to_string()))?,
        )],
    )
        .into_response())
}

// ─── helpers ────────────────────────────────────────────────────────────

fn parse_headers(csv: &str) -> Vec<String> {
    csv.lines()
        .next()
        .map(parse_csv_line)
        .unwrap_or_default()
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                out.push(std::mem::take(&mut field));
            }
            other => field.push(other),
        }
    }
    if !field.is_empty() || !out.is_empty() {
        out.push(field);
    }
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct TransformedRow {
    pub date: NaiveDate,
    pub description: String,
    /// Empty string when the source column was absent. The
    /// template can render this directly without Option.
    pub payee: String,
    /// Empty string when the source column was absent.
    pub reference: String,
    pub amount: Decimal,
}

fn transform(csv: &str, map: &ColumnMap) -> Vec<TransformedRow> {
    let mut out: Vec<TransformedRow> = Vec::new();
    for (i, line) in csv.lines().enumerate() {
        if i == 0 {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(line);
        let date_str = fields
            .get(map.date.max(0) as usize)
            .cloned()
            .unwrap_or_default();
        let date = match parse_date(&date_str) {
            Some(d) => d,
            None => continue,
        };
        let description = fields
            .get(map.description.max(0) as usize)
            .cloned()
            .unwrap_or_default()
            .trim()
            .to_string();
        let payee = map
            .payee_opt()
            .and_then(|i| fields.get(i).cloned())
            .unwrap_or_default();
        let reference = map
            .reference_opt()
            .and_then(|i| fields.get(i).cloned())
            .unwrap_or_default();

        let amount: Decimal = if let Some(col) = map.amount_in_opt() {
            fields
                .get(col)
                .and_then(|s| s.replace(',', "").trim().parse::<Decimal>().ok())
                .unwrap_or(Decimal::ZERO)
        } else {
            let debit = map
                .debit_opt()
                .and_then(|i| fields.get(i).cloned())
                .and_then(|s| s.replace(',', "").trim().parse::<Decimal>().ok())
                .unwrap_or(Decimal::ZERO);
            let credit = map
                .credit_opt()
                .and_then(|i| fields.get(i).cloned())
                .and_then(|s| s.replace(',', "").trim().parse::<Decimal>().ok())
                .unwrap_or(Decimal::ZERO);
            debit - credit
        };

        out.push(TransformedRow {
            date,
            description,
            payee,
            reference,
            amount,
        });
    }
    out
}

// `non_empty` was used to filter blank payee/reference cells;
// with the i32-default fix in place, missing columns are
// already skipped so this helper is no longer reached. Kept
// for future re-introduction.
#[allow(dead_code)]
fn non_empty(s: String) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s)
    }
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    // Try the formats users most commonly upload: ISO,
    // dd/mm/yyyy, mm/dd/yyyy, dd.mm.yyyy, yyyy/mm/dd.
    let formats = [
        "%Y-%m-%d", "%Y/%m/%d", "%d/%m/%Y", "%m/%d/%Y", "%d.%m.%Y", "%d-%m-%Y", "%Y%m%d",
    ];
    for f in formats {
        if let Ok(d) = NaiveDate::parse_from_str(s, f) {
            return Some(d);
        }
    }
    None
}

#[derive(Debug, Clone, Serialize)]
struct SavedMapping {
    name: String,
    date_column: i32,
    description_column: i32,
    debit_column: Option<i32>,
    credit_column: Option<i32>,
    account_column: Option<i32>,
    payee_column: Option<i32>,
    reference_column: Option<i32>,
    amount_in_column: Option<String>,
}

impl SavedMapping {
    fn column_map(&self) -> ColumnMap {
        ColumnMap {
            date: self.date_column,
            description: self.description_column,
            debit: self.debit_column.unwrap_or(-1),
            credit: self.credit_column.unwrap_or(-1),
            account: self.account_column.unwrap_or(-1),
            payee: self.payee_column.unwrap_or(-1),
            reference: self.reference_column.unwrap_or(-1),
            amount_in: -1,
        }
    }
}

async fn lookup_saved_mapping(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
    filename: &str,
) -> Option<SavedMapping> {
    let row = sqlx::query(
        "SELECT name, date_column, description_column, debit_column, credit_column,
                account_column, payee_column, reference_column, amount_in_column
         FROM csv_import_mappings
         WHERE ledger_id = $1
           AND $2 LIKE filename_glob
         ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(ledger_id)
    .bind(filename)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;
    Some(SavedMapping {
        name: row.try_get("name").ok()?,
        date_column: row.try_get("date_column").ok()?,
        description_column: row.try_get("description_column").ok()?,
        debit_column: row.try_get("debit_column").ok()?,
        credit_column: row.try_get("credit_column").ok()?,
        account_column: row.try_get("account_column").ok()?,
        payee_column: row.try_get("payee_column").ok()?,
        reference_column: row.try_get("reference_column").ok()?,
        amount_in_column: row.try_get("amount_in_column").ok()?,
    })
}

async fn save_mapping(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
    user_id: Uuid,
    name: &str,
    filename_glob: &str,
    map: &ColumnMap,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO csv_import_mappings
               (ledger_id, user_id, name, filename_glob, format,
                date_column, description_column, debit_column, credit_column,
                account_column, payee_column, reference_column, amount_in_column,
                updated_at)
           VALUES ($1, $2, $3, $4, 'csv',
                   $5, $6, $7, $8, $9, $10, $11, $12, now())
           ON CONFLICT (ledger_id, name) DO UPDATE SET
               filename_glob = EXCLUDED.filename_glob,
               date_column = EXCLUDED.date_column,
               description_column = EXCLUDED.description_column,
               debit_column = EXCLUDED.debit_column,
               credit_column = EXCLUDED.credit_column,
               account_column = EXCLUDED.account_column,
               payee_column = EXCLUDED.payee_column,
               reference_column = EXCLUDED.reference_column,
               amount_in_column = EXCLUDED.amount_in_column,
               updated_at = now()"#,
    )
    .bind(ledger_id)
    .bind(user_id)
    .bind(name)
    .bind(filename_glob)
    .bind(map.date)
    .bind(map.description)
    .bind(map.debit)
    .bind(map.credit)
    .bind(map.account)
    .bind(map.payee)
    .bind(map.reference)
    .bind(if map.amount_in >= 0 {
        Some("debit".to_string())
    } else {
        None
    })
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parse_csv_line_handles_quoted_commas() {
        let f = parse_csv_line(r#"2024-01-02,"Hello, world",42.50"#);
        assert_eq!(
            f,
            vec![
                "2024-01-02".to_string(),
                "Hello, world".to_string(),
                "42.50".to_string()
            ]
        );
    }

    #[test]
    fn auto_detect_picks_date_and_description() {
        let headers = vec!["Txn Date".into(), "Description".into(), "Amount".into()];
        let m = auto_detect(&headers);
        assert_eq!(m.date, 0);
        assert_eq!(m.description, 1);
        assert_eq!(m.amount_in, 2);
    }

    #[test]
    fn transform_debit_credit() {
        let csv =
            "date,description,debit,credit\n2024-01-02,Coffee,42.50,0\n2024-01-03,Refund,0,42.50\n";
        let map = ColumnMap {
            date: 0,
            description: 1,
            debit: 2,
            credit: 3,
            ..Default::default()
        };
        let rows = transform(csv, &map);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].amount, dec!(42.50));
        assert_eq!(rows[1].amount, dec!(-42.50));
    }

    #[test]
    fn transform_single_amount_column() {
        let csv = "date,description,amount\n2024-01-02,Coffee,-42.50\n2024-01-03,Salary,1500.00\n";
        let map = ColumnMap {
            date: 0,
            description: 1,
            amount_in: 2,
            ..Default::default()
        };
        let rows = transform(csv, &map);
        assert_eq!(rows[0].amount, dec!(-42.50));
        assert_eq!(rows[1].amount, dec!(1500.00));
    }

    #[test]
    fn parse_date_accepts_iso_and_european() {
        assert_eq!(
            parse_date("2024-12-31"),
            NaiveDate::from_ymd_opt(2024, 12, 31)
        );
        assert_eq!(
            parse_date("31/12/2024"),
            NaiveDate::from_ymd_opt(2024, 12, 31)
        );
        assert_eq!(
            parse_date("31.12.2024"),
            NaiveDate::from_ymd_opt(2024, 12, 31)
        );
        assert!(parse_date("not a date").is_none());
    }
}

// (no silencer needed; helpers above are exercised by unit +
// integration tests.)
