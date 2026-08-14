use crate::templates::render_response;
use axum::response::Response;
use axum::{
    extract::{Path, State},
    http::header,
};
use axum_login::AuthSession;
use chrono::{Datelike, NaiveDate};
use uuid::Uuid;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    reports::{
        build_balance_sheet, build_cash_flow, build_forecast, build_general_ledger,
        build_income_statement, build_trial_balance, ReportBasis,
    },
    templates::reports::{
        BalanceSheetPage, CashFlowForecastPage, CashFlowPage, GeneralLedgerPage,
        IncomeStatementPage, ReportsIndex, TrialBalancePage,
    },
    AppState,
};

/// Sanitize a value for use in HTTP header values.
/// Strips control characters and double-quotes to prevent header injection.
fn sanitize_header_value(s: &str) -> String {
    s.chars().filter(|c| !c.is_control() && *c != '"').collect()
}

pub async fn index(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    Ok(render_response(ReportsIndex {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
    }))
}

fn today() -> NaiveDate {
    chrono::Utc::now().date_naive()
}
fn first_of_year() -> NaiveDate {
    let t = today();
    NaiveDate::from_ymd_opt(t.year(), 1, 1).unwrap_or(t)
}
fn parse_or(s: Option<&String>, default: NaiveDate) -> NaiveDate {
    s.and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
        .unwrap_or(default)
}
fn validate_date_range(from: NaiveDate, to: NaiveDate) -> AppResult<()> {
    if from > to {
        return Err(AppError::Validation("from must be <= to".into()));
    }
    Ok(())
}

/// Parse `?basis=…` from the query string, falling back to the
/// ledger's stored `basis` column if the parameter is absent.
/// An explicit value that is not `accrual` or `cash` returns a
/// 400 with the list of valid options.
fn parse_basis(
    q: &std::collections::HashMap<String, String>,
    ledger: &crate::domain::Ledger,
) -> AppResult<ReportBasis> {
    match q.get("basis") {
        Some(v) => ReportBasis::parse(v),
        None => ReportBasis::parse(&ledger.basis),
    }
}

pub async fn trial_balance(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let as_of = parse_or(q.get("as_of"), today());
    let result = build_trial_balance(&state.pool, ledger_id, as_of).await?;
    let balanced = result.total_debit == result.total_credit;
    Ok(render_response(TrialBalancePage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        as_of,
        rows: result.rows,
        totals_debit: result.total_debit,
        totals_credit: result.total_credit,
        balanced,
    }))
}

pub async fn balance_sheet(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let as_of = parse_or(q.get("as_of"), today());
    let from = first_of_year();
    // The balance sheet's "Net income (current year)" line is
    // derived from the income statement, which is now
    // basis-aware. The balance sheet itself is basis-neutral
    // (it shows point-in-time balances), so we use accrual for
    // the net-income figure to match the equity semantics the
    // rest of the system already uses.
    let is =
        build_income_statement(&state.pool, ledger_id, from, as_of, ReportBasis::Accrual).await?;
    let bs = build_balance_sheet(&state.pool, ledger_id, as_of, is.net_income).await?;
    let balanced = bs.total_assets == bs.total_liab_equity;
    Ok(render_response(BalanceSheetPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        as_of,
        assets: bs.assets,
        liabilities: bs.liabilities,
        equity: bs.equity,
        net_income: bs.net_income,
        total_assets: bs.total_assets,
        total_liab_equity: bs.total_liab_equity,
        balanced,
    }))
}

pub async fn income_statement(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let from = parse_or(q.get("from"), first_of_year());
    let to = parse_or(q.get("to"), today());
    validate_date_range(from, to)?;
    let basis = parse_basis(&q, &ledger)?;
    let is = build_income_statement(&state.pool, ledger_id, from, to, basis).await?;
    Ok(render_response(IncomeStatementPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        from,
        to,
        basis,
        revenue: is.revenue,
        cost_of_goods_sold: is.cost_of_goods_sold,
        gross_profit: is.gross_profit,
        operating_expenses: is.operating_expenses,
        operating_income: is.operating_income,
        non_operating: is.non_operating,
        income_before_tax: is.income_before_tax,
        tax_expense: is.tax_expense,
        net_income: is.net_income,
        excluded: is.excluded,
    }))
}

pub async fn cash_flow(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let from = parse_or(q.get("from"), first_of_year());
    let to = parse_or(q.get("to"), today());
    validate_date_range(from, to)?;
    let basis = parse_basis(&q, &ledger)?;
    let cf = build_cash_flow(&state.pool, ledger_id, from, to, basis).await?;
    Ok(render_response(CashFlowPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        from,
        to,
        basis,
        opening: cf.opening,
        closing: cf.closing,
        movement: cf.movement,
        inflows: cf.inflows,
        outflows: cf.outflows,
        total_inflows: cf.total_inflows,
        total_outflows: cf.total_outflows,
    }))
}

pub async fn cash_flow_forecast(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let horizon_days: u32 = q
        .get("days")
        .and_then(|s| s.parse().ok())
        .unwrap_or(90)
        .clamp(1, 365);
    let result = build_forecast(&state.pool, ledger_id, horizon_days).await?;
    let chart_svg = render_forecast_chart(&result);
    Ok(render_response(CashFlowForecastPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        horizon_days,
        result,
        chart_svg,
    }))
}

