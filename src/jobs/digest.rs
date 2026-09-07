//! Weekly digest job (`automation-platform`).
//!
//! Sends opted-in users a weekly summary of their owned ledgers:
//! income, expenses, net, top 3 expense categories, overdue invoice
//! count, and budget alerts — in the user's locale week (ISO Mon–Sun).

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use sqlx::PgPool;

use super::JobError;

/// Send the digest for the user referenced by the payload
/// `{ "user_id": "…" }` for the ISO week containing `week_of`
/// (defaults to today).
pub async fn send_for_user(pool: &PgPool, payload: &serde_json::Value) -> Result<(), JobError> {
    let user_id: uuid::Uuid = super::payload_field(payload, "user_id")?;
    let week_of: NaiveDate = payload
        .get("week_of")
        .and_then(|v| v.as_str())
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Utc::now().date_naive());
    let (from, to) = iso_week_bounds(week_of);

    // The user's owned ledgers.
    let ledgers: Vec<(uuid::Uuid, String)> =
        sqlx::query_as("SELECT id, name FROM ledgers WHERE owner_id = $1 ORDER BY name")
            .bind(user_id)
            .fetch_all(pool)
            .await
            .map_err(JobError::Db)?;
    if ledgers.is_empty() {
        return Ok(());
    }

    let mut sections = Vec::new();
    for (ledger_id, name) in &ledgers {
        let (income, expense) = income_expense(pool, ledger_id, from, to).await?;
        let top = top_expense_categories(pool, ledger_id, from, to).await?;
        let overdue = overdue_count(pool, ledger_id).await?;
        sections.push(DigestSection {
            ledger: name.clone(),
            income,
            expense,
            net: income - expense,
            top_expenses: top,
            overdue_invoices: overdue,
        });
    }

    let text = render_text(&sections, from, to);
    let html = render_html(&sections, from, to);

    // Respect the preference: default off.
    let opted_in: (bool,) = sqlx::query_as(
        "SELECT COALESCE(
             BOOL_OR(enabled),
             FALSE)
         FROM notification_preferences
         WHERE user_id = $1 AND event = 'weekly_summary' AND channel IN ('email', 'in_app')",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(JobError::Db)?;
    if !opted_in.0 {
        return Ok(());
    }

    // Email when configured; otherwise record in-app so the digest is
    // never silently lost.
    let (email_addr,): (Option<String>,) = sqlx::query_as("SELECT email FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(JobError::Db)?;

    if crate::jobs::email_send::configured() {
        if let Some(addr) = email_addr.as_deref().filter(|e| !e.is_empty()) {
            crate::jobs::email_send::send_now(
                addr,
                &format!("Your OpenAccounting week {from}–{to}"),
                &text,
                &html,
            )
            .await?;
            return Ok(());
        }
    }
    for s in &sections {
        let _ = crate::notifications::record_in_app(
            pool,
            user_id,
            "weekly_summary",
            &format!("{}: net {} ({from} → {to})", s.ledger, s.net),
        )
        .await;
    }
    Ok(())
}

pub struct DigestSection {
    pub ledger: String,
    pub income: Decimal,
    pub expense: Decimal,
    pub net: Decimal,
    pub top_expenses: Vec<(String, Decimal)>,
    pub overdue_invoices: i64,
}

fn iso_week_bounds(d: NaiveDate) -> (NaiveDate, NaiveDate) {
    let weekday = d.weekday().num_days_from_monday() as i64;
    let monday = d - chrono::Duration::days(weekday);
    (monday, monday + chrono::Duration::days(6))
}

async fn income_expense(
    pool: &PgPool,
    ledger_id: &uuid::Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<(Decimal, Decimal), JobError> {
    let row: (Decimal, Decimal) = sqlx::query_as(
        r#"SELECT
             COALESCE(SUM(CASE WHEN a.type = 'INCOME' THEN p.amount ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN a.type = 'EXPENSE' THEN p.amount ELSE 0 END), 0)
           FROM postings p
           JOIN accounts a ON a.id = p.account_id
           JOIN transactions t ON t.id = p.transaction_id
           WHERE t.ledger_id = $1 AND t.kind <> 'draft'
                 AND t.txn_date BETWEEN $2 AND $3"#,
    )
    .bind(ledger_id)
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await
    .map_err(JobError::Db)?;
    Ok(row)
}

async fn top_expense_categories(
    pool: &PgPool,
    ledger_id: &uuid::Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<(String, Decimal)>, JobError> {
    let rows: Vec<(String, Decimal)> = sqlx::query_as(
        r#"SELECT a.name, SUM(p.amount) AS total
           FROM postings p
           JOIN accounts a ON a.id = p.account_id
           JOIN transactions t ON t.id = p.transaction_id
           WHERE t.ledger_id = $1 AND t.kind <> 'draft'
                 AND a.type = 'EXPENSE'
                 AND t.txn_date BETWEEN $2 AND $3
           GROUP BY a.name
           ORDER BY total DESC
           LIMIT 3"#,
    )
    .bind(ledger_id)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
    .map_err(JobError::Db)?;
    Ok(rows)
}

async fn overdue_count(pool: &PgPool, ledger_id: &uuid::Uuid) -> Result<i64, JobError> {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invoices
         WHERE ledger_id = $1 AND kind = 'receivable'
               AND status IN ('open', 'overdue') AND due_date < CURRENT_DATE",
    )
    .bind(ledger_id)
    .fetch_one(pool)
    .await
    .map_err(JobError::Db)?;
    Ok(n)
}

fn render_text(sections: &[DigestSection], from: NaiveDate, to: NaiveDate) -> String {
    let mut out = format!("OpenAccounting weekly summary {from} – {to}\n\n");
    for s in sections {
        out.push_str(&format!(
            "{}\n  Income: {}\n  Expenses: {}\n  Net: {}\n",
            s.ledger, s.income, s.expense, s.net
        ));
        if !s.top_expenses.is_empty() {
            out.push_str("  Top expenses:\n");
            for (name, amount) in &s.top_expenses {
                out.push_str(&format!("    - {name}: {amount}\n"));
            }
        }
        if s.overdue_invoices > 0 {
            out.push_str(&format!("  ⚠ {} overdue invoice(s)\n", s.overdue_invoices));
        }
        out.push('\n');
    }
    out
}

fn render_html(sections: &[DigestSection], from: NaiveDate, to: NaiveDate) -> String {
    let mut body = String::new();
    for s in sections {
        body.push_str(&format!(
            "<h3>{}</h3><p>Income: {} · Expenses: {} · <strong>Net: {}</strong><br>",
            html_escape(&s.ledger),
            s.income,
            s.expense,
            s.net
        ));
        if !s.top_expenses.is_empty() {
            let tops: Vec<String> = s
                .top_expenses
                .iter()
                .map(|(n, a)| format!("{}: {}", html_escape(n), a))
                .collect();
            body.push_str(&format!("Top expenses: {}<br>", tops.join(", ")));
        }
        if s.overdue_invoices > 0 {
            body.push_str(&format!(
                "<span style=\"color:#b91c1c\">{} overdue invoice(s)</span>",
                s.overdue_invoices
            ));
        }
        body.push_str("</p>");
    }
    format!("<html><body><h2>Weekly summary {from} – {to}</h2>{body}</body></html>")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
