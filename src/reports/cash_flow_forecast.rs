//! Cash-flow forecasting.
//!
//! Combines today's cash balance with materialized recurring
//! transactions (`transaction_templates` rows that are active
//! and have postings) to project the daily balance over the
//! next N days.
//!
//! Deterministic: no Monte Carlo, no AI suggestions, no AR/AP
//! aging as input. Only what the user has explicitly set up as
//! a recurring template is projected forward.

use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct TemplateRow {
    pub id: Uuid,
    pub description: String,
    pub payee: Option<String>,
    pub frequency: String,
    pub next_date: NaiveDate,
    pub amount: Decimal,
    /// "DEBIT" or "CREDIT" — the side of the cash account.
    /// Templates with multiple postings would need a different
    /// schema; we sum the debit amount minus the credit amount
    /// of the cash leg, which is the projection of the cash
    /// balance change.
    pub cash_direction: String,
}

/// A single projected future entry on a specific date.
#[derive(Clone, Debug)]
pub struct ForecastEntry {
    pub date: NaiveDate,
    pub description: String,
    pub payee: Option<String>,
    pub amount: Decimal, // signed: negative = cash out
}

/// A single point on the daily balance line.
#[derive(Clone, Debug)]
pub struct ForecastPoint {
    pub date: NaiveDate,
    pub balance: Decimal,
}

#[derive(Clone, Debug)]
pub struct ForecastResult {
    pub today_balance: Decimal,
    pub horizon_days: u32,
    pub entries: Vec<ForecastEntry>,
    pub points: Vec<ForecastPoint>,
    pub min_balance: Decimal,
    pub min_balance_date: NaiveDate,
    pub max_balance: Decimal,
    pub max_balance_date: NaiveDate,
    pub ending_balance: Decimal,
}

/// Advance a date by `n` periods of the given frequency.
/// Month-end semantics: if the source date is the 31st, the
/// next monthly occurrence is the last day of the next month.
pub fn advance(date: NaiveDate, frequency: &str, n: u32) -> NaiveDate {
    let mut d = date;
    for _ in 0..n {
        d = match frequency {
            "weekly" => d + Duration::days(7),
            "biweekly" => d + Duration::days(14),
            "monthly" => next_month(d),
            "quarterly" => next_month_n(d, 3),
            "yearly" => next_year(d),
            // Unknown frequency: leave the date alone so the
            // row is projected once at its `next_date` and not
            // again. Better than crashing the report.
            _ => d,
        };
    }
    d
}

fn next_month(d: NaiveDate) -> NaiveDate {
    next_month_n(d, 1)
}

fn next_month_n(d: NaiveDate, n: i64) -> NaiveDate {
    let mut year = d.year();
    let mut month = d.month() as i32 + n as i32;
    while month > 12 {
        month -= 12;
        year += 1;
    }
    while month < 1 {
        month += 12;
        year -= 1;
    }
    let target_day = d.day();
    // Clamp the day to the last day of the target month.
    let last_day = last_day_of_month(year, month as u32);
    let day = target_day.min(last_day);
    NaiveDate::from_ymd_opt(year, month as u32, day).unwrap_or(d)
}

fn next_year(d: NaiveDate) -> NaiveDate {
    next_month_n(d, 12)
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    // First day of the NEXT month minus 1 day.
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .unwrap()
        .pred_opt()
        .unwrap()
        .day()
}

use chrono::Datelike;

/// Materialize the next N occurrences of every active template,
/// capped by `horizon`. Returns the entries sorted by date.
pub fn materialize_forecast(
    templates: &[TemplateRow],
    today: NaiveDate,
    horizon_days: u32,
) -> Vec<ForecastEntry> {
    let horizon = today + Duration::days(horizon_days as i64);
    let mut out = Vec::new();
    for t in templates {
        if !is_recognised_frequency(&t.frequency) {
            continue;
        }
        let mut d = t.next_date;
        // Safety cap: at most one entry per template per day,
        // and at most 366 entries (max horizon is 365 days, so
        // weekly = ~53, daily would be 365).
        let mut guard = 0u32;
        while d <= horizon && guard < 400 {
            if d >= today {
                let signed = match t.cash_direction.as_str() {
                    "DEBIT" => t.amount,   // cash out: subtract from balance
                    "CREDIT" => -t.amount, // cash in: add to balance
                    _ => t.amount,
                };
                out.push(ForecastEntry {
                    date: d,
                    description: t.description.clone(),
                    payee: t.payee.clone(),
                    amount: signed,
                });
            }
            d = advance(d, &t.frequency, 1);
            guard += 1;
        }
    }
    out.sort_by_key(|e| e.date);
    out
}

fn is_recognised_frequency(f: &str) -> bool {
    matches!(
        f,
        "weekly" | "biweekly" | "monthly" | "quarterly" | "yearly"
    )
}

/// Walk day-by-day from today through the horizon, applying
/// each entry on its date, and return the daily balance line.
pub fn project(
    today: NaiveDate,
    today_balance: Decimal,
    entries: &[ForecastEntry],
    horizon_days: u32,
) -> Vec<ForecastPoint> {
    let mut points = Vec::with_capacity(horizon_days as usize + 1);
    let mut balance = today_balance;
    let mut running = today;
    // Index into entries sorted by date.
    let mut idx = 0usize;
    for day_offset in 0..=horizon_days {
        let d = today + Duration::days(day_offset as i64);
        while idx < entries.len() && entries[idx].date == d {
            balance += entries[idx].amount;
            idx += 1;
        }
        points.push(ForecastPoint { date: d, balance });
        running = d;
    }
    let _ = running; // suppress unused warning
    points
}

