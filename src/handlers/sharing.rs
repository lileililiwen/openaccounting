use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::sharing::SharePage,
    AppState,
};

#[derive(Deserialize)]
pub struct InviteMemberForm {
    pub email: String,
    pub role: String,
}

pub async fn page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let members = sqlx::query_as::<_, (Uuid, String, String, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT lm.user_id, u.username, lm.role, lm.created_at
           FROM ledger_members lm
           JOIN users u ON u.id = lm.user_id
           WHERE lm.ledger_id = $1
           ORDER BY lm.created_at"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    let invitations = sqlx::query_as::<_, (String, String, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT invitee_email, role, created_at
           FROM ledger_invitations
           WHERE ledger_id = $1 AND status = 'pending'
           ORDER BY created_at"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(render_response(SharePage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        members,
        invitations,
        error: String::new(),
    }))
}

pub async fn invite(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<InviteMemberForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    let email = form.email.trim().to_lowercase();
    if email.is_empty() {
        return Ok(render_response(SharePage {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name,
            members: Vec::new(),
            invitations: Vec::new(),
            error: "Email is required".into(),
        }));
    }

    let role = match form.role.as_str() {
        "editor" | "viewer" => form.role.clone(),
        _ => {
            return Ok(render_response(SharePage {
                user_id: user.id,
                username: user.username.clone(),
                user_role: user.role.clone(),
                ledger_id,
                ledger_name: ledger.name,
                members: Vec::new(),
                invitations: Vec::new(),
                error: "Invalid role".into(),
            }));
        }
    };

    // Check if already a member
    let is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ledger_members WHERE ledger_id = $1 AND user_id = (SELECT id FROM users WHERE email = $2))",
    )
    .bind(ledger_id)
    .bind(&email)
    .fetch_one(&state.pool)
    .await?;

    if is_member {
        return Ok(render_response(SharePage {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: ledger.name,
            members: Vec::new(),
            invitations: Vec::new(),
            error: "User is already a member".into(),
        }));
    }

    // Create invitation
    sqlx::query(
        r#"INSERT INTO ledger_invitations (ledger_id, inviter_id, invitee_email, role)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (ledger_id, invitee_email) DO UPDATE SET role = $4, status = 'pending', expires_at = now() + INTERVAL '7 days'"#,
    )
    .bind(ledger_id)
    .bind(user.id)
    .bind(&email)
    .bind(&role)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "invite",
        "member",
        None,
        None,
        Some(serde_json::json!({
            "email": email,
            "role": role
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/share", ledger_id)).into_response())
}

pub async fn accept(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(invitation_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    let invitation = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"SELECT ledger_id, role, status FROM ledger_invitations WHERE id = $1 AND invitee_email = $2"#,
    )
    .bind(invitation_id)
    .bind(&user.email)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    if invitation.2 != "pending" {
        return Err(AppError::Validation("Invitation is not pending".into()));
    }

    let ledger_id = invitation.0;
    let role = invitation.1;

    // Add as member
    sqlx::query(
        r#"INSERT INTO ledger_members (ledger_id, user_id, role)
           VALUES ($1, $2, $3)
           ON CONFLICT (ledger_id, user_id) DO UPDATE SET role = $3"#,
    )
    .bind(ledger_id)
    .bind(user.id)
    .bind(&role)
    .execute(&state.pool)
    .await?;

    // Update invitation status
    sqlx::query("UPDATE ledger_invitations SET status = 'accepted' WHERE id = $1")
        .bind(invitation_id)
        .execute(&state.pool)
        .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "accept_invitation",
        "member",
        None,
        None,
        Some(serde_json::json!({
            "role": role
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}", ledger_id)).into_response())
}

pub async fn decline(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(invitation_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    sqlx::query(
        "UPDATE ledger_invitations SET status = 'declined' WHERE id = $1 AND invitee_email = $2",
    )
    .bind(invitation_id)
    .bind(&user.email)
    .execute(&state.pool)
    .await?;

    Ok(Redirect::to("/ledgers").into_response())
}

pub async fn remove_member(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, member_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;

    sqlx::query("DELETE FROM ledger_members WHERE ledger_id = $1 AND user_id = $2")
        .bind(ledger_id)
        .bind(member_id)
        .execute(&state.pool)
        .await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "remove_member",
        "member",
        None,
        None,
        Some(serde_json::json!({
            "member_id": member_id
        })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{}/share", ledger_id)).into_response())
}
