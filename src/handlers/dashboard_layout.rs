//! Per-user dashboard layout (`u5-dashboard-widgets`).
//!
//! Eight built-in widgets:
//!
//! | ID                | Section on the dashboard                                |
//! |-------------------|--------------------------------------------------------|
//! | `kpis`            | 6-up KPI grid (assets / liab / NW / MTD income + exp + net) |
//! | `cash_runway`     | Cash-runway + AR + AP summary KPI row                   |
//! | `charts`          | Income vs. expense + expense-breakdown SVG row         |
//! | `top_expenses`    | Top expense categories (this month) list               |
//! | `recent_txns`     | Recent transactions list                                |
//! | `budget_burn`     | Budget vs. actual per active budget                     |
//! | `account_balances`| Account balances table (top 10 by absolute balance)   |
//!
//! The default order matches the existing pre-spec dashboard:
//! `[kpis, cash_runway, charts, top_expenses]`. Users may
//! override via `POST /ledgers/{id}/dashboard/layout`.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Form,
};
use axum_login::AuthSession;
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    AppState,
};

/// Built-in widget vocabulary. Keep in sync with the
/// `widgets_vocabulary` CHECK in migration 0034.
pub const VOCABULARY: &[&str] = &[
    "kpis",
    "cash_runway",
    "charts",
    "top_expenses",
    "recent_txns",
    "budget_burn",
    "account_balances",
];

/// Default layout for a fresh user — the four widgets that
/// ship enabled.
pub const DEFAULT_LAYOUT: &[&str] = &["kpis", "cash_runway", "charts", "top_expenses"];

/// Load the user's layout for `ledger_id`. Returns
/// [`DEFAULT_LAYOUT`] when no row exists.
pub async fn load(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    ledger_id: Uuid,
) -> sqlx::Result<Vec<String>> {
    let row: Option<(Vec<String>,)> = sqlx::query_as(
        "SELECT widgets FROM dashboard_layouts WHERE user_id = $1 AND ledger_id = $2",
    )
    .bind(user_id)
    .bind(ledger_id)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(|(w,)| w)
        .unwrap_or_else(|| DEFAULT_LAYOUT.iter().map(|s| s.to_string()).collect()))
}

/// Replace the user's layout for `ledger_id`. The widget
/// list is validated against [`VOCABULARY`]; unknown IDs
/// trigger a 400 so a corrupted form can't poison the row.
pub async fn save(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    ledger_id: Uuid,
    widgets: &[String],
) -> AppResult<()> {
    for w in widgets {
        if !VOCABULARY.contains(&w.as_str()) {
            return Err(AppError::Validation(format!("unknown widget: {w}")));
        }
    }
    sqlx::query(
        r#"INSERT INTO dashboard_layouts (user_id, ledger_id, widgets, updated_at)
           VALUES ($1, $2, $3, now())
           ON CONFLICT (user_id, ledger_id)
           DO UPDATE SET widgets = EXCLUDED.widgets, updated_at = now()"#,
    )
    .bind(user_id)
    .bind(ledger_id)
    .bind(widgets)
    .execute(pool)
    .await?;
    Ok(())
}

