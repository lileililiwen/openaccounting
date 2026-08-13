use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use chrono::Datelike;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::Backend,
    audit,
    error::{AppError, AppResult},
    handlers::ledgers,
    reports::{build_balance_sheet, build_income_statement},
    templates::entities::{ConsolidatedBalanceSheet, EntityForm, EntityList, EntityRow},
    AppState,
};

#[derive(Deserialize)]
pub struct NewEntityForm {
    pub name: String,
    pub legal_name: Option<String>,
    pub tax_id: Option<String>,
}

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    let rows = sqlx::query_as::<_, (Uuid, String, String, String, i64)>(
        r#"SELECT e.id, e.name, COALESCE(e.legal_name, '') AS legal_name, COALESCE(e.tax_id, '') AS tax_id,
                  (SELECT COUNT(*) FROM ledgers WHERE entity_id = e.id) AS ledger_count
           FROM entities e
           WHERE e.owner_id = $1
           ORDER BY e.name"#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;

    let entities: Vec<EntityRow> = rows
        .into_iter()
        .map(|(id, name, legal_name, tax_id, ledger_count)| EntityRow {
            id,
            name,
            legal_name,
            tax_id,
            ledger_count,
        })
        .collect();

    Ok(render_response(EntityList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id: Uuid::nil(),
        ledger_name: String::new(),
        entities,
    }))
}

pub async fn new_page(
    auth: AuthSession<Backend>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    Ok(render_response(EntityForm {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id: Uuid::nil(),
        ledger_name: String::new(),
        name: String::new(),
        legal_name: String::new(),
        tax_id: String::new(),
        error: String::new(),
    }))
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Form(form): Form<NewEntityForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("Name is required".into()));
    }

    let entity_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO entities (name, legal_name, tax_id, owner_id)
           VALUES ($1, $2, $3, $4)
           RETURNING id"#,
    )
    .bind(name)
    .bind(form.legal_name.as_deref().filter(|p| !p.is_empty()))
    .bind(form.tax_id.as_deref().filter(|p| !p.is_empty()))
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        None,
        user.id,
        "create",
        "entity",
        Some(entity_id),
        None,
        Some(serde_json::json!({
            "name": name,
        })),
    )
    .await;

    Ok(Redirect::to("/entities").into_response())
}

pub async fn consolidated(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    let entity: (String, String) = sqlx::query_as(
        r#"SELECT name, COALESCE(legal_name, '') AS legal_name
           FROM entities WHERE id = $1 AND owner_id = $2"#,
    )
    .bind(entity_id)
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let ledgers: Vec<(Uuid, String, String)> = sqlx::query_as(
        r#"SELECT id, name, base_currency FROM ledgers WHERE entity_id = $1 ORDER BY name"#,
    )
    .bind(entity_id)
    .fetch_all(&state.pool)
    .await?;

    let today = chrono::Utc::now().date_naive();
    let first_of_year = chrono::NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap_or(today);

    let mut total_assets = Decimal::ZERO;
    let mut total_liabilities = Decimal::ZERO;
    let mut total_equity = Decimal::ZERO;
    let mut total_income = Decimal::ZERO;
    let mut total_expense = Decimal::ZERO;
    let mut ledger_rows: Vec<(String, Decimal, Decimal, Decimal)> = Vec::new();

    for (ledger_id, _ledger_name, _currency) in &ledgers {
        let is = build_income_statement(&state.pool, *ledger_id, first_of_year, today).await?;
        let bs = build_balance_sheet(&state.pool, *ledger_id, today, is.net_income).await?;

        total_assets += bs.total_assets;
        total_liabilities += bs.total_liabilities;
        total_equity += bs.total_equity;
        total_income += is.revenue.total;
        total_expense += is.operating_expenses.total;

        let equity_calc = bs.total_assets - bs.total_liabilities;
        ledger_rows.push((_ledger_name.clone(), bs.total_assets, bs.total_liabilities, equity_calc));
    }

    // Detect inter-entity eliminations: count of "elimination" kind transactions
    let elimination_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM transactions WHERE kind = 'elimination'"#,
    )
    .fetch_one(&state.pool)
    .await?;

    Ok(render_response(ConsolidatedBalanceSheet {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id: Uuid::nil(),
        ledger_name: String::new(),
        entity_id,
        entity_name: entity.0,
        legal_name: entity.1,
        ledger_rows,
        total_assets,
        total_liabilities,
        total_equity,
        total_income,
        total_expense,
        net_income: total_income - total_expense,
        elimination_count,
    }))
}
