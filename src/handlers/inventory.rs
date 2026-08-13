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
    auth::Backend,
    audit,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::inventory::{InventoryForm, InventoryItem, InventoryList, InventoryValuation},
    AppState,
};

#[derive(Deserialize)]
pub struct NewItemForm {
    pub name: String,
    pub sku: Option<String>,
    pub description: Option<String>,
    pub asset_account_id: String,
    pub cogs_account_id: String,
    pub income_account_id: String,
}

#[derive(Deserialize)]
pub struct AdjustmentForm {
    pub quantity: i32,
    pub reason: String,
    pub adjustment_date: String,
}

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let items = sqlx::query_as::<_, InventoryItem>(
        r#"SELECT id, name, COALESCE(sku, '') AS sku, COALESCE(description, '') AS description,
                  asset_account_id, cogs_account_id, income_account_id,
                  quantity_on_hand, unit_cost, is_active
           FROM inventory_items WHERE ledger_id = $1 ORDER BY name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(InventoryList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        items,
    }))
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let accounts = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"SELECT id, code, name FROM accounts
           WHERE ledger_id = $1 AND is_archived = FALSE
           ORDER BY type, code NULLS LAST, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(InventoryForm {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        accounts,
        name: String::new(),
        sku: String::new(),
        description: String::new(),
        error: String::new(),
    }))
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewItemForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("Name is required".into()));
    }

    let asset_account_id = Uuid::parse_str(&form.asset_account_id)
        .map_err(|_| AppError::Validation("Invalid asset account".into()))?;
    let cogs_account_id = Uuid::parse_str(&form.cogs_account_id)
        .map_err(|_| AppError::Validation("Invalid COGS account".into()))?;
    let income_account_id = Uuid::parse_str(&form.income_account_id)
        .map_err(|_| AppError::Validation("Invalid income account".into()))?;

    let item_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO inventory_items (ledger_id, name, sku, description, asset_account_id, cogs_account_id, income_account_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(name)
    .bind(form.sku.as_deref().filter(|p| !p.is_empty()))
    .bind(form.description.as_deref().filter(|p| !p.is_empty()))
    .bind(asset_account_id)
    .bind(cogs_account_id)
    .bind(income_account_id)
    .fetch_one(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "inventory_item",
        Some(item_id),
        None,
        None,
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/inventory", ledger_id)).into_response())
}

pub async fn purchase(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, item_id)): Path<(Uuid, Uuid)>,
    axum::extract::Form(form): axum::extract::Form<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let quantity: i32 = form
        .get("quantity")
        .ok_or(AppError::Validation("Missing quantity".into()))?
        .parse()
        .map_err(|_| AppError::Validation("Invalid quantity".into()))?;
    let unit_cost: Decimal = form
        .get("unit_cost")
        .ok_or(AppError::Validation("Missing unit_cost".into()))?
        .parse()
        .map_err(|_| AppError::Validation("Invalid cost".into()))?;

    let item: (Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT asset_account_id, cogs_account_id, income_account_id FROM inventory_items WHERE id = $1"#,
    )
    .bind(item_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let purchase_date = chrono::Utc::now().date_naive();
    let total_cost = unit_cost * Decimal::from(quantity);

    let mut tx = state.pool.begin().await?;

    sqlx::query("UPDATE inventory_items SET quantity_on_hand = quantity_on_hand + $1, updated_at = now() WHERE id = $2")
        .bind(quantity)
        .bind(item_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        r#"INSERT INTO inventory_layers (item_id, quantity, unit_cost, remaining, purchase_date)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(item_id)
    .bind(quantity)
    .bind(unit_cost)
    .bind(quantity)
    .bind(purchase_date)
    .execute(&mut *tx)
    .await?;

    // Recompute average unit cost.
    let new_total: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(remaining * unit_cost), 0) FROM inventory_layers
           WHERE item_id = $1 AND remaining > 0"#,
    )
    .bind(item_id)
    .fetch_one(&mut *tx)
    .await?;
    let new_qty: i32 = sqlx::query_scalar("SELECT quantity_on_hand FROM inventory_items WHERE id = $1")
        .bind(item_id)
        .fetch_one(&mut *tx)
        .await?;
    let new_unit_cost = if new_qty > 0 {
        new_total / Decimal::from(new_qty)
    } else {
        Decimal::ZERO
    };
    sqlx::query("UPDATE inventory_items SET unit_cost = $1 WHERE id = $2")
        .bind(new_unit_cost)
        .bind(item_id)
        .execute(&mut *tx)
        .await?;

    // Create the inventory asset debit
    let cash_account: Uuid = sqlx::query_scalar(
        r#"SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'ASSET' AND subtype = 'cash' LIMIT 1"#,
    )
    .bind(ledger_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let txn_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, kind, currency, created_by)
           VALUES ($1, $2, $3, 'standard',
                   (SELECT base_currency FROM ledgers WHERE id = $1), $4)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(purchase_date)
    .bind(format!("Inventory purchase: qty {} @ {}", quantity, unit_cost))
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r#"INSERT INTO postings (transaction_id, account_id, direction, amount, currency)
           VALUES ($1, $2, 'DEBIT', $3, (SELECT base_currency FROM ledgers WHERE id = $4))"#,
    )
    .bind(txn_id)
    .bind(item.0)
    .bind(total_cost)
    .bind(ledger_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"INSERT INTO postings (transaction_id, account_id, direction, amount, currency)
           VALUES ($1, $2, 'CREDIT', $3, (SELECT base_currency FROM ledgers WHERE id = $4))"#,
    )
    .bind(txn_id)
    .bind(cash_account)
    .bind(total_cost)
    .bind(ledger_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Redirect::to(&format!("/ledgers/{}/inventory", ledger_id)).into_response())
}

