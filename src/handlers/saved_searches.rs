//! Saved searches (`u2-saved-searches`).
//!
//! Per-user named filters for the transactions list. Each row
//! stores a raw query string (`from=...&to=...&q=...`) that
//! can be re-applied to the list endpoint. A single row per
//! user can be flagged `is_default = TRUE` so the list
//! endpoint auto-applies it.
//!
//! Routes:
//! * `GET  /ledgers/{id}/searches`           — list saved searches
//! * `POST /ledgers/{id}/searches`           — create
//! * `POST /ledgers/{id}/searches/{sid}/delete`  — remove
//! * `POST /ledgers/{id}/searches/{sid}/default` — flag default

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use axum_login::AuthSession;
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    AppState,
};

#[derive(Deserialize)]
pub struct SaveForm {
    pub name: String,
    pub query: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Deserialize)]
pub struct DefaultForm {
    #[serde(default)]
    pub next: Option<String>,
}

/// GET /ledgers/{id}/searches — render the saved-searches
/// partial. Used via HTMX partial from the transactions list.
pub async fn list_searches(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_access(&state, user.id, ledger_id).await?;

    let rows = sqlx::query(
        "SELECT id, name, query, color, is_default
         FROM saved_searches WHERE user_id = $1
         ORDER BY is_default DESC, name ASC",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;

    let mut html = String::new();
    if rows.is_empty() {
        return Ok(axum::response::Html::<String>(
            "<div class=\"text-xs text-slate-500\">No saved searches yet.</div>".into(),
        )
        .into_response());
    }
    html.push_str("<ul class=\"space-y-1\">");
    for row in rows {
        let name: String = row.try_get("name")?;
        let query: String = row.try_get("query")?;
        let color: Option<String> = row.try_get("color")?;
        let is_default: bool = row.try_get("is_default")?;
        let id: Uuid = row.try_get("id")?;
        let safe_query = query.replace('"', "&quot;");
        let color_class = match color.as_deref() {
            Some("red") => "bg-rose-100 text-rose-800",
            Some("amber") => "bg-amber-100 text-amber-800",
            Some("emerald") => "bg-emerald-100 text-emerald-800",
            Some("sky") => "bg-sky-100 text-sky-800",
            Some("violet") => "bg-violet-100 text-violet-800",
            Some("pink") => "bg-pink-100 text-pink-800",
            _ => "bg-slate-100 text-slate-700",
        };
        let star = if is_default { "★ " } else { "" };
        html.push_str(&format!(
            "<li class=\"flex items-center gap-2 text-sm\">\
              <a href=\"/ledgers/{ledger_id}/transactions?{safe_query}\" class=\"flex-1 rounded px-2 py-1 {color_class}\">{star}{name}</a>\
              <form method=\"post\" action=\"/ledgers/{ledger_id}/searches/{id}/default\" class=\"inline\">\
                <button class=\"text-xs text-slate-500 hover:text-slate-800\" title=\"Make default\">★</button>\
              </form>\
              <form method=\"post\" action=\"/ledgers/{ledger_id}/searches/{id}/delete\" class=\"inline\" onsubmit=\"return confirm('Delete this saved search?')\">\
                <button class=\"text-xs text-rose-600 hover:text-rose-800\">Delete</button>\
              </form>\
            </li>"
        ));
    }
    html.push_str("</ul>");
    Ok(axum::response::Html::<String>(html).into_response())
}

/// POST /ledgers/{id}/searches — create or update a saved
/// search for the signed-in user. We round-trip via the
/// transactions list URL on completion so the user lands back
/// where they were.
pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<SaveForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_access(&state, user.id, ledger_id).await?;

    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }
    let query = form.query.trim_start_matches('?').to_string();
    if query.is_empty() {
        return Err(AppError::Validation("query must not be empty".into()));
    }
    let color = match form.color.as_deref() {
        Some("slate") | Some("red") | Some("amber") | Some("emerald") | Some("sky")
        | Some("violet") | Some("pink") => form.color.clone(),
        _ => None,
    };

    // Upsert on (user_id, name). If the user already had a
    // search with this name, we update its query + color.
    sqlx::query(
        r#"INSERT INTO saved_searches (user_id, name, query, color, updated_at)
           VALUES ($1, $2, $3, $4, now())
           ON CONFLICT (user_id, name)
           DO UPDATE SET query = EXCLUDED.query,
                         color = EXCLUDED.color,
                         updated_at = now()"#,
    )
    .bind(user.id)
    .bind(name)
    .bind(&query)
    .bind(color)
    .execute(&state.pool)
    .await?;

    let next = form
        .next
        .as_deref()
        .filter(|s| s.starts_with('/') && !s.starts_with("//"))
        .unwrap_or("/ledgers/{ledger_id}/transactions");
    Ok(Redirect::to(next).into_response())
}

/// POST /ledgers/{id}/searches/{sid}/default — flag this row as
/// the user's default landing filter. The transactions list
/// endpoint reads `is_default` and auto-applies the query.
pub async fn make_default(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, sid)): Path<(Uuid, Uuid)>,
    Form(form): Form<DefaultForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_access(&state, user.id, ledger_id).await?;

    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM saved_searches WHERE id = $1 AND user_id = $2")
            .bind(sid)
            .bind(user.id)
            .fetch_optional(&state.pool)
            .await?;
    if row.is_none() {
        return Err(AppError::NotFound);
    }

    let mut tx = state.pool.begin().await?;
    // Clear any existing default for this user.
    sqlx::query("UPDATE saved_searches SET is_default = FALSE WHERE user_id = $1")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    // Flag this one.
    sqlx::query("UPDATE saved_searches SET is_default = TRUE WHERE id = $1")
        .bind(sid)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let next = form
        .next
        .as_deref()
        .filter(|s| s.starts_with('/') && !s.starts_with("//"))
        .unwrap_or("/ledgers/{ledger_id}/transactions");
    Ok(Redirect::to(next).into_response())
}

/// POST /ledgers/{id}/searches/{sid}/delete — remove the saved
/// search. The list endpoint never reads from this table when
/// no rows match, so removal is safe even if the row was the
/// default.
pub async fn delete(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, sid)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_access(&state, user.id, ledger_id).await?;

    let n: u64 = sqlx::query("DELETE FROM saved_searches WHERE id = $1 AND user_id = $2")
        .bind(sid)
        .bind(user.id)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/transactions")).into_response())
}

/// Return the user's default search query (the `?…` part), or
/// `None` if the user has not flagged a default. Called from
/// the transactions list endpoint so the default is applied
/// automatically when no other filters are in the URL.
pub async fn default_query(pool: &sqlx::PgPool, user_id: Uuid) -> AppResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT query FROM saved_searches
         WHERE user_id = $1 AND is_default = TRUE LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(q,)| q))
}
