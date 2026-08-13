use crate::templates::render_response;
use axum::response::Response;
use axum::{
    extract::{Path, State},
    http::header,
};
use axum_login::AuthSession;
use chrono::{Datelike, Duration, NaiveDate};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{
    auth::Backend,
    charts::{render_donut, render_line, DonutSegment, LineSeries},
    error::{AppError, AppResult},
    handlers::ledgers,
    reports::{build_balance_sheet, build_income_statement},
    templates::{dashboard::DashboardPage, transactions::TransactionRow},
    AppState,
};

pub async fn show(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let today = chrono::Utc::now().date_naive();
    let first_of_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    let _first_of_year = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap_or(today);

    // Income statement for the month-to-date and balance sheet at today.
    let is = build_income_statement(&state.pool, ledger_id, first_of_month, today).await?;
    let bs = build_balance_sheet(&state.pool, ledger_id, today, is.net_income).await?;
    let (txn_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE ledger_id = $1")
            .bind(ledger_id)
            .fetch_one(&state.pool)
            .await?;

    let recent: Vec<TransactionRow> = sqlx::query_as::<_, TransactionRow>(
        r#"
        SELECT t.id, t.txn_date AS date, t.description, t.payee, t.currency,
               COALESCE((SELECT SUM(p.amount) FROM postings p WHERE p.transaction_id=t.id AND p.direction='DEBIT'), 0) AS total,
               (SELECT COUNT(*) FROM documents d WHERE d.transaction_id = t.id) AS doc_count,
               COALESCE((SELECT array_agg(tg.name)
                         FROM transaction_tags tt
                         JOIN tags tg ON tg.id = tt.tag_id
                         WHERE tt.transaction_id = t.id), ARRAY[]::text[]) AS tags
        FROM transactions t
        WHERE t.ledger_id = $1
        ORDER BY t.txn_date DESC, t.created_at DESC
        LIMIT 10
        "#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    // Last 6 months income vs expense (for line chart).
    let months = last_n_months(today, 6);
    let mut x_labels: Vec<String> = Vec::new();
    let mut income_series = LineSeries {
        name: "Income".into(),
        color: "#0ea5e9".into(),
        values: vec![],
    };
    let mut expense_series = LineSeries {
        name: "Expense".into(),
        color: "#f43f5e".into(),
        values: vec![],
    };
    for m_start in &months {
        let m_end = next_month(*m_start).pred_opt().unwrap_or(*m_start);
        x_labels.push(m_start.format("%b %Y").to_string());
        let r = build_income_statement(&state.pool, ledger_id, *m_start, m_end).await?;
        income_series.values.push(decimal_to_f64(r.revenue.total));
        expense_series.values.push(decimal_to_f64(r.operating_expenses.total));
    }
    let income_expense_svg = render_line(640, 220, x_labels, vec![income_series, expense_series]);

    // Expense breakdown last 30 days (donut).
    let from = today - Duration::days(30);
    let is_30 = build_income_statement(&state.pool, ledger_id, from, today).await?;
    let palette = [
        "#0ea5e9", "#f43f5e", "#10b981", "#f59e0b", "#8b5cf6", "#ec4899", "#14b8a6", "#f97316",
    ];
    let mut segments: Vec<DonutSegment> = Vec::new();
    for (i, e) in is_30.operating_expenses.accounts.iter().take(8).enumerate() {
        if e.amount > Decimal::ZERO {
            segments.push(DonutSegment {
                label: e.account_name.clone(),
                value: decimal_to_f64(e.amount),
                color: palette[i % palette.len()].to_string(),
            });
        }
    }
    let total_exp_30: f64 = segments.iter().map(|s| s.value).sum();
    let center_label = if total_exp_30 > 0.0 {
        format_expense(total_exp_30)
    } else {
        "—".into()
    };
    let expense_breakdown_svg = render_donut(220, segments, &center_label);

    Ok(render_response(DashboardPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        currency: ledger.base_currency.clone(),
        total_assets: format_money(bs.total_assets, &ledger.base_currency),
        total_liabilities: format_money(bs.total_liabilities, &ledger.base_currency),
        net_worth: format_money(
            bs.total_assets - bs.total_liabilities,
            &ledger.base_currency,
        ),
        month_income: format_money(is.revenue.total, &ledger.base_currency),
        month_expense: format_money(is.operating_expenses.total, &ledger.base_currency),
        month_net: format_money(is.net_income, &ledger.base_currency),
        txn_count,
        recent_transactions: recent,
        income_expense_svg,
        expense_breakdown_svg,
    }))
}

pub async fn redirect_to_first_ledger(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let first: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM ledgers WHERE owner_id = $1 ORDER BY created_at ASC LIMIT 1",
    )
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?;
    let dest = match first {
        Some((id,)) => format!("/ledgers/{}/dashboard", id),
        None => "/ledgers".to_string(),
    };
    Ok(Response::builder()
        .status(303)
        .header(header::LOCATION, dest)
        .body(axum::body::Body::empty())
        .map_err(|e| AppError::Internal(e.to_string()))?)
}

fn last_n_months(today: NaiveDate, n: usize) -> Vec<NaiveDate> {
    let mut out = Vec::with_capacity(n);
    let mut y = today.year();
    let mut m = today.month() as i32;
    for _ in 0..n {
        m -= 1;
        if m <= 0 {
            m += 12;
            y -= 1;
        }
        if let Some(d) = NaiveDate::from_ymd_opt(y, m as u32, 1) {
            out.push(d);
        }
    }
    out.reverse();
    out
}

fn next_month(d: NaiveDate) -> NaiveDate {
    let mut y = d.year();
    let mut m = d.month() as i32 + 1;
    if m > 12 {
        m = 1;
        y += 1;
    }
    NaiveDate::from_ymd_opt(y, m as u32, 1).unwrap_or(d)
}

fn decimal_to_f64(d: Decimal) -> f64 {
    use std::str::FromStr;
    f64::from_str(&d.to_string()).unwrap_or(0.0)
}

fn format_money(d: Decimal, currency: &str) -> String {
    let sign = if d < Decimal::ZERO { "-" } else { "" };
    let abs = d.abs();
    let s = format!("{:.2}", abs);
    // Insert thousands separators
    let mut out = String::new();
    let (int_part, frac_part) = s.split_once('.').unwrap_or((&s, "00"));
    for (i, ch) in int_part.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    let int_with_commas: String = out.chars().rev().collect();
    format!("{}{} {}.{}", sign, currency, int_with_commas, frac_part)
}

fn format_expense(v: f64) -> String {
    if v >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
        format!("{:.1}k", v / 1_000.0)
    } else {
        format!("{:.0}", v)
    }
}
