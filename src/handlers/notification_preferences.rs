//! HTTP handlers for notification preferences
//! (`u6-notification-preferences`).
//!
//! Routes:
//!   GET  /account/notifications  — render the grid
//!   POST /account/notifications  — upsert one toggle

use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use axum_login::AuthSession;
use serde::Deserialize;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    notifications::preferences::{self, Channel, Event},
    templates::notification_preferences::PreferencesPage,
    AppState,
};

/// GET /account/notifications — render the grid for the
/// signed-in user. Each cell shows the persisted value, or the
/// default when no row is present.
pub async fn show(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;

    // One bool per (event, channel) pair in row-major order:
    // event 0 (in_app, email, push), event 1, ...
    let mut cells: Vec<bool> = Vec::with_capacity(12);
    for &ev in &[
        Event::BudgetOverrun,
        Event::ReimbursementSubmitted,
        Event::LargeTransaction,
        Event::WeeklySummary,
    ] {
        for &ch in &[Channel::InApp, Channel::Email, Channel::Push] {
            let enabled = preferences::is_enabled(&state.pool, user.id, ch, ev).await?;
            cells.push(enabled);
        }
    }

    let page = PreferencesPage::build(user.id, user.username.clone(), user.role.clone(), cells);
    Ok(crate::templates::render_response(page))
}

#[derive(Deserialize)]
pub struct ToggleForm {
    pub channel: String,
    pub event: String,
    pub enabled: String,
}

/// POST /account/notifications — upsert one (channel, event)
/// toggle. `enabled` accepts "on" / "off" / "true" / "false"
/// so the HTML checkbox convention works without JS.
pub async fn toggle(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Form(form): Form<ToggleForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let channel = Channel::parse(&form.channel)
        .ok_or_else(|| AppError::Validation(format!("invalid channel: {}", form.channel)))?;
    let event = Event::parse(&form.event)
        .ok_or_else(|| AppError::Validation(format!("invalid event: {}", form.event)))?;
    let enabled = matches!(
        form.enabled.to_ascii_lowercase().as_str(),
        "on" | "true" | "1"
    );

    preferences::set_enabled(&state.pool, user.id, channel, event, enabled).await?;

    Ok(Redirect::to("/account/notifications").into_response())
}