/// Reset the user's layout for `ledger_id` to
/// [`DEFAULT_LAYOUT`] by deleting the row.
pub async fn reset(pool: &sqlx::PgPool, user_id: Uuid, ledger_id: Uuid) -> AppResult<()> {
    sqlx::query("DELETE FROM dashboard_layouts WHERE user_id = $1 AND ledger_id = $2")
        .bind(user_id)
        .bind(ledger_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct LayoutForm {
    /// Comma-separated widget IDs. The browser default for an
    /// `<input name="widgets">` list is one value per line, so
    /// the picker POSTs a single comma-joined value. We split
    /// here so the route stays a plain HTML form with no JS.
    #[serde(default)]
    pub widgets: String,
}

/// POST /ledgers/{id}/dashboard/layout — replace the user's
/// widget ordering. `widgets` is a repeated form field.
pub async fn set_layout(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<LayoutForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let widgets: Vec<String> = form
        .widgets
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if widgets.is_empty() {
        return Err(AppError::Validation(
            "widgets must contain at least one id".into(),
        ));
    }
    save(&state.pool, user.id, ledger_id, &widgets).await?;
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

/// POST /ledgers/{id}/dashboard/reset — delete the row so
/// the dashboard falls back to [`DEFAULT_LAYOUT`].
pub async fn reset_layout(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    reset(&state.pool, user.id, ledger_id).await?;
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

/// Get the layout row directly (for tests + JSON export).
pub async fn row(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    ledger_id: Uuid,
) -> AppResult<Option<(Vec<String>, chrono::DateTime<chrono::Utc>)>> {
    let row: Option<(Vec<String>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT widgets, updated_at FROM dashboard_layouts
         WHERE user_id = $1 AND ledger_id = $2",
    )
    .bind(user_id)
    .bind(ledger_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Fetch budgets for the budget_burn widget. Returns one row
/// per active budget: `(account_name, period, amount, spent,
/// pct)`.
pub async fn budget_burn_rows(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
) -> AppResult<Vec<BudgetBurnRow>> {
    let rows = sqlx::query(
        r#"SELECT a.name AS account_name,
                  b.period, b.amount,
                  COALESCE(SUM(CASE WHEN p.direction='DEBIT' THEN p.amount ELSE 0 END), 0)
                - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0)
                  AS spent,
                  b.start_date, b.end_date
           FROM budgets b
           JOIN accounts a ON a.id = b.account_id
           LEFT JOIN postings p ON p.account_id = a.id
                AND p.created_at >= b.start_date
                AND p.created_at <= b.end_date
           WHERE b.ledger_id = $1
           GROUP BY a.name, b.period, b.amount, b.start_date, b.end_date
           ORDER BY b.start_date DESC, a.name"#,
    )
    .bind(ledger_id)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let account_name: String = row.try_get("account_name")?;
        let period: String = row.try_get("period")?;
        let amount: rust_decimal::Decimal = row.try_get("amount")?;
        let spent: rust_decimal::Decimal = row.try_get("spent")?;
        let pct = if amount.is_zero() {
            0.0
        } else {
            (spent / amount * rust_decimal::Decimal::from(100))
                .to_string()
                .parse::<f64>()
                .unwrap_or(0.0)
        };
        out.push(BudgetBurnRow {
            account_name,
            period,
            amount: amount.to_string(),
            spent: spent.to_string(),
            pct,
        });
    }
    Ok(out)
}

/// Fetch top-10 accounts by absolute balance for the
/// account_balances widget.
pub async fn account_balances_rows(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
) -> AppResult<Vec<AccountBalanceRow>> {
    let rows = sqlx::query_as::<_, (Uuid, String, String, rust_decimal::Decimal)>(
        r#"SELECT a.id, a.name, a.type,
                  COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
                - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0)
                  AS raw
           FROM accounts a
           LEFT JOIN postings p ON p.account_id = a.id
           WHERE a.ledger_id = $1
           GROUP BY a.id, a.name, a.type
           ORDER BY ABS(raw) DESC
           LIMIT 10"#,
    )
    .bind(ledger_id)
    .fetch_all(pool)
    .await?;
    let out = rows
        .into_iter()
        .map(|(id, name, ty, raw)| {
            // ASSET/EXPENSE: positive = debit balance.
            // LIABILITY/EQUITY/INCOME: positive = credit balance,
            // so flip the sign so the column reads as a normal
            // balance.
            let value = match ty.as_str() {
                "ASSET" | "EXPENSE" => raw,
                _ => -raw,
            };
            AccountBalanceRow {
                id,
                name,
                account_type: ty,
                balance: value.to_string(),
            }
        })
        .collect();
    Ok(out)
}

#[derive(Debug, serde::Serialize)]
pub struct BudgetBurnRow {
    pub account_name: String,
    pub period: String,
    pub amount: String,
    pub spent: String,
    pub pct: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct AccountBalanceRow {
    pub id: Uuid,
    pub name: String,
    pub account_type: String,
    pub balance: String,
}
