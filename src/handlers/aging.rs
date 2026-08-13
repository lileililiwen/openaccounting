use axum::extract::{Path, State};
use axum::response::Response;
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::reports::AgingReportPage,
    AppState,
};

pub async fn ar_aging(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let today = chrono::Utc::now().date_naive();
    let aging = compute_aging(&state.pool, ledger_id, "receivable", today).await?;

    Ok(crate::templates::render_response(AgingReportPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: _ledger.name,
        report_type: "Accounts Receivable".into(),
        as_of: today,
        aging,
    }))
}

pub async fn ap_aging(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let today = chrono::Utc::now().date_naive();
    let aging = compute_aging(&state.pool, ledger_id, "payable", today).await?;

    Ok(crate::templates::render_response(AgingReportPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: _ledger.name,
        report_type: "Accounts Payable".into(),
        as_of: today,
        aging,
    }))
}

#[derive(Clone, Debug)]
pub struct AgingBucketData {
    pub contact_name: String,
    pub current: Decimal,
    pub days_1_30: Decimal,
    pub days_31_60: Decimal,
    pub days_61_90: Decimal,
    pub days_90_plus: Decimal,
    pub total: Decimal,
}

async fn compute_aging(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
    kind: &str,
    today: NaiveDate,
) -> AppResult<Vec<AgingBucketData>> {
    let invoices = sqlx::query_as::<_, (Uuid, String, NaiveDate, Decimal)>(
        r#"SELECT contact_id, COALESCE((SELECT name FROM contacts WHERE id = i.contact_id), 'Unknown') AS contact_name, due_date, total - amount_paid AS outstanding
           FROM invoices i
           WHERE ledger_id = $1 AND kind = $2 AND status != 'void' AND total - amount_paid > 0"#,
    )
    .bind(ledger_id)
    .bind(kind)
    .fetch_all(pool)
    .await?;

    // Group by contact
    let mut by_contact: std::collections::HashMap<Uuid, AgingBucketData> = std::collections::HashMap::new();

    for (contact_id, contact_name, due_date, outstanding) in invoices {
        let days_overdue = (today - due_date).num_days();

        let bucket = by_contact.entry(contact_id).or_insert_with(|| AgingBucketData {
            contact_name,
            current: Decimal::ZERO,
            days_1_30: Decimal::ZERO,
            days_31_60: Decimal::ZERO,
            days_61_90: Decimal::ZERO,
            days_90_plus: Decimal::ZERO,
            total: Decimal::ZERO,
        });

        if days_overdue <= 0 {
            bucket.current += outstanding;
        } else if days_overdue <= 30 {
            bucket.days_1_30 += outstanding;
        } else if days_overdue <= 60 {
            bucket.days_31_60 += outstanding;
        } else if days_overdue <= 90 {
            bucket.days_61_90 += outstanding;
        } else {
            bucket.days_90_plus += outstanding;
        }
        bucket.total += outstanding;
    }

    let mut result: Vec<AgingBucketData> = by_contact.into_values().collect();
    result.sort_by(|a, b| b.total.cmp(&a.total));

    Ok(result)
}
