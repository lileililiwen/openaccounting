//! FX rate management, revaluation, and the FX gains report
//! (`multi-currency-fx`).

use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    domain::fx,
    error::{AppError, AppResult},
    handlers::{ledgers, reports},
    reports::fx_gains::build_fx_gains,
    templates::fx::FxRatesPage,
    templates::reports::FxGainsPage,
    AppState,
};

#[derive(Deserialize)]
pub struct NewRateForm {
    pub base_currency: String,
    pub quote_currency: String,
    pub rate: String,
    pub rate_date: String,
    /// Manual override reason (`accounting-dimensions` FX override
    /// audit): required, audited with actor/old/new.
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct RevaluationForm {
    pub date: String,
}

fn parse_currency(raw: &str) -> AppResult<String> {
    let c = raw.trim().to_ascii_uppercase();
    if c.len() == 3 && c.chars().all(|c| c.is_ascii_alphabetic()) {
        Ok(c)
    } else {
        Err(AppError::Validation(format!(
            "'{raw}' is not a 3-letter currency code"
        )))
    }
}

fn parse_date(raw: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::Validation(format!("invalid date '{raw}', expected YYYY-MM-DD")))
}

pub async fn rates_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let rates: Vec<(String, String, Decimal, NaiveDate, String)> = sqlx::query_as(
        r#"SELECT base_currency, quote_currency, rate, rate_date, source
           FROM fx_rates
           ORDER BY rate_date DESC, base_currency, quote_currency
           LIMIT 200"#,
    )
    .fetch_all(&state.pool)
    .await?;

    let overrides = sqlx::query_as::<_, crate::templates::fx::FxOverrideRow>(
        r#"SELECT o.base_currency, o.quote_currency, o.rate_date, o.old_rate, o.new_rate,
                  o.reason, COALESCE(u.username, '') AS actor_name
           FROM fx_override_audit o
           LEFT JOIN users u ON u.id = o.actor_id
           WHERE o.ledger_id = $1
           ORDER BY o.created_at DESC
           LIMIT 100"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(FxRatesPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name.clone(),
        current_section: "transactions".to_string(),
        base_currency: ledger.base_currency.clone(),
        rates,
        overrides,
        error: String::new(),
    }))
}