pub async fn general_ledger(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let from = parse_or(q.get("from"), first_of_year());
    let to = parse_or(q.get("to"), today());
    validate_date_range(from, to)?;
    let account_filter: String = q.get("account_id").cloned().unwrap_or_default();
    let account_uuid = Uuid::parse_str(&account_filter).ok();
    let accounts = sqlx::query_as::<_, crate::domain::Account>(
        r#"SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
           FROM accounts WHERE ledger_id = $1 ORDER BY type, code, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    let (entries, running_balances) =
        build_general_ledger(&state.pool, ledger_id, from, to, account_uuid).await?;
    let account_filter_str = account_uuid.map(|u| u.to_string()).unwrap_or_default();
    Ok(render_response(GeneralLedgerPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        from,
        to,
        account_filter: account_filter_str,
        accounts,
        entries,
        running_balances,
    }))
}

pub async fn export_csv(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let from = parse_or(q.get("from"), first_of_year());
    let to = parse_or(q.get("to"), today());
    validate_date_range(from, to)?;
    let report = q
        .get("report")
        .map(|s| s.as_str())
        .unwrap_or("general_ledger");
    let mut wtr = csv::Writer::from_writer(vec![]);
    match report {
        "general_ledger" => {
            let (entries, _bal) =
                build_general_ledger(&state.pool, ledger_id, from, to, None).await?;
            wtr.write_record([
                "date",
                "description",
                "payee",
                "account",
                "direction",
                "amount",
                "memo",
            ])?;
            for e in entries {
                wtr.write_record([
                    e.txn_date.to_string(),
                    e.description,
                    e.payee.clone(),
                    e.account_name,
                    e.direction,
                    e.amount.to_string(),
                    e.memo.clone(),
                ])?;
            }
        }
        "trial_balance" => {
            let r = build_trial_balance(&state.pool, ledger_id, to).await?;
            wtr.write_record(["account", "type", "debit", "credit"])?;
            for row in r.rows {
                wtr.write_record([
                    row.account_name,
                    row.account_type,
                    row.debit.to_string(),
                    row.credit.to_string(),
                ])?;
            }
        }
        _ => return Err(AppError::Validation("unknown report".into())),
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let safe_name = sanitize_header_value(&ledger.name);
    let resp = Response::builder()
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}_{}_{}.csv\"",
                report, safe_name, to
            ),
        )
        .body(axum::body::Body::from(bytes))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(resp)
}

/// Render a hand-drawn SVG line chart for the forecast. Keeps
/// the file standalone (does not pull in `crate::charts`)
/// because the chart is small and we want zero allocation for
/// the axes — we just emit a single polyline.
fn render_forecast_chart(result: &crate::reports::ForecastResult) -> String {
    if result.points.is_empty() {
        return String::new();
    }
    let width = 720.0_f64;
    let height = 240.0_f64;
    let pad_l = 50.0;
    let pad_r = 16.0;
    let pad_t = 16.0;
    let pad_b = 30.0;
    let plot_w = width - pad_l - pad_r;
    let plot_h = height - pad_t - pad_b;

    let pts = &result.points;
    let n = pts.len();
    let (min_b, max_b) = pts
        .iter()
        .fold((result.today_balance, result.today_balance), |(lo, hi), p| {
            (lo.min(p.balance), hi.max(p.balance))
        });
    let span_f = {
        let lo = decimal_to_f64(min_b);
        let hi = decimal_to_f64(max_b);
        if hi > lo {
            hi - lo
        } else {
            1.0
        }
    };

    let mut poly = String::new();
    for (i, p) in pts.iter().enumerate() {
        let x = pad_l + plot_w * (i as f64 / (n as f64 - 1.0).max(1.0));
        let bal_f = decimal_to_f64(p.balance);
        let lo = decimal_to_f64(min_b);
        let y_norm = if span_f > 0.0 {
            ((bal_f - lo) / span_f).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let y = pad_t + plot_h * (1.0 - y_norm);
        if i > 0 {
            poly.push(' ');
        }
        poly.push_str(&format!("{x:.1},{y:.1}"));
    }
    let max_f = decimal_to_f64(max_b);
    let min_f = decimal_to_f64(min_b);
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.0} {h:.0}\" class=\"w-full h-auto\">\
  <rect x=\"0\" y=\"0\" width=\"{w:.0}\" height=\"{h:.0}\" fill=\"white\" />\
  <polyline fill=\"none\" stroke=\"#0f172a\" stroke-width=\"2\" points=\"{poly}\" />\
  <text x=\"6\" y=\"14\" font-size=\"10\" fill=\"#475569\">max {max_f:.2}</text>\
  <text x=\"6\" y=\"{hmax:.0}\" font-size=\"10\" fill=\"#475569\">min {min_f:.2}</text>\
</svg>",
        w = width,
        h = height,
        poly = poly,
        max_f = max_f,
        min_f = min_f,
        hmax = height as i32 - 4
    );
    svg
}

fn decimal_to_f64(d: rust_decimal::Decimal) -> f64 {
    d.to_string().parse::<f64>().unwrap_or(0.0)
}