/// Load the active recurring templates for a ledger, joined to
/// the postings of the cash account leg (the leg whose account
/// is in the cash / bank set). If the template has postings on
/// non-cash accounts, we project only the net cash impact (the
/// single leg whose account name matches the cash heuristic).
pub async fn load_active_templates(pool: &PgPool, ledger_id: Uuid) -> AppResult<Vec<TemplateRow>> {
    let rows: Vec<TemplateRow> = sqlx::query_as(
        r#"
        SELECT t.id, t.description, t.payee, t.frequency, t.next_date,
               tp.amount,
               tp.direction AS cash_direction
        FROM transaction_templates t
        JOIN template_postings tp ON tp.template_id = t.id
        JOIN accounts a ON a.id = tp.account_id
        WHERE t.ledger_id = $1
          AND t.is_active = TRUE
          AND (LOWER(a.name) LIKE '%cash%' OR LOWER(a.name) LIKE '%bank%')
        "#,
    )
    .bind(ledger_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Sum the current balance across cash and bank accounts.
pub async fn today_cash_balance(pool: &PgPool, ledger_id: Uuid) -> AppResult<Decimal> {
    let (bal,): (Decimal,) = sqlx::query_as(
        r#"
        SELECT COALESCE(SUM(
            CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END
          - CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END
        ), 0)
        FROM postings p
        JOIN transactions t ON t.id = p.transaction_id
        JOIN accounts a ON a.id = p.account_id
        WHERE t.ledger_id = $1
          AND a.ledger_id = $1
          AND (LOWER(a.name) LIKE '%cash%' OR LOWER(a.name) LIKE '%bank%')
        "#,
    )
    .bind(ledger_id)
    .fetch_one(pool)
    .await?;
    Ok(bal)
}

pub async fn build_forecast(
    pool: &PgPool,
    ledger_id: Uuid,
    horizon_days: u32,
) -> AppResult<ForecastResult> {
    let today = chrono::Utc::now().date_naive();
    let today_balance = today_cash_balance(pool, ledger_id).await?;
    let templates = load_active_templates(pool, ledger_id).await?;
    let entries = materialize_forecast(&templates, today, horizon_days);
    let points = project(today, today_balance, &entries, horizon_days);

    let min_point = points
        .iter()
        .min_by_key(|p| p.balance)
        .cloned()
        .unwrap_or(ForecastPoint {
            date: today,
            balance: today_balance,
        });
    let max_point = points
        .iter()
        .max_by_key(|p| p.balance)
        .cloned()
        .unwrap_or(ForecastPoint {
            date: today,
            balance: today_balance,
        });
    let ending_balance = points.last().map(|p| p.balance).unwrap_or(today_balance);

    Ok(ForecastResult {
        today_balance,
        horizon_days,
        entries,
        points,
        min_balance: min_point.balance,
        min_balance_date: min_point.date,
        max_balance: max_point.balance,
        max_balance_date: max_point.date,
        ending_balance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn tpl(
        id: u128,
        desc: &str,
        freq: &str,
        next: NaiveDate,
        amt: Decimal,
        dir: &str,
    ) -> TemplateRow {
        TemplateRow {
            id: Uuid::from_u128(id),
            description: desc.to_string(),
            payee: None,
            frequency: freq.to_string(),
            next_date: next,
            amount: amt,
            cash_direction: dir.to_string(),
        }
    }

    #[test]
    fn advance_monthly_eom_clamp() {
        assert_eq!(
            advance(NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(), "monthly", 1),
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        );
        assert_eq!(
            advance(NaiveDate::from_ymd_opt(2026, 2, 28).unwrap(), "monthly", 1),
            NaiveDate::from_ymd_opt(2026, 3, 28).unwrap()
        );
        assert_eq!(
            advance(NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(), "monthly", 1),
            NaiveDate::from_ymd_opt(2026, 9, 14).unwrap()
        );
    }

    #[test]
    fn advance_weekly() {
        assert_eq!(
            advance(NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(), "weekly", 1),
            NaiveDate::from_ymd_opt(2026, 8, 21).unwrap()
        );
    }

    #[test]
    fn materialize_monthly_30_days() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let templates = vec![tpl(
            1,
            "Rent",
            "monthly",
            NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            dec!(100),
            "DEBIT",
        )];
        let entries = materialize_forecast(&templates, today, 30);
        // 1/15 falls within [1/1, 1/31].
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].date,
            NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()
        );
        assert_eq!(entries[0].amount, dec!(100));
    }

    #[test]
    fn project_running_balance() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let entries = vec![ForecastEntry {
            date: NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
            description: "Rent".into(),
            payee: None,
            amount: dec!(-300),
        }];
        let pts = project(today, dec!(1000), &entries, 7);
        assert_eq!(pts.len(), 8);
        // Days 1-4: 1000
        for p in &pts[0..4] {
            assert_eq!(p.balance, dec!(1000));
        }
        // Day 5: 700
        assert_eq!(pts[4].balance, dec!(700));
        // Day 6 onwards: still 700
        assert_eq!(pts[5].balance, dec!(700));
        assert_eq!(pts[7].balance, dec!(700));
    }
}
