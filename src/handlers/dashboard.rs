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
    let (ledger, _role) = ledgers::ensure_access(&state, user.id, ledger_id).await?;

    let today = chrono::Utc::now().date_naive();
    let first_of_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    let prev_month_first = previous_month(first_of_month);
    let prev_month_end = first_of_month.pred_opt().unwrap_or(prev_month_first);
    let _first_of_year = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap_or(today);

    // Income statement for the month-to-date and balance sheet at today.
    let is = build_income_statement(&state.pool, ledger_id, first_of_month, today).await?;
    let prev_is = build_income_statement(&state.pool, ledger_id, prev_month_first, prev_month_end).await?;
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

    // Top 5 expense categories for the current month
    let top_expenses: Vec<(String, Decimal)> = sqlx::query_as(
        r#"SELECT a.name, COALESCE(SUM(p.amount), 0) AS total
           FROM postings p
           JOIN transactions t ON t.id = p.transaction_id
           JOIN accounts a ON a.id = p.account_id
           WHERE t.ledger_id = $1 AND a.type = 'EXPENSE'
                 AND t.txn_date BETWEEN $2 AND $3
                 AND p.direction = 'DEBIT'
           GROUP BY a.name
           ORDER BY total DESC
           LIMIT 5"#,
    )
    .bind(ledger_id)
    .bind(first_of_month)
    .bind(today)
    .fetch_all(&state.pool)
    .await?;

    // Cash runway: bank balance / avg monthly expenses (last 3 months)
    let bank_balance: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(p.amount), 0) FROM postings p
           JOIN accounts a ON a.id = p.account_id
           JOIN transactions t ON t.id = p.transaction_id
           WHERE t.ledger_id = $1 AND a.type = 'ASSET' AND a.subtype = 'cash'
                 AND p.direction = 'DEBIT'"#,
    )
    .bind(ledger_id)
    .fetch_one(&state.pool)
    .await?;

    let three_months_ago = today - Duration::days(90);
    let avg_monthly_expense: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(p.amount), 0) / 3
           FROM postings p
           JOIN transactions t ON t.id = p.transaction_id
           JOIN accounts a ON a.id = p.account_id
           WHERE t.ledger_id = $1 AND a.type = 'EXPENSE'
                 AND t.txn_date >= $2
                 AND p.direction = 'DEBIT'"#,
    )
    .bind(ledger_id)
    .bind(three_months_ago)
    .fetch_one(&state.pool)
    .await?;

    let cash_runway = if avg_monthly_expense > Decimal::ZERO {
        decimal_to_f64(bank_balance) / decimal_to_f64(avg_monthly_expense)
    } else {
        0.0
    };
    let cash_runway_color = if cash_runway > 6.0 { "emerald" } else if cash_runway > 3.0 { "amber" } else { "rose" };

    // MoM calculations
    let revenue_mom_pct = pct_change(is.revenue.total, prev_is.revenue.total);
    let expense_mom_pct = pct_change(is.operating_expenses.total, prev_is.operating_expenses.total);
    let net_income_mom_pct = pct_change(is.net_income, prev_is.net_income);

    // AR/AP summary
    let (ar_outstanding, ar_overdue, ar_count, ar_overdue_count): (Decimal, Decimal, i64, i64) = sqlx::query_as(
        r#"SELECT
              COALESCE(SUM(total - COALESCE(amount_paid, 0)), 0) AS outstanding,
              COALESCE(SUM(CASE WHEN due_date < CURRENT_DATE THEN total - COALESCE(amount_paid, 0) ELSE 0 END), 0) AS overdue,
              COUNT(*) FILTER (WHERE status NOT IN ('paid', 'void')) AS open_count,
              COUNT(*) FILTER (WHERE due_date < CURRENT_DATE AND status NOT IN ('paid', 'void')) AS overdue_count
           FROM invoices WHERE ledger_id = $1 AND kind = 'receivable'"#,
    )
    .bind(ledger_id)
    .fetch_one(&state.pool)
    .await?;

    let (ap_outstanding, ap_upcoming, ap_count, ap_overdue_count): (Decimal, Decimal, i64, i64) = sqlx::query_as(
        r#"SELECT
              COALESCE(SUM(total - COALESCE(amount_paid, 0)), 0) AS outstanding,
              COALESCE(SUM(CASE WHEN due_date BETWEEN CURRENT_DATE AND CURRENT_DATE + INTERVAL '7 days' THEN total - COALESCE(amount_paid, 0) ELSE 0 END), 0) AS upcoming,
              COUNT(*) FILTER (WHERE status NOT IN ('paid', 'void')) AS open_count,
              COUNT(*) FILTER (WHERE due_date < CURRENT_DATE AND status NOT IN ('paid', 'void')) AS overdue_count
           FROM invoices WHERE ledger_id = $1 AND kind = 'payable'"#,
    )
    .bind(ledger_id)
    .fetch_one(&state.pool)
    .await?;

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
        cash_runway,
        cash_runway_color: cash_runway_color.to_string(),
        revenue_mom_pct,
        expense_mom_pct,
        net_income_mom_pct,
        top_expenses,
        ar_outstanding: format_money(ar_outstanding, &ledger.base_currency),
        ar_overdue: format_money(ar_overdue, &ledger.base_currency),
        ar_count,
        ar_overdue_count,
        ap_outstanding: format_money(ap_outstanding, &ledger.base_currency),
        ap_upcoming: format_money(ap_upcoming, &ledger.base_currency),
        ap_count,
        ap_overdue_count,
    }))
}

fn previous_month(d: NaiveDate) -> NaiveDate {
    let mut y = d.year();
    let mut m = d.month() as i32 - 1;
    if m <= 0 {
        m += 12;
        y -= 1;
    }
    NaiveDate::from_ymd_opt(y, m as u32, 1).unwrap_or(d)
}

fn pct_change(current: Decimal, previous: Decimal) -> f64 {
    let cur = decimal_to_f64(current);
    let prev = decimal_to_f64(previous);
    if prev == 0.0 {
        0.0
    } else {
        ((cur - prev) / prev) * 100.0
    }
}

pub async fn redirect_to_first_ledger(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    // If not logged in, send to login page (don't render 401)
    let user = match auth.user.as_ref() {
        Some(u) => u,
        None => {
            return Ok(Response::builder()
                .status(303)
                .header(header::LOCATION, "/login")
                .body(axum::body::Body::empty())
                .map_err(|e| AppError::Internal(e.to_string()))?);
        }
    };
    let first: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT id FROM ledgers
           WHERE owner_id = $1
               OR id IN (SELECT ledger_id FROM ledger_members WHERE user_id = $1)
           ORDER BY created_at ASC LIMIT 1"#,
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
