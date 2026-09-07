use crate::templates::render_response;
use axum::response::{IntoResponse, Redirect, Response};
use axum::{
    extract::{Path, State},
    Form,
};
use axum_login::AuthSession;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    domain::{
        posting_service::PostingService, Account, AccountSubtype, AccountType, Direction,
        TxnLineInput,
    },
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::accounts::{
        AccountEdit, AccountGroup, AccountList, AccountNew, OpeningBalancesPage,
    },
    AppState,
};

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let accounts = sqlx::query_as::<_, Account>(
        r#"SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
           FROM accounts WHERE ledger_id = $1 ORDER BY type, code NULLS LAST, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    // Current balance per account.
    let balances = compute_balances(&state, ledger_id).await?;

    // Group accounts by type for the chart-of-accounts view.
    let groups: Vec<AccountGroup> = vec![
        ("ASSET", AccountType::Asset),
        ("LIABILITY", AccountType::Liability),
        ("EQUITY", AccountType::Equity),
        ("INCOME", AccountType::Income),
        ("EXPENSE", AccountType::Expense),
    ]
    .into_iter()
    .map(|(label, ty)| AccountGroup {
        account_type: label.to_string(),
        accounts: accounts
            .iter()
            .filter(|a| a.r#type == ty.as_str())
            .cloned()
            .collect(),
    })
    .filter(|g| !g.accounts.is_empty())
    .collect();

    Ok(render_response(AccountList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "accounts".to_string(),
        groups,
        balances,
    }))
}

pub async fn balances_for(state: &AppState, ledger_id: Uuid) -> AppResult<HashMap<Uuid, Decimal>> {
    compute_balances(state, ledger_id).await
}

async fn compute_balances(state: &AppState, ledger_id: Uuid) -> AppResult<HashMap<Uuid, Decimal>> {
    let rows = sqlx::query_as::<_, (Uuid, String, Decimal)>(
        r#"
        SELECT a.id, a.type,
               COALESCE(SUM(CASE WHEN p.direction='DEBIT'  THEN p.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN p.direction='CREDIT' THEN p.amount ELSE 0 END), 0) AS raw
        FROM accounts a
        LEFT JOIN postings p ON p.account_id = a.id
        WHERE a.ledger_id = $1
        GROUP BY a.id, a.type
        "#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    let mut map = HashMap::new();
    for (id, ty, raw) in rows {
        let val = match ty.as_str() {
            "ASSET" | "EXPENSE" => raw,
            _ => -raw,
        };
        map.insert(id, val);
    }
    Ok(map)
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    Ok(render_response(AccountNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "accounts".to_string(),
        account_types: vec![
            AccountType::Asset,
            AccountType::Liability,
            AccountType::Equity,
            AccountType::Income,
            AccountType::Expense,
        ],
        error: String::new(),
    }))
}