pub async fn adjust(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, item_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<AdjustmentForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let adjustment_date = NaiveDate::parse_from_str(&form.adjustment_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid date".into()))?;

    let item: (Uuid, Uuid, i32, Decimal) = sqlx::query_as(
        r#"SELECT asset_account_id, cogs_account_id, quantity_on_hand, unit_cost
           FROM inventory_items WHERE id = $1"#,
    )
    .bind(item_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let new_qty = item.2 + form.quantity;
    if new_qty < 0 {
        return Err(AppError::Validation("Cannot reduce below 0".into()));
    }
    let cost_diff = Decimal::from(form.quantity) * item.3;

    let mut tx = state.pool.begin().await?;

    sqlx::query("UPDATE inventory_items SET quantity_on_hand = $1, updated_at = now() WHERE id = $2")
        .bind(new_qty)
        .bind(item_id)
        .execute(&mut *tx)
        .await?;

    // Create journal entry
    let txn_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO transactions (ledger_id, txn_date, description, kind, currency, created_by)
           VALUES ($1, $2, $3, 'standard',
                   (SELECT base_currency FROM ledgers WHERE id = $1), $4)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(adjustment_date)
    .bind(format!("Inventory adjustment: {}", form.reason))
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;

    if cost_diff > Decimal::ZERO {
        // Increase: debit inventory, credit COGS reduction
        sqlx::query(
            r#"INSERT INTO postings (transaction_id, account_id, direction, amount, currency)
               VALUES ($1, $2, 'DEBIT', $3, (SELECT base_currency FROM ledgers WHERE id = $4))"#,
        )
        .bind(txn_id)
        .bind(item.0)
        .bind(cost_diff)
        .bind(ledger_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"INSERT INTO postings (transaction_id, account_id, direction, amount, currency)
               VALUES ($1, $2, 'CREDIT', $3, (SELECT base_currency FROM ledgers WHERE id = $4))"#,
        )
        .bind(txn_id)
        .bind(item.1)
        .bind(cost_diff)
        .bind(ledger_id)
        .execute(&mut *tx)
        .await?;
    } else if cost_diff < Decimal::ZERO {
        let amount = cost_diff.abs();
        // Decrease (shrinkage): debit COGS, credit inventory
        sqlx::query(
            r#"INSERT INTO postings (transaction_id, account_id, direction, amount, currency)
               VALUES ($1, $2, 'DEBIT', $3, (SELECT base_currency FROM ledgers WHERE id = $4))"#,
        )
        .bind(txn_id)
        .bind(item.1)
        .bind(amount)
        .bind(ledger_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"INSERT INTO postings (transaction_id, account_id, direction, amount, currency)
               VALUES ($1, $2, 'CREDIT', $3, (SELECT base_currency FROM ledgers WHERE id = $4))"#,
        )
        .bind(txn_id)
        .bind(item.0)
        .bind(amount)
        .bind(ledger_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "adjust",
        "inventory_item",
        Some(item_id),
        None,
        Some(serde_json::json!({
            "quantity": form.quantity,
            "reason": form.reason,
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/inventory", ledger_id)).into_response())
}

pub async fn valuation(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let items = sqlx::query_as::<_, InventoryItem>(
        r#"SELECT id, name, COALESCE(sku, '') AS sku, COALESCE(description, '') AS description,
                  asset_account_id, cogs_account_id, income_account_id,
                  quantity_on_hand, unit_cost, is_active
           FROM inventory_items WHERE ledger_id = $1 AND is_active = TRUE ORDER BY name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    let total_value: Decimal = items
        .iter()
        .map(|i| Decimal::from(i.quantity_on_hand) * i.unit_cost)
        .sum();

    Ok(render_response(InventoryValuation {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        items,
        total_value,
    }))
}
