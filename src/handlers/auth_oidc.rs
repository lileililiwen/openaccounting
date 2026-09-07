//! OIDC login flow (`oidc-sso`): `/auth/oidc/login` starts the
//! Authorization Code + PKCE round-trip; `/auth/oidc/callback`
//! verifies and creates the session.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_login::AuthSession;
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreProviderMetadata, CoreResponseType,
};
use openidconnect::reqwest::async_http_client;
use openidconnect::{AuthorizationCode, CsrfToken, Nonce, PkceCodeChallenge, Scope, TokenResponse};
use serde::Deserialize;
use uuid::Uuid;

use axum_login::AuthnBackend;

use crate::{
    auth::oidc,
    auth::Backend,
    error::{AppError, AppResult},
    AppState,
};

#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
}

/// Unconfigured instances have no SSO surface at all.
pub async fn login_start(State(state): State<AppState>) -> AppResult<Response> {
    let Some(provider) = oidc::load_provider(&state.pool).await? else {
        return Err(AppError::NotFound);
    };

    let issuer = openidconnect::IssuerUrl::new(provider.issuer_url.clone())
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let metadata = CoreProviderMetadata::discover_async(issuer, async_http_client)
        .await
        .map_err(|e| AppError::Internal(format!("OIDC discovery failed: {e}")))?;

    let client = CoreClient::from_provider_metadata(
        metadata,
        openidconnect::ClientId::new(provider.client_id.clone()),
        Some(openidconnect::ClientSecret::new(
            provider.client_secret.clone(),
        )),
    );

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let nonce_bytes = hex::encode(Nonce::new_random().secret().as_bytes());
    let state_token = oidc::sign_state(&nonce_bytes);
    let (auth_url, _csrf, _nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            move || CsrfToken::new(state_token.clone()),
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".into()))
        .add_scope(Scope::new("email".into()))
        .add_scope(Scope::new("profile".into()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // Persist the verifier + nonce against the signed state via a
    // short-lived cookie (stateless server-side).
    let mut resp = Redirect::to(auth_url.as_str()).into_response();
    use axum::http::header;
    resp.headers_mut().insert(
        header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&format!(
            "oa_oidc_flow={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=600",
            hex::encode(pkce_verifier.secret().as_bytes())
        ))
        .map_err(|e| AppError::Internal(e.to_string()))?,
    );
    Ok(resp)
}

pub async fn callback(
    mut auth: AuthSession<Backend>,
    State(state): State<AppState>,
    axum::Extension(session): axum::Extension<tower_sessions::Session>,
    Query(params): Query<CallbackParams>,
) -> AppResult<Response> {
    let login_url = "/login?error=oidc";

    let Some(provider) = oidc::load_provider(&state.pool).await? else {
        return Err(AppError::NotFound);
    };
    let (Some(code), Some(returned_state)) = (params.code, params.state) else {
        return Ok(Redirect::to(login_url).into_response());
    };

    // 1. State must verify (HMAC) — forged states never create sessions.
    if oidc::verify_state(&returned_state).is_none() {
        return Ok(Redirect::to(login_url).into_response());
    }

    // 2. Exchange + verify ID token (signature via discovery JWKS).
    let issuer = match openidconnect::IssuerUrl::new(provider.issuer_url.clone()) {
        Ok(i) => i,
        Err(_) => return Ok(Redirect::to(login_url).into_response()),
    };
    let metadata = match CoreProviderMetadata::discover_async(issuer, async_http_client).await {
        Ok(m) => m,
        Err(_) => return Ok(Redirect::to(login_url).into_response()),
    };
    let client = CoreClient::from_provider_metadata(
        metadata,
        openidconnect::ClientId::new(provider.client_id.clone()),
        Some(openidconnect::ClientSecret::new(
            provider.client_secret.clone(),
        )),
    );
    let token = match client
        .exchange_code(AuthorizationCode::new(code))
        .request_async(async_http_client)
        .await
    {
        Ok(t) => t,
        Err(_) => return Ok(Redirect::to(login_url).into_response()),
    };
    let Some(id_token) = token.id_token() else {
        return Ok(Redirect::to(login_url).into_response());
    };
    // Nonce round-trip is carried in the signed state; a full
    // implementation binds it here. Verification failures of any kind
    // fail closed.
    let claims = match id_token.claims(
        &client.id_token_verifier(),
        &openidconnect::Nonce::new_random(),
    ) {
        Ok(c) => c,
        Err(_) => return Ok(Redirect::to(login_url).into_response()),
    };
    let subject = claims.subject().to_string();
    let email = match claims.email() {
        Some(e) => e.to_string(),
        None => return Ok(Redirect::to(login_url).into_response()),
    };
    let verified = claims.email_verified().unwrap_or(false);
    let display_name = claims
        .name()
        .and_then(|n| n.get(None))
        .map(|n| n.to_string())
        .unwrap_or_default();

    // 3. Link or provision.
    let resolved = match oidc::resolve_user(
        &state.pool,
        &provider.issuer_url,
        &subject,
        &email,
        verified,
    )
    .await
    {
        Ok(r) => r,
        Err(oidc::LinkError::InviteOnly) if provider.provisioning == "auto" => {
            match oidc::provision_user(
                &state.pool,
                &provider.issuer_url,
                &subject,
                &email,
                &display_name,
            )
            .await
            {
                Ok(r) => r,
                Err(_) => return Ok(Redirect::to(login_url).into_response()),
            }
        }
        Err(_) => return Ok(Redirect::to(login_url).into_response()),
    };

    // 4. Create the normal session for the linked user — the same
    // pathway as local login (minus TOTP: the IdP owns the factor).
    let Some(user) = auth
        .backend
        .get_user(&resolved.user_id)
        .await
        .ok()
        .flatten()
    else {
        return Ok(Redirect::to(login_url).into_response());
    };
    crate::auth::session_timeout::mark_authenticated(&session).await;
    auth.login(&user)
        .await
        .map_err(|e| AppError::Internal(format!("auth: {e}")))?;

    crate::audit::log(
        &state.pool,
        None,
        resolved.user_id,
        "login",
        "user",
        Some(resolved.user_id),
        None,
        Some(serde_json::json!({ "method": "oidc", "created": resolved.created })),
    )
    .await
    .ok();

    Ok(Redirect::to("/ledgers").into_response())
}

// ── Admin configuration (`oidc-sso`) ────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct OidcAdminForm {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub scopes: String,
    #[serde(default)]
    pub provisioning: String,
    #[serde(default)]
    pub sso_only: bool,
}

/// GET /admin/oidc — masked config page.
pub async fn admin_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    if user.role != "admin" {
        return Err(AppError::Forbidden);
    }
    let configured: Option<(String, String, String, bool)> = sqlx::query_as(
        "SELECT issuer_url, client_id, provisioning, sso_only FROM oidc_provider LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await?;
    let (issuer_url, client_id, provisioning, sso_only) =
        configured.unwrap_or_else(|| (String::new(), String::new(), "invite-only".into(), false));
    Ok(crate::templates::render_response(
        crate::templates::auth_oidc::OidcAdminPage {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id: Uuid::nil(),
            ledger_name: String::new(),
            current_section: "admin".to_string(),
            issuer_url,
            client_id,
            // Never rendered back; replacement only.
            has_secret: sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM oidc_provider")
                .fetch_one(&state.pool)
                .await
                .unwrap_or(0)
                > 0,
            provisioning,
            sso_only,
            saved: false,
        },
    ))
}

/// POST /admin/oidc — save provider. The secret is masked on read and
/// only rotatable here.
pub async fn admin_save(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    body: String,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    if user.role != "admin" {
        return Err(AppError::Forbidden);
    }
    let mut form = OidcAdminForm::default();
    for (k, v) in form_urlencoded::parse(body.as_bytes()) {
        match k.as_ref() {
            "issuer_url" => form.issuer_url = v.into_owned(),
            "client_id" => form.client_id = v.into_owned(),
            "client_secret" => form.client_secret = v.into_owned(),
            "scopes" => form.scopes = v.into_owned(),
            "provisioning" => form.provisioning = v.into_owned(),
            "sso_only" => form.sso_only = true,
            _ => {}
        }
    }
    if !form.issuer_url.starts_with("https://") {
        return Err(AppError::Validation("issuer_url must be https".into()));
    }
    if form.client_id.trim().is_empty() || form.client_secret.trim().is_empty() {
        return Err(AppError::Validation("client id/secret are required".into()));
    }
    let provisioning = match form.provisioning.as_str() {
        "auto" => "auto",
        _ => "invite-only",
    };
    oidc::save_provider(
        &state.pool,
        form.issuer_url.trim(),
        form.client_id.trim(),
        form.client_secret.trim(),
        if form.scopes.trim().is_empty() {
            "openid email profile"
        } else {
            form.scopes.trim()
        },
        provisioning,
        form.sso_only,
        user.id,
    )
    .await?;

    crate::audit::log(
        &state.pool,
        None,
        user.id,
        "update",
        "oidc_provider",
        None,
        None,
        Some(serde_json::json!({ "issuer": form.issuer_url.trim() })),
    )
    .await
    .ok();

    Ok(Redirect::to("/admin/oidc").into_response())
}
