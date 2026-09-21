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
    audit,
    auth::Backend,
    domain::depreciation,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::fixed_assets::{FixedAssetForm, FixedAssetList, FixedAssetRow, FixedAssetShow},
    AppState,
};

#[derive(Deserialize)]
pub struct NewAssetForm {
    pub name: String,
    pub description: Option<String>,
    pub account_id: String,
    pub purchase_date: String,
    pub purchase_cost: String,
    pub salvage_value: Option<String>,
    pub useful_life_years: String,
    pub depreciation_method: String,
}

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let assets = sqlx::query_as::<_, FixedAssetRow>(
        r#"SELECT id, name, COALESCE(description, '') AS description, account_id, purchase_date,
                  purchase_cost, salvage_value, useful_life_years, depreciation_method,
                  accumulated_depreciation, status, disposed_date, disposed_amount
           FROM fixed_assets WHERE ledger_id = $1 ORDER BY purchase_date DESC"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(FixedAssetList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        assets,
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
        r#"SELECT id, code, name FROM accounts
           WHERE ledger_id = $1 AND type = 'ASSET' AND is_archived = FALSE
           ORDER BY code NULLS LAST, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(FixedAssetForm {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        accounts,
        name: String::new(),
        description: String::new(),
        purchase_date: chrono::Utc::now().date_naive().to_string(),
        purchase_cost: String::new(),
        salvage_value: String::new(),
        useful_life_years: String::new(),
        depreciation_method: "straight_line".to_string(),
        error: String::new(),
    }))
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewAssetForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("Name is required".into()));
    }

    let account_id = Uuid::parse_str(&form.account_id)
        .map_err(|_| AppError::Validation("Invalid account".into()))?;
    let purchase_cost: Decimal = form
        .purchase_cost
        .parse()
        .map_err(|_| AppError::Validation("Invalid cost".into()))?;
    let salvage_value: Decimal = form
        .salvage_value
        .as_deref()
        .unwrap_or("0")
        .parse()
        .map_err(|_| AppError::Validation("Invalid salvage".into()))?;
    let useful_life_years: i32 = form
        .useful_life_years
        .parse()
        .map_err(|_| AppError::Validation("Invalid life".into()))?;
    if useful_life_years <= 0 {
        return Err(AppError::Validation("Life must be > 0".into()));
    }
    let depreciation_method = form.depreciation_method.clone();
    if !["straight_line", "declining_balance"].contains(&depreciation_method.as_str()) {
        return Err(AppError::Validation("Invalid method".into()));
    }
    let purchase_date = chrono::NaiveDate::parse_from_str(&form.purchase_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid date".into()))?;

    let asset_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO fixed_assets (ledger_id, name, description, account_id, purchase_date, purchase_cost, salvage_value, useful_life_years, depreciation_method)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(name)
    .bind(form.description.as_deref().filter(|p| !p.is_empty()))
    .bind(account_id)
    .bind(purchase_date)
    .bind(purchase_cost)
    .bind(salvage_value)
    .bind(useful_life_years)
    .bind(&depreciation_method)
    .fetch_one(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "fixed_asset",
        Some(asset_id),
        None,
        Some(serde_json::json!({
            "name": name,
            "cost": purchase_cost.to_string(),
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/fixed-assets", ledger_id)).into_response())
}

pub async fn show(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, asset_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let asset = sqlx::query_as::<_, FixedAssetRow>(
        r#"SELECT id, name, COALESCE(description, '') AS description, account_id, purchase_date,
                  purchase_cost, salvage_value, useful_life_years, depreciation_method,
                  accumulated_depreciation, status, disposed_date, disposed_amount
           FROM fixed_assets WHERE id = $1 AND ledger_id = $2"#,
    )
    .bind(asset_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(render_response(FixedAssetShow {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        asset,
    }))
}

pub async fn calculate_depreciation(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, asset_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let asset: (Decimal, Decimal, i32, String, Decimal, String) = sqlx::query_as(
        r#"SELECT purchase_cost, salvage_value, useful_life_years, depreciation_method, accumulated_depreciation, status
           FROM fixed_assets WHERE id = $1"#,
    )
    .bind(asset_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    if asset.5 == "disposed" || asset.5 == "fully_depreciated" {
        return Err(AppError::Validation("Asset cannot be depreciated".into()));
    }

    // Method-aware monthly charge (`accounting-dimensions`): the stored
    // `depreciation_method` picks straight-line (existing behavior) or
    // double-declining-balance on net book value, capped at the
    // depreciable base so NBV never drops below salvage.
    let proposed = if asset.3 == "declining_balance" {
        depreciation::declining_balance_monthly(asset.0 - asset.4, asset.2)
    } else {
        depreciation::straight_line_monthly(asset.0, asset.1, asset.2)
    };
    let to_add = depreciation::cap_at_depreciable(proposed, asset.4, asset.0, asset.1);
    let new_total = asset.4 + to_add;
    let depreciable = asset.0 - asset.1;
    let new_status = if new_total >= depreciable {
        "fully_depreciated"
    } else {
        "active"
    };

    sqlx::query("UPDATE fixed_assets SET accumulated_depreciation = $1, status = $2, updated_at = now() WHERE id = $3")
        .bind(new_total)
        .bind(new_status)
        .bind(asset_id)
        .execute(&state.pool)
        .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "calculate_depreciation",
        "fixed_asset",
        Some(asset_id),
        None,
        Some(serde_json::json!({
            "amount": to_add.to_string(),
            "new_total": new_total.to_string(),
            "method": asset.3,
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/fixed-assets/{}", ledger_id, asset_id)).into_response())
}

pub async fn dispose(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, asset_id)): Path<(Uuid, Uuid)>,
    axum::extract::Form(form): axum::extract::Form<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let disposed_date = form.get("disposed_date").cloned().unwrap_or_default();
    let disposed_date = chrono::NaiveDate::parse_from_str(&disposed_date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid date".into()))?;
    let disposed_amount: Decimal = form
        .get("disposed_amount")
        .ok_or(AppError::Validation("Missing amount".into()))?
        .parse()
        .map_err(|_| AppError::Validation("Invalid amount".into()))?;

    // Disposal gain/loss posting (`accounting-dimensions`): proceeds vs
    // net book value. Gain (proceeds > NBV) credits the first income
    // account; loss debits the first expense account; cash receives the
    // proceeds and the asset account is relieved of NBV.
    let asset_row: Option<(Uuid, Decimal, Decimal)> = sqlx::query_as(
        "SELECT account_id, purchase_cost, accumulated_depreciation FROM fixed_assets WHERE id = $1 AND ledger_id = $2",
    )
    .bind(asset_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((asset_account, cost, accumulated)) = asset_row else {
        return Err(AppError::NotFound);
    };
    let nbv = depreciation::net_book_value(cost, accumulated);
    let gain_loss = disposed_amount - nbv;

    if gain_loss != Decimal::ZERO {
        let cash: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'ASSET'
               AND (LOWER(name) LIKE '%cash%' OR LOWER(name) LIKE '%bank%')
               ORDER BY code NULLS LAST LIMIT 1"#,
        )
        .bind(ledger_id)
        .fetch_optional(&state.pool)
        .await?
        .flatten()
        .or(sqlx::query_scalar(
            "SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'ASSET' ORDER BY code NULLS LAST LIMIT 1",
        )
        .bind(ledger_id)
        .fetch_optional(&state.pool)
        .await?
        .flatten());
        let Some(cash_account) = cash else {
            return Err(AppError::Validation(
                "disposal needs a cash account for the proceeds".into(),
            ));
        };
        let (gl_account, gl_type): (Option<Uuid>, &str) = if gain_loss > Decimal::ZERO {
            (
                sqlx::query_scalar(
                    "SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'INCOME' ORDER BY name LIMIT 1",
                )
                .bind(ledger_id)
                .fetch_optional(&state.pool)
                .await?,
                "gain",
            )
        } else {
            (
                sqlx::query_scalar(
                    "SELECT id FROM accounts WHERE ledger_id = $1 AND type = 'EXPENSE' ORDER BY name LIMIT 1",
                )
                .bind(ledger_id)
                .fetch_optional(&state.pool)
                .await?,
                "loss",
            )
        };
        let Some(gl_account) = gl_account else {
            return Err(AppError::Validation(format!(
                "disposal needs an income/expense account for the {gl_type}"
            )));
        };

        let mut lines = vec![
            crate::domain::transaction::TxnLineInput {
                account_id: cash_account,
                signed_amount: disposed_amount,
                memo: Some("Disposal proceeds".to_string()),
                tax_rate_id: None,
                foreign: None,
                cost_center_id: None,
                project_id: None,
            },
            crate::domain::transaction::TxnLineInput {
                account_id: asset_account,
                signed_amount: -nbv,
                memo: Some("Disposal: relieve asset NBV".to_string()),
                tax_rate_id: None,
                foreign: None,
                cost_center_id: None,
                project_id: None,
            },
        ];
        if gain_loss > Decimal::ZERO {
            lines.push(crate::domain::transaction::TxnLineInput {
                account_id: gl_account,
                signed_amount: -gain_loss,
                memo: Some("Disposal gain".to_string()),
                tax_rate_id: None,
                foreign: None,
                cost_center_id: None,
                project_id: None,
            });
        } else {
            lines.push(crate::domain::transaction::TxnLineInput {
                account_id: gl_account,
                signed_amount: -gain_loss, // -gain_loss > 0 = debit loss
                memo: Some("Disposal loss".to_string()),
                tax_rate_id: None,
                foreign: None,
                cost_center_id: None,
                project_id: None,
            });
        }
        crate::domain::posting_service::PostingService::create(
            &state.pool,
            crate::domain::posting_service::NewTransaction {
                ledger_id,
                txn_date: disposed_date,
                description: "Disposal of fixed asset".to_string(),
                payee: None,
                reference: None,
                lines,
                kind: None,
                created_by: user.id,
                reverses_id: None,
                number: None,
                tax_links: vec![],
            },
        )
        .await
        .map_err(|e| AppError::Validation(format!("disposal posting failed: {e}")))?;
    }

    sqlx::query(
        "UPDATE fixed_assets SET status = 'disposed', disposed_date = $1, disposed_amount = $2, updated_at = now() WHERE id = $3",
    )
    .bind(disposed_date)
    .bind(disposed_amount)
    .bind(asset_id)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "dispose",
        "fixed_asset",
        Some(asset_id),
        None,
        Some(serde_json::json!({
            "amount": disposed_amount.to_string(),
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/fixed-assets", ledger_id)).into_response())
}

pub fn annual_depreciation(cost: Decimal, salvage: Decimal, life: i32) -> Decimal {
    if life <= 0 {
        return Decimal::ZERO;
    }
    (cost - salvage) / Decimal::from(life)
}

pub fn net_book_value(cost: Decimal, accumulated: Decimal) -> Decimal {
    cost - accumulated
}

#[allow(dead_code)]
fn next_year_month(d: chrono::NaiveDate) -> chrono::NaiveDate {
    let mut y = d.year();
    let mut m = d.month() as i32 + 1;
    if m > 12 {
        m = 1;
        y += 1;
    }
    chrono::NaiveDate::from_ymd_opt(y, m as u32, 1).unwrap_or(d)
}