#[derive(Deserialize)]
pub struct NewAccountForm {
    pub name: String,
    pub code: Option<String>,
    pub account_type: String,
    pub account_subtype: String,
    pub description: Option<String>,
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NewAccountForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let account_type = match form.account_type.as_str() {
        "ASSET" => AccountType::Asset,
        "LIABILITY" => AccountType::Liability,
        "EQUITY" => AccountType::Equity,
        "INCOME" => AccountType::Income,
        "EXPENSE" => AccountType::Expense,
        other => {
            return Ok(render_response(AccountNew {
                user_id: user.id,
                username: user.username.clone(),
                user_role: user.role.clone(),
                ledger_id,
                ledger_name: ledger.name.clone(),
                current_section: "accounts".to_string(),
                account_types: vec![],
                error: format!("Invalid account type: {}", other),
            }));
        }
    };
    let account_subtype = match AccountSubtype::from_db(&form.account_subtype) {
        Some(st) => st,
        None => {
            return Ok(render_response(AccountNew {
                user_id: user.id,
                username: user.username.clone(),
                user_role: user.role.clone(),
                ledger_id,
                ledger_name: ledger.name.clone(),
                current_section: "accounts".to_string(),
                account_types: vec![],
                error: format!("Invalid account subtype: {}", form.account_subtype),
            }));
        }
    };
    // Validate subtype is valid for the account type
    if !account_type.valid_subtypes().contains(&account_subtype) {
        return Ok(render_response(AccountNew {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name.clone(),
            current_section: "accounts".to_string(),
            account_types: vec![],
            error: format!(
                "Subtype {} is not valid for account type {}",
                account_subtype.display_label(),
                account_type.display_label()
            ),
        }));
    }
    let name = form.name.trim();
    if name.is_empty() {
        return Ok(render_response(AccountNew {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name.clone(),
            current_section: "accounts".to_string(),
            account_types: vec![],
            error: "Name is required".into(),
        }));
    }
    let result = sqlx::query(
        r#"INSERT INTO accounts (ledger_id, name, code, type, subtype, currency, description)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(name)
    .bind(
        form.code
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(account_type.as_str())
    .bind(account_subtype.as_str())
    .bind(&ledger.base_currency)
    .bind(
        form.description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint().is_some() => {
            AppError::Conflict("Account name already exists in this ledger".into())
        }
        _ => AppError::Db(e),
    })?;

    // Audit log
    let account_id: Uuid = result.get(0);
    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "account",
        Some(account_id),
        None,
        Some(serde_json::json!({
            "name": name,
            "type": account_type.as_str(),
            "subtype": account_subtype.as_str()
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/accounts", ledger_id)).into_response())
}

async fn load_account(state: &AppState, ledger_id: Uuid, account_id: Uuid) -> AppResult<Account> {
    sqlx::query_as::<_, Account>(
        r#"SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
           FROM accounts WHERE id = $1 AND ledger_id = $2"#,
    )
    .bind(account_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)
}

async fn posting_count(state: &AppState, account_id: Uuid) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM postings WHERE account_id = $1")
            .bind(account_id)
            .fetch_one(&state.pool)
            .await?,
    )
}

/// Build the `AccountEdit` page for an account, shared by the GET page
/// and the error re-render on POST.
fn edit_view(
    user_id: Uuid,
    username: String,
    user_role: String,
    ledger_id: Uuid,
    ledger_name: String,
    account: Account,
    can_change_type: bool,
    error: String,
) -> AccountEdit {
    let valid_subtypes = account
        .account_type()
        .map(|t| {
            t.valid_subtypes()
                .iter()
                .map(|s| (s.as_str().to_string(), s.display_label().to_string()))
                .collect()
        })
        .unwrap_or_default();
    AccountEdit {
        user_id,
        username,
        user_role,
        ledger_id,
        ledger_name,
        current_section: "accounts".to_string(),
        account,
        account_types: vec![
            AccountType::Asset,
            AccountType::Liability,
            AccountType::Equity,
            AccountType::Income,
            AccountType::Expense,
        ],
        valid_subtypes,
        can_change_type,
        error,
    }
}

pub async fn edit_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let account = load_account(&state, ledger_id, account_id).await?;
    let can_change_type = posting_count(&state, account_id).await? == 0;
    Ok(render_response(edit_view(
        user.id,
        user.username.clone(),
        user.role.clone(),
        ledger_id,
        ledger.name,
        account,
        can_change_type,
        String::new(),
    )))
}

#[derive(Deserialize)]
pub struct EditAccountForm {
    pub name: String,
    pub code: Option<String>,
    pub description: Option<String>,
    /// Present only when the selects are enabled (account has no postings).
    pub account_type: Option<String>,
    pub account_subtype: Option<String>,
}

pub async fn update(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<EditAccountForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let account = load_account(&state, ledger_id, account_id).await?;
    let can_change_type = posting_count(&state, account_id).await? == 0;

    let name = form.name.trim();
    if name.is_empty() {
        return Ok(render_response(edit_view(
            user.id,
            user.username.clone(),
            user.role.clone(),
            ledger_id,
            ledger.name.clone(),
            account,
            can_change_type,
            "Name is required".into(),
        )));
    }

    // Type/subtype are only read from the form when the account has no
    // postings; otherwise the existing classification is kept.
    let (new_type, new_subtype): (String, String) = if can_change_type {
        let ty = match form.account_type.as_deref() {
            Some("ASSET") => AccountType::Asset,
            Some("LIABILITY") => AccountType::Liability,
            Some("EQUITY") => AccountType::Equity,
            Some("INCOME") => AccountType::Income,
            Some("EXPENSE") => AccountType::Expense,
            _ => {
                return Ok(render_response(edit_view(
                    user.id,
                    user.username.clone(),
                    user.role.clone(),
                    ledger_id,
                    ledger.name.clone(),
                    account,
                    can_change_type,
                    "Invalid account type".into(),
                )))
            }
        };
        let st = match form
            .account_subtype
            .as_deref()
            .and_then(AccountSubtype::from_db)
        {
            Some(s) => s,
            None => {
                return Ok(render_response(edit_view(
                    user.id,
                    user.username.clone(),
                    user.role.clone(),
                    ledger_id,
                    ledger.name.clone(),
                    account,
                    can_change_type,
                    "Invalid account subtype".into(),
                )))
            }
        };
        if !ty.valid_subtypes().contains(&st) {
            return Ok(render_response(edit_view(
                user.id,
                user.username.clone(),
                user.role.clone(),
                ledger_id,
                ledger.name.clone(),
                account,
                can_change_type,
                format!(
                    "Subtype {} is not valid for account type {}",
                    st.display_label(),
                    ty.display_label()
                ),
            )));
        }
        (ty.as_str().to_string(), st.as_str().to_string())
    } else {
        (account.r#type.clone(), account.subtype.clone())
    };

    sqlx::query(
        r#"UPDATE accounts
           SET name = $1, code = $2, description = $3, type = $4, subtype = $5, updated_at = now()
           WHERE id = $6 AND ledger_id = $7"#,
    )
    .bind(name)
    .bind(
        form.code
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(
        form.description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(&new_type)
    .bind(&new_subtype)
    .bind(account_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint().is_some() => {
            AppError::Conflict("Account name already exists in this ledger".into())
        }
        _ => AppError::Db(e),
    })?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "update",
        "account",
        Some(account_id),
        None,
        Some(serde_json::json!({
            "name": name,
            "type": new_type,
            "subtype": new_subtype,
            "changed_classification": can_change_type,
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/accounts", ledger_id)).into_response())
}

pub async fn toggle_archive(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, account_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let current: Option<(bool, String)> =
        sqlx::query_as("SELECT is_archived, name FROM accounts WHERE id = $1 AND ledger_id = $2")
            .bind(account_id)
            .bind(ledger_id)
            .fetch_optional(&state.pool)
            .await?;
    let (was_archived, name) = current.ok_or(AppError::NotFound)?;
    let archived = !was_archived;
    sqlx::query(
        "UPDATE accounts SET is_archived = $1, updated_at = now() WHERE id = $2 AND ledger_id = $3",
    )
    .bind(archived)
    .bind(account_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        if archived { "archive" } else { "activate" },
        "account",
        Some(account_id),
        None,
        Some(serde_json::json!({ "name": name })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/accounts", ledger_id)).into_response())
}

/// True when the ledger already has an opening-balances entry.
async fn opening_balances_exist(state: &AppState, ledger_id: Uuid) -> AppResult<bool> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM transactions
         WHERE ledger_id = $1 AND description = 'Opening balances' AND kind != 'draft'",
    )
    .bind(ledger_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(n > 0)
}

/// Balance-sheet accounts (asset / liability / equity) for the
/// opening-balances form, paired with their current balance.
async fn balance_sheet_accounts(
    state: &AppState,
    ledger_id: Uuid,
) -> AppResult<Vec<(Account, Decimal)>> {
    let accounts = sqlx::query_as::<_, Account>(
        r#"SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
           FROM accounts
           WHERE ledger_id = $1 AND type IN ('ASSET', 'LIABILITY', 'EQUITY')
           ORDER BY type, code NULLS LAST, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    let balances = compute_balances(state, ledger_id).await?;
    Ok(accounts
        .into_iter()
        .map(|a| (a.clone(), balances.get(&a.id).copied().unwrap_or_default()))
        .collect())
}

fn opening_balances_view(
    user_id: Uuid,
    username: String,
    user_role: String,
    ledger_id: Uuid,
    ledger_name: String,
    accounts: Vec<(Account, Decimal)>,
    exists: bool,
    date: String,
    error: String,
) -> OpeningBalancesPage {
    OpeningBalancesPage {
        user_id,
        username,
        user_role,
        ledger_id,
        ledger_name,
        current_section: "accounts".to_string(),
        accounts,
        has_opening: exists,
        date,
        error,
    }
}

pub async fn opening_balances_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let rows = balance_sheet_accounts(&state, ledger_id).await?;
    let exists = opening_balances_exist(&state, ledger_id).await?;
    let date = ledger.created_at.date_naive().to_string();
    Ok(render_response(opening_balances_view(
        user.id,
        user.username.clone(),
        user.role.clone(),
        ledger_id,
        ledger.name,
        rows,
        exists,
        date,
        String::new(),
    )))
}

#[derive(Deserialize)]
pub struct OpeningBalancesForm {
    pub date: String,
    /// `amount[<account_id>]` fields for each balance-sheet account.
    #[serde(flatten, default)]
    pub extra: std::collections::HashMap<String, String>,
}

pub async fn opening_balances_create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<OpeningBalancesForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let rows = balance_sheet_accounts(&state, ledger_id).await?;
    let exists = opening_balances_exist(&state, ledger_id).await?;

    let re_render = |error: String| {
        render_response(opening_balances_view(
            user.id,
            user.username.clone(),
            user.role.clone(),
            ledger_id,
            ledger.name.clone(),
            rows.clone(),
            exists,
            form.date.clone(),
            error,
        ))
    };

    if exists {
        return Ok(re_render(
            "Opening balances have already been recorded for this ledger. Reverse the existing \"Opening balances\" entry to change them.".into(),
        ));
    }

    let date = match NaiveDate::parse_from_str(&form.date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return Ok(re_render("Invalid date (YYYY-MM-DD)".into())),
    };

    // Build one leg per account whose desired balance differs from the
    // current balance, in the account's normal direction.
    let mut lines: Vec<TxnLineInput> = Vec::new();
    for (account, current) in &rows {
        let key = format!("amount[{}]", account.id);
        let raw = form
            .extra
            .get(&key)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if raw.is_empty() {
            continue;
        }
        let target: Decimal = raw
            .parse()
            .map_err(|_| AppError::Validation(format!("Invalid amount for {}", account.name)))?;
        let delta = target - current;
        if delta == Decimal::ZERO {
            continue;
        }
        let debit_normal = account.r#type == "ASSET";
        let signed = if debit_normal { delta } else { -delta };
        lines.push(TxnLineInput {
            account_id: account.id,
            signed_amount: signed,
            memo: Some(format!("Opening balance — {}", account.name)),
            tax_rate_id: None,
            foreign: None,
        });
    }

    let net: Decimal = lines.iter().map(|l| l.signed_amount).sum();
    if lines.is_empty() || net == Decimal::ZERO {
        return Ok(re_render(
            "Enter at least one opening balance that differs from the account's current balance."
                .into(),
        ));
    }

    // The contra side is the "Opening Balances" equity account (created
    // on the fly if a ledger somehow lacks it).
    let opening_account_id: Uuid = match sqlx::query_scalar(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = 'Opening Balances'",
    )
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    {
        Some(id) => id,
        None => {
            sqlx::query_scalar(
                "INSERT INTO accounts (ledger_id, name, code, type, subtype, currency)
             VALUES ($1, 'Opening Balances', '3010', 'EQUITY', 'EQUITY', $2)
             RETURNING id",
            )
            .bind(ledger_id)
            .bind(&ledger.base_currency)
            .fetch_one(&state.pool)
            .await?
        }
    };
    lines.push(TxnLineInput {
        account_id: opening_account_id,
        signed_amount: -net,
        memo: Some("Opening balances contra".to_string()),
        tax_rate_id: None,
        foreign: None,
    });

    let new_txn = crate::domain::posting_service::NewTransaction {
        ledger_id,
        txn_date: date,
        description: "Opening balances".to_string(),
        payee: None,
        reference: Some("opening-balances".to_string()),
        kind: Some("standard".to_string()),
        created_by: user.id,
        lines,
        reverses_id: None,
        number: None,
        tax_links: vec![],
    };
    if let Err(e) = PostingService::create(&state.pool, new_txn).await {
        return Err(match e {
            crate::domain::posting_service::PostingServiceError::PeriodClosed { year, .. } => {
                AppError::Validation(format!("Period {year} is closed"))
            }
            crate::domain::posting_service::PostingServiceError::Unbalanced { debits, credits } => {
                AppError::Validation(format!(
                    "Opening balances do not balance (debits={debits}, credits={credits})"
                ))
            }
            other => AppError::Internal(format!("opening balances: {other}")),
        });
    }

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "opening_balances",
        None,
        None,
        Some(serde_json::json!({
            "ledger_id": ledger_id,
            "date": date.to_string(),
            "accounts": rows.len(),
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/accounts", ledger_id)).into_response())
}