/// Manual rate entry (ledger owner only — manual beats the feed).
pub async fn create_rate(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewRateForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let base = parse_currency(&form.base_currency)?;
    let quote = parse_currency(&form.quote_currency)?;
    if base == quote {
        return Err(AppError::Validation("currency pair must differ".into()));
    }
    let rate: Decimal = form
        .rate
        .trim()
        .replace(',', ".")
        .parse()
        .map_err(|_| AppError::Validation("invalid rate".into()))?;
    if rate <= Decimal::ZERO {
        return Err(AppError::Validation("rate must be positive".into()));
    }
    let date = parse_date(&form.rate_date)?;

    // Override audit gate: manual entries require a reason.
    let reason = form.reason.as_deref().map(str::trim).unwrap_or_default();
    if reason.is_empty() {
        return Err(AppError::Validation(
            "a reason is required for a manual FX rate (override audit)".into(),
        ));
    }

    let old_rate: Option<Decimal> = sqlx::query_scalar(
        "SELECT rate FROM fx_rates WHERE base_currency = $1 AND quote_currency = $2 AND rate_date = $3",
    )
    .bind(&base)
    .bind(&quote)
    .bind(date)
    .fetch_optional(&state.pool)
    .await?
    .flatten();

    sqlx::query(
        r#"INSERT INTO fx_rates (base_currency, quote_currency, rate, rate_date, source)
           VALUES ($1, $2, $3, $4, 'manual')
           ON CONFLICT (base_currency, quote_currency, rate_date)
           DO UPDATE SET rate = EXCLUDED.rate, source = 'manual'"#,
    )
    .bind(&base)
    .bind(&quote)
    .bind(rate)
    .bind(date)
    .execute(&state.pool)
    .await?;

    sqlx::query(
        r#"INSERT INTO fx_override_audit
           (ledger_id, actor_id, base_currency, quote_currency, rate_date, old_rate, new_rate, reason)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    )
    .bind(ledger_id)
    .bind(user.id)
    .bind(&base)
    .bind(&quote)
    .bind(date)
    .bind(old_rate)
    .bind(rate)
    .bind(reason)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "fx_rate",
        None,
        None,
        Some(serde_json::json!({
            "pair": format!("{base}/{quote}"),
            "rate": rate.to_string(),
            "date": date.to_string(),
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/fx-rates")).into_response())
}

/// Month-end revaluation (`POST /ledgers/{id}/fx-revaluation`).
pub async fn revaluate(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<RevaluationForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let date = parse_date(&form.date)?;

    match fx::run_revaluation(&state.pool, ledger_id, date, user.id).await {
        Ok(posted) => {
            if posted.is_empty() {
                // Eligible accounts existed but every difference was zero.
            }
            let _ = audit::log(
                &state.pool,
                Some(ledger_id),
                user.id,
                "revalue",
                "ledger",
                Some(ledger_id),
                None,
                Some(serde_json::json!({
                    "date": date.to_string(),
                    "accounts": posted.iter().map(|p| p.currency.clone()).collect::<Vec<_>>(),
                })),
            )
            .await;
            Ok(Redirect::to(&format!("/ledgers/{ledger_id}/reports/fx-gains")).into_response())
        }
        Err(fx::RevaluationError::AlreadyRevalued { month }) => Err(AppError::Conflict(format!(
            "FX revaluation for {month} has already been run for every eligible account"
        ))),
        Err(fx::RevaluationError::Fx(e)) => Err(AppError::Validation(e.to_string())),
        Err(fx::RevaluationError::Posting(
            crate::domain::posting_service::PostingServiceError::PeriodClosed { year, .. },
        )) => Err(AppError::Validation(format!(
            "Period {year} is closed. Cannot post revaluations into closed periods."
        ))),
        Err(fx::RevaluationError::Posting(
            crate::domain::posting_service::PostingServiceError::HardClosed { closed_through, .. },
        )) => Err(AppError::Conflict(format!(
            "Ledger is closed through {closed_through}. Cannot post revaluations into closed periods."
        ))),
        Err(fx::RevaluationError::Posting(e)) => Err(AppError::Validation(e.to_string())),
        Err(fx::RevaluationError::Db(e)) => Err(AppError::Db(e)),
    }
}

pub async fn fx_gains_report(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let from_str = params.get("from").cloned().unwrap_or_default();
    let to_str = params.get("to").cloned().unwrap_or_default();
    let from = NaiveDate::parse_from_str(&from_str, "%Y-%m-%d").ok();
    let to = NaiveDate::parse_from_str(&to_str, "%Y-%m-%d").ok();
    if let (Some(f), Some(t)) = (from, to) {
        if f > t {
            return Err(AppError::Validation("from must be <= to".into()));
        }
    }

    let report = build_fx_gains(&state.pool, ledger_id, from, to).await?;
    let years = reports::closed_years(&state.pool, ledger_id).await?;
    let closed_notice = reports::closed_notice(&years, |y| {
        let in_range = |d: Option<NaiveDate>| d.map_or(false, |d| d.year() == y);
        in_range(from) || in_range(to)
    });

    Ok(render_response(FxGainsPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name.clone(),
        current_section: "reports".to_string(),
        from: from_str,
        to: to_str,
        report,
        closed_notice,
    }))
}

pub async fn fx_gains_export_csv(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let from = params
        .get("from")
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let to = params
        .get("to")
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let report = build_fx_gains(&state.pool, ledger_id, from, to).await?;

    let mut csv = String::from("section,date,account,currency,gain_loss\n");
    for r in &report.unrealized {
        csv.push_str(&format!(
            "unrealized,{},{},{},{}\n",
            r.period_month.format("%Y-%m-%d"),
            csv_escape(&r.account_name),
            r.currency,
            r.fx_gain_loss
        ));
    }
    for r in &report.realized {
        let net = r.gain - r.loss;
        csv.push_str(&format!(
            "realized,{},{},USD,{}\n",
            r.txn_date.format("%Y-%m-%d"),
            csv_escape(&r.description),
            net
        ));
    }

    Ok(Response::builder()
        .status(200)
        .header("content-type", "text/csv; charset=utf-8")
        .header(
            "content-disposition",
            "attachment; filename=fx-gains-export.csv",
        )
        .body(axum::body::Body::from(csv))
        .map_err(|e| AppError::Internal(e.to_string()))?)
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
