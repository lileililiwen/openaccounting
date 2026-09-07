//! Contact endpoints (`api-v2-coverage`).

use axum::response::IntoResponse;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::{
        helpers::{db_problem, next_link, page_from, require_access, with_next_link, PageParams},
        problem::Problem,
        ApiUser,
    },
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ledgers/{ledger_id}/contacts", get(list).post(create))
        .route(
            "/ledgers/{ledger_id}/contacts/{id}",
            get(get_one).patch(update),
        )
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ContactDto {
    pub id: Uuid,
    pub ledger_id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub kind: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateContactBody {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// "customer" | "vendor" | "both"
    pub kind: String,
}

async fn list(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
    Query(params): Query<PageParams>,
) -> Result<axum::response::Response, Problem> {
    require_access(&state.pool, user.0, ledger_id, false).await?;
    let page = page_from(&params)?;
    let rows: Vec<ContactDto> = sqlx::query_as(
        "SELECT id, ledger_id, name, email, phone, kind
         FROM contacts WHERE ledger_id = $1
         ORDER BY name LIMIT $2 OFFSET $3",
    )
    .bind(ledger_id)
    .bind(page.limit)
    .bind(page.offset)
    .fetch_all(&state.pool)
    .await
    .map_err(db_problem)?;
    let link = next_link(
        &format!("/api/v1/ledgers/{ledger_id}/contacts"),
        page,
        rows.len(),
    );
    Ok(with_next_link(
        Json(serde_json::json!({ "data": rows })).into_response(),
        link,
    ))
}

async fn get_one(
    State(state): State<AppState>,
    user: ApiUser,
    Path((ledger_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ContactDto>, Problem> {
    require_access(&state.pool, user.0, ledger_id, false).await?;
    let row: Option<ContactDto> = sqlx::query_as(
        "SELECT id, ledger_id, name, email, phone, kind
         FROM contacts WHERE id = $1 AND ledger_id = $2",
    )
    .bind(id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_problem)?;
    row.map(Json)
        .ok_or_else(|| Problem::new(StatusCode::NOT_FOUND, "Not Found", "contact not found"))
}

async fn create(
    State(state): State<AppState>,
    user: ApiUser,
    Path(ledger_id): Path<Uuid>,
    Json(raw): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<ContactDto>), Problem> {
    require_access(&state.pool, user.0, ledger_id, true).await?;
    let body: CreateContactBody = serde_json::from_value(raw)
        .map_err(|e| Problem::new(StatusCode::BAD_REQUEST, "Bad Request", e.to_string()))?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "name is required",
        ));
    }
    if !matches!(body.kind.as_str(), "customer" | "vendor" | "both") {
        return Err(Problem::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "kind must be customer, vendor, or both",
        ));
    }
    let dto: ContactDto = sqlx::query_as(
        "INSERT INTO contacts (ledger_id, name, email, phone, kind)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, ledger_id, name, email, phone, kind",
    )
    .bind(ledger_id)
    .bind(name)
    .bind(body.email.as_deref().filter(|e| !e.is_empty()))
    .bind(body.phone.as_deref().filter(|p| !p.is_empty()))
    .bind(&body.kind)
    .fetch_one(&state.pool)
    .await
    .map_err(db_problem)?;
    Ok((StatusCode::CREATED, Json(dto)))
}

/// PATCH semantics: only provided fields change.
#[derive(Debug, Deserialize)]
pub struct UpdateContactBody {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub kind: Option<String>,
}

async fn update(
    State(state): State<AppState>,
    user: ApiUser,
    Path((ledger_id, id)): Path<(Uuid, Uuid)>,
    Json(raw): Json<serde_json::Value>,
) -> Result<Json<ContactDto>, Problem> {
    require_access(&state.pool, user.0, ledger_id, true).await?;
    let body: UpdateContactBody = serde_json::from_value(raw)
        .map_err(|e| Problem::new(StatusCode::BAD_REQUEST, "Bad Request", e.to_string()))?;
    if let Some(kind) = &body.kind {
        if !matches!(kind.as_str(), "customer" | "vendor" | "both") {
            return Err(Problem::new(
                StatusCode::BAD_REQUEST,
                "Bad Request",
                "invalid kind",
            ));
        }
    }
    let dto: Option<ContactDto> = sqlx::query_as(
        "UPDATE contacts SET
            name = COALESCE($3, name),
            email = COALESCE($4, email),
            phone = COALESCE($5, phone),
            kind = COALESCE($6, kind),
            updated_at = now()
         WHERE id = $1 AND ledger_id = $2
         RETURNING id, ledger_id, name, email, phone, kind",
    )
    .bind(id)
    .bind(ledger_id)
    .bind(
        body.name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(body.email.as_deref())
    .bind(body.phone.as_deref())
    .bind(body.kind.as_deref())
    .fetch_optional(&state.pool)
    .await
    .map_err(db_problem)?;
    dto.map(Json)
        .ok_or_else(|| Problem::new(StatusCode::NOT_FOUND, "Not Found", "contact not found"))
}
