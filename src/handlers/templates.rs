use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::templates::{TemplateList, TemplateNew, TemplateRow, TemplateShow},
    AppState,
};

#[derive(Deserialize)]
pub struct NewTemplateForm {
    pub description: String,
    pub payee: Option<String>,
    pub reference: Option<String>,
    pub frequency: String,
    pub next_date: String,
    pub account_id: Vec<String>,
    pub direction: Vec<String>,
    pub amount: Vec<String>,
    pub memo: Vec<String>,
}

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let rows = sqlx::query_as::<_, TemplateRow>(
        r#"SELECT t.id, t.description, COALESCE(t.payee, '') AS payee, t.frequency,
                  t.next_date, t.is_active,
                  (SELECT COUNT(*) FROM template_postings tp WHERE tp.template_id = t.id) AS posting_count
           FROM transaction_templates t
           WHERE t.ledger_id = $1
           ORDER BY t.is_active DESC, t.next_date"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(TemplateList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        templates: rows,
        error: String::new(),
    }))
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let accounts = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"SELECT id, code, name FROM accounts WHERE ledger_id = $1 AND is_archived = FALSE
           ORDER BY type, code NULLS LAST, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(TemplateNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        accounts,
        description: String::new(),
        payee: String::new(),
        reference: String::new(),
        frequency: "monthly".to_string(),
        next_date: chrono::Utc::now().date_naive().to_string(),
        error: String::new(),
    }))
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewTemplateForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let description = form.description.trim();
    if description.is_empty() {
        return Err(AppError::Validation("Description is required".into()));
    }

    let frequency = form.frequency.clone();
    if !["weekly", "biweekly", "monthly", "quarterly", "yearly"].contains(&frequency.as_str()) {
        return Err(AppError::Validation("Invalid frequency".into()));
    }

    let next_date = NaiveDate::parse_from_str(&form.next_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid date".into()))?;

    if form.account_id.len() < 2 {
        return Err(AppError::Validation(
            "At least two posting lines required".into(),
        ));
    }

    let mut tx = state.pool.begin().await?;

    let template_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO transaction_templates (ledger_id, description, payee, reference, frequency, next_date)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(description)
    .bind(form.payee.as_deref().filter(|p| !p.is_empty()))
    .bind(form.reference.as_deref().filter(|p| !p.is_empty()))
    .bind(&frequency)
    .bind(next_date)
    .fetch_one(&mut *tx)
    .await?;

    for i in 0..form.account_id.len() {
        let account_id = Uuid::parse_str(&form.account_id[i])
            .map_err(|_| AppError::Validation("Invalid account".into()))?;
        let direction = form.direction[i].clone();
        let amount: Decimal = form.amount[i]
            .parse()
            .map_err(|_| AppError::Validation("Invalid amount".into()))?;
        let memo = form.memo.get(i).cloned().unwrap_or_default();
        let memo_filtered = if memo.is_empty() { None } else { Some(memo) };

        sqlx::query(
            r#"INSERT INTO template_postings (template_id, account_id, direction, amount, memo)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(template_id)
        .bind(account_id)
        .bind(&direction)
        .bind(amount)
        .bind(memo_filtered)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "template",
        Some(template_id),
        None,
        Some(serde_json::json!({
            "description": description,
            "frequency": frequency,
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/templates", ledger_id)).into_response())
}

pub async fn show(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, template_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let template = sqlx::query_as::<_, TemplateRow>(
        r#"SELECT id, description, COALESCE(payee, '') AS payee, frequency,
                  next_date, is_active,
                  (SELECT COUNT(*) FROM template_postings tp WHERE tp.template_id = transaction_templates.id) AS posting_count
           FROM transaction_templates WHERE ledger_id = $1 AND id = $2"#,
    )
    .bind(ledger_id)
    .bind(template_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let postings = sqlx::query_as::<_, (Uuid, String, String, String, Decimal, Option<String>)>(
        r#"SELECT tp.account_id, a.code, COALESCE(a.code,'') || ' ' || a.name AS account_name,
                  tp.direction, tp.amount, tp.memo
           FROM template_postings tp
           JOIN accounts a ON a.id = tp.account_id
           WHERE tp.template_id = $1
           ORDER BY tp.direction DESC, a.code NULLS LAST, a.name"#,
    )
    .bind(template_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(TemplateShow {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: _ledger.name,
        current_section: "transactions".to_string(),
        template,
        postings,
    }))
}

pub async fn toggle(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, template_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    sqlx::query("UPDATE transaction_templates SET is_active = NOT is_active, updated_at = now() WHERE id = $1")
        .bind(template_id)
        .execute(&state.pool)
        .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "toggle",
        "template",
        Some(template_id),
        None,
        None,
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/templates", ledger_id)).into_response())
}

pub async fn delete(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, template_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    sqlx::query("DELETE FROM transaction_templates WHERE id = $1")
        .bind(template_id)
        .execute(&state.pool)
        .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "delete",
        "template",
        Some(template_id),
        None,
        None,
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/templates", ledger_id)).into_response())
}

pub async fn run(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, template_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let template = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            String,
            NaiveDate,
            bool,
        ),
    >(
        r#"SELECT description, payee, reference, frequency, next_date, is_active
           FROM transaction_templates WHERE id = $1"#,
    )
    .bind(template_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    if !template.5 {
        return Err(AppError::Validation("Template is inactive".into()));
    }

    let postings = sqlx::query_as::<_, (Uuid, String, Decimal, Option<String>)>(
        r#"SELECT account_id, direction, amount, memo FROM template_postings WHERE template_id = $1"#,
    )
    .bind(template_id)
    .fetch_all(&state.pool)
    .await?;

    let next_date = template.4;
    run_template(&state, ledger_id, &template, &postings, next_date, user.id).await?;

    Ok(Redirect::to(&format!("/ledgers/{}/templates", ledger_id)).into_response())
}

pub async fn process_due(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let _user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    let due = sqlx::query_as::<
        _,
        (
            Uuid,
            Uuid,
            String,
            Option<String>,
            Option<String>,
            String,
            NaiveDate,
            bool,
        ),
    >(
        r#"SELECT id, ledger_id, description, payee, reference, frequency, next_date, is_active
           FROM transaction_templates WHERE is_active = TRUE AND next_date <= CURRENT_DATE"#,
    )
    .fetch_all(&state.pool)
    .await?;

    for (template_id, ledger_id, description, payee, reference, frequency, next_date, _is_active) in
        due
    {
        let template = (description, payee, reference, frequency, next_date, true);
        let postings = sqlx::query_as::<_, (Uuid, String, Decimal, Option<String>)>(
            "SELECT account_id, direction, amount, memo FROM template_postings WHERE template_id = $1",
        )
        .bind(template_id)
        .fetch_all(&state.pool)
        .await?;

        // For auto-generation, use the system user
        if let Err(e) =
            run_template(&state, ledger_id, &template, &postings, next_date, _user.id).await
        {
            tracing::warn!(
                template_id = %template_id,
                error = %e,
                "process_due: run_template failed"
            );
        }
    }

    Ok(Redirect::to("/").into_response())
}

async fn run_template(
    state: &AppState,
    ledger_id: Uuid,
    template: &(
        String,
        Option<String>,
        Option<String>,
        String,
        NaiveDate,
        bool,
    ),
    postings: &[(Uuid, String, Decimal, Option<String>)],
    due_date: NaiveDate,
    user_id: Uuid,
) -> AppResult<Uuid> {
    let mut tx = state.pool.begin().await?;

    let txn_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, payee, reference, kind, currency, created_by)
           VALUES ($1, $2, $3, $4, $5, 'recurring',
                   (SELECT base_currency FROM ledgers WHERE id = $1),
                   $6)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(due_date)
    .bind(&template.0)
    .bind(&template.1)
    .bind(&template.2)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    for (account_id, direction, amount, memo) in postings {
        sqlx::query(
            "INSERT INTO postings (transaction_id, account_id, direction, amount, memo)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(txn_id)
        .bind(account_id)
        .bind(direction)
        .bind(amount)
        .bind(memo)
        .execute(&mut *tx)
        .await?;
    }

    let advance_date = advance_frequency(due_date, &template.3);
    sqlx::query(
        "UPDATE transaction_templates SET next_date = $1, updated_at = now() WHERE id = $2",
    )
    .bind(advance_date)
    .bind(txn_id)
    .execute(&mut *tx)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user_id,
        "auto_generate",
        "transaction",
        Some(txn_id),
        None,
        Some(serde_json::json!({
            "template_id": txn_id,
            "template_description": template.0,
        })),
    )
    .await;

    tx.commit().await?;

    Ok(txn_id)
}

fn advance_frequency(date: NaiveDate, frequency: &str) -> NaiveDate {
    use chrono::Datelike;
    match frequency {
        "weekly" => date + chrono::Duration::weeks(1),
        "biweekly" => date + chrono::Duration::weeks(2),
        "monthly" => {
            let mut y = date.year();
            let mut m = date.month() as i32 + 1;
            if m > 12 {
                m = 1;
                y += 1;
            }
            NaiveDate::from_ymd_opt(y, m as u32, date.day()).unwrap_or(date)
        }
        "quarterly" => {
            let mut y = date.year();
            let mut m = date.month() as i32 + 3;
            while m > 12 {
                m -= 12;
                y += 1;
            }
            NaiveDate::from_ymd_opt(y, m as u32, date.day()).unwrap_or(date)
        }
        "yearly" => {
            NaiveDate::from_ymd_opt(date.year() + 1, date.month(), date.day()).unwrap_or(date)
        }
        _ => date,
    }
}
