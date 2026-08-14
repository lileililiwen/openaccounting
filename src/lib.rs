//! OpenAccounting library crate.
//!
//! This file is the composition root for both the production binary
//! (`src/main.rs`, a thin shell) and the integration tests under
//! `tests/`. Splitting the binary out into a library lets tests
//! drive the real `axum::Router` via [`build_router`] without
//! spawning a child process.
//!
//! See `openspec/specs/architecture/spec.md` for the rule that
//! `main.rs` is a process-startup shell and `build_router` is the
//! composition entry point.

#![forbid(unsafe_code)]

pub mod audit;
pub mod auth;
pub mod bank_feeds;
pub mod charts;
pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod handlers;
pub mod import;
pub mod ocr;
pub mod reports;
pub mod storage;
pub mod templates;
pub mod workers;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

use axum::{
    body::Body,
    http::{header, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use axum_login::{login_required, tower_sessions::SessionManagerLayer, AuthManagerLayerBuilder};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tower_sessions_sqlx_store::PostgresStore;

use auth::Backend;
use storage::FilesystemStore;

/// Serve `/static/sw.js` with the `Service-Worker-Allowed: /`
/// header set. The worker can then intercept any path under
/// `/`, not only its own `/static/` subtree. Returns 404 if the
/// file does not exist (e.g. asset wasn't deployed).
async fn serve_sw(path: std::path::PathBuf) -> Response {
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mut resp = (StatusCode::OK, bytes).into_response();
            let sw_allowed = HeaderName::from_static("service-worker-allowed");
            resp.headers_mut()
                .insert(sw_allowed, HeaderValue::from_static("/"));
            // Make sure intermediaries don't try to cache the
            // script with a stale version when we bump its
            // contents.
            resp.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            resp
        }
        Err(_) => (StatusCode::NOT_FOUND, Body::empty()).into_response(),
    }
}

/// Application state shared by every request handler.
#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub storage: FilesystemStore,
}

/// Configuration that controls the router's session / cookie layer.
///
/// `app_secret` MUST be at least 32 bytes; the binary rejects
/// shorter secrets at startup. The test fixture passes a fixed
/// 64-byte secret. Today the router does not sign cookies (the
/// `signed` feature on `tower-sessions` is not enabled), so the
/// secret is reserved for future use; the length invariant is
/// still enforced so the public surface is stable.
pub struct AppConfig {
    #[allow(dead_code)]
    pub app_secret: String,
}

impl AppConfig {
    /// Create a config from an env-like key/value pair. The secret
    /// is rejected if shorter than 32 bytes; this is the same
    /// invariant the production config layer enforces.
    pub fn new(app_secret: impl Into<String>) -> anyhow::Result<Self> {
        let app_secret = app_secret.into();
        if app_secret.len() < 32 {
            anyhow::bail!("APP_SECRET must be at least 32 characters");
        }
        Ok(Self { app_secret })
    }
}

/// Build the full axum router (public + protected routes, auth
/// layer, session layer, trace layer).
///
/// This is the single entry point used by both `main.rs` and the
/// `TestServer` fixture. Any new route must be added here, not in
/// `main.rs`.
pub fn build_router(state: AppState, _config: AppConfig) -> Router {
    let session_store = PostgresStore::new(state.pool.clone());
    // `SessionManagerLayer` migrations are run in `run()` and in
    // `TestServer::new()`. We don't re-run them here so the same
    // `Router` can be built many times cheaply.

    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_http_only(true)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_name("oa_session");

    let backend = Backend {
        pool: state.pool.clone(),
    };
    let auth_layer = AuthManagerLayerBuilder::new(backend, session_layer).build();

    // Resolve the static dir at compile time from the manifest
    // directory so the test binary (which is run from
    // `target/debug/deps/`) still finds the assets. The
    // STATIC_DIR env var overrides this for production deploys
    // that place static files elsewhere.
    let static_dir = std::env::var("STATIC_DIR")
        .unwrap_or_else(|_| format!("{}/static", env!("CARGO_MANIFEST_DIR")));

    // Sanity-check the static dir at boot so a typo fails the
    // test loudly instead of producing a 404 in every test that
    // touches an asset.
    let static_path = std::path::Path::new(&static_dir);
    if !static_path.exists() {
        tracing::error!("STATIC_DIR does not exist: {static_dir:?}; static assets will 404");
    }
    let static_path_for_dir = static_path.to_path_buf();
    // The service worker needs `Service-Worker-Allowed: /` so it
    // can intercept requests outside its own `/static/…` scope.
    // Serve that single file through a custom handler; let
    // ServeDir handle the rest of the static tree.
    let sw_path = static_path.join("sw.js");
    let public = Router::new()
        // Unauthenticated webhook endpoints (provider-signed).
        .route(
            "/ledgers/{id}/webhooks/plaid",
            post(handlers::bank_feeds::webhook_plaid),
        )
        .route("/", get(handlers::dashboard::redirect_to_first_ledger))
        .route(
            "/login",
            get(auth::handlers::login_page).post(auth::handlers::login_submit),
        )
        .route(
            "/register",
            get(auth::handlers::register_page).post(auth::handlers::register_submit),
        )
        .route(
            "/static/sw.js",
            get(move || {
                let path = sw_path.clone();
                async move { serve_sw(path).await }
            }),
        )
        .nest_service(
            "/static",
            tower_http::services::ServeDir::new(static_path_for_dir),
        );
    // `static_dir` is moved into the message above; mark it
    // as moved to silence the unused warning.
    let _ = static_dir;

    let protected = Router::new()
        .route("/ledgers", get(handlers::ledgers::list))
        .route(
            "/ledgers/new",
            get(handlers::ledgers::new_page).post(handlers::ledgers::create),
        )
        .route("/ledgers/{id}", get(handlers::ledgers::show))
        .route("/ledgers/{id}/dashboard", get(handlers::dashboard::show))
        .route("/account", get(handlers::account::show))
        .route(
            "/account/password",
            post(handlers::account::change_password),
        )
        .route("/ledgers/{id}/accounts", get(handlers::accounts::list))
        .route(
            "/ledgers/{id}/accounts/new",
            get(handlers::accounts::new_page).post(handlers::accounts::create),
        )
        .route(
            "/ledgers/{id}/transactions",
            get(handlers::transactions::list),
        )
        .route(
            "/ledgers/{id}/transactions/new",
            get(handlers::transactions::new_page).post(handlers::transactions::create),
        )
        .route(
            "/ledgers/{id}/transactions/{txn_id}",
            get(handlers::transactions::show),
        )
        .route(
            "/ledgers/{id}/transactions/{txn_id}/documents",
            post(handlers::documents::upload),
        )
        .route("/ledgers/{id}/documents", get(handlers::documents::list))
        .route(
            "/ledgers/{id}/documents/{doc_id}/download",
            get(handlers::documents::download),
        )
        .route(
            "/ledgers/{id}/documents/{doc_id}/delete",
            post(handlers::documents::delete),
        )
        .route(
            "/ledgers/{id}/documents/{doc_id}/ocr",
            get(handlers::document_ocr::show).post(handlers::document_ocr::run),
        )
        .route(
            "/ledgers/{id}/documents/{doc_id}/ocr/apply",
            post(handlers::document_ocr::apply),
        )
        .route("/ledgers/{id}/bank-feeds", get(handlers::bank_feeds::list))
        .route(
            "/ledgers/{id}/bank-feeds/link",
            get(handlers::bank_feeds::link_page).post(handlers::bank_feeds::link_submit),
        )
        .route(
            "/ledgers/{id}/bank-feeds/{link_id}/sync",
            post(handlers::bank_feeds::sync),
        )
        .route(
            "/ledgers/{id}/bank-feeds/{link_id}/unlink",
            post(handlers::bank_feeds::unlink),
        )
        .route("/ledgers/{id}/reports", get(handlers::reports::index))
        .route(
            "/ledgers/{id}/reports/trial-balance",
            get(handlers::reports::trial_balance),
        )
        .route(
            "/ledgers/{id}/reports/balance-sheet",
            get(handlers::reports::balance_sheet),
        )
        .route(
            "/ledgers/{id}/reports/income-statement",
            get(handlers::reports::income_statement),
        )
        .route(
            "/ledgers/{id}/reports/cash-flow",
            get(handlers::reports::cash_flow),
        )
        .route(
            "/ledgers/{id}/reports/cash-flow-forecast",
            get(handlers::reports::cash_flow_forecast),
        )
        .route(
            "/ledgers/{id}/reports/general-ledger",
            get(handlers::reports::general_ledger),
        )
        .route(
            "/ledgers/{id}/reports/export.csv",
            get(handlers::reports::export_csv),
        )
        .route(
            "/ledgers/{id}/close-year/{year}",
            post(handlers::closing::close_year),
        )
        .route("/ledgers/{id}/audit", get(handlers::audit::list))
        .route("/ledgers/{id}/contacts", get(handlers::contacts::list))
        .route(
            "/ledgers/{id}/contacts/new",
            get(handlers::contacts::new_page).post(handlers::contacts::create),
        )
        .route("/ledgers/{id}/invoices", get(handlers::invoices::list))
        .route(
            "/ledgers/{id}/invoices/new",
            get(handlers::invoices::new_page).post(handlers::invoices::create),
        )
        .route(
            "/ledgers/{id}/reports/ar-aging",
            get(handlers::aging::ar_aging),
        )
        .route(
            "/ledgers/{id}/reports/ap-aging",
            get(handlers::aging::ap_aging),
        )
        .route(
            "/ledgers/{id}/import",
            get(handlers::import::upload_page).post(handlers::import::upload),
        )
        .route(
            "/ledgers/{id}/import/confirm",
            post(handlers::import::confirm),
        )
        .route(
            "/ledgers/{id}/import/wechat",
            get(handlers::import_wechat::upload_page).post(handlers::import_wechat::preview),
        )
        .route(
            "/ledgers/{id}/import/wechat/commit",
            post(handlers::import_wechat::commit),
        )
        .route(
            "/ledgers/{id}/import/alipay",
            get(handlers::import_alipay::upload_page).post(handlers::import_alipay::preview),
        )
        .route(
            "/ledgers/{id}/import/alipay/commit",
            post(handlers::import_alipay::commit),
        )
        .route("/ledgers/{id}/templates", get(handlers::templates::list))
        .route("/ledgers/{id}/payments", get(handlers::payments::list))
        .route(
            "/ledgers/{id}/reconcile/{account_id}",
            get(handlers::reconciliation::page),
        )
        .route("/ledgers/{id}/taxes", get(handlers::taxes::list))
        .route("/ledgers/{id}/budgets", get(handlers::budgets::list))
        .route(
            "/ledgers/{id}/fixed-assets",
            get(handlers::fixed_assets::list),
        )
        .route("/ledgers/{id}/inventory", get(handlers::inventory::list))
        .route("/entities", get(handlers::entities::list))
        .route(
            "/entities/new",
            get(handlers::entities::new_page).post(handlers::entities::create),
        )
        .route(
            "/entities/{id}/consolidated",
            get(handlers::entities::consolidated),
        )
        .route(
            "/ledgers/{id}/inventory/new",
            get(handlers::inventory::new_page).post(handlers::inventory::create),
        )
        .route(
            "/ledgers/{id}/inventory/{item_id}/purchase",
            post(handlers::inventory::purchase),
        )
        .route(
            "/ledgers/{id}/inventory/{item_id}/adjust",
            post(handlers::inventory::adjust),
        )
        .route(
            "/ledgers/{id}/inventory/valuation",
            get(handlers::inventory::valuation),
        )
        .route(
            "/ledgers/{id}/fixed-assets/new",
            get(handlers::fixed_assets::new_page).post(handlers::fixed_assets::create),
        )
        .route(
            "/ledgers/{id}/fixed-assets/{asset_id}",
            get(handlers::fixed_assets::show),
        )
        .route(
            "/ledgers/{id}/fixed-assets/{asset_id}/depreciate",
            post(handlers::fixed_assets::calculate_depreciation),
        )
        .route(
            "/ledgers/{id}/fixed-assets/{asset_id}/dispose",
            post(handlers::fixed_assets::dispose),
        )
        .route("/admin/backups", get(handlers::backups::list))
        .route(
            "/admin/backups/create",
            post(handlers::backups::create_manual),
        )
        .route(
            "/admin/backups/{id}/download",
            get(handlers::backups::download),
        )
        .route("/admin/integrity", get(handlers::backups::integrity_check))
        .route(
            "/ledgers/{id}/budgets/new",
            get(handlers::budgets::new_page).post(handlers::budgets::create),
        )
        .route(
            "/ledgers/{id}/budgets/{budget_id}/delete",
            post(handlers::budgets::delete),
        )
        .route(
            "/ledgers/{id}/budgets/report",
            get(handlers::budgets::report),
        )
        .route(
            "/ledgers/{id}/taxes/new",
            get(handlers::taxes::new_page).post(handlers::taxes::create),
        )
        .route(
            "/ledgers/{id}/taxes/{rate_id}/toggle",
            post(handlers::taxes::toggle),
        )
        .route("/ledgers/{id}/taxes/report", get(handlers::taxes::report))
        .route(
            "/ledgers/{id}/taxes/export.csv",
            get(handlers::taxes::export_csv),
        )
        .route(
            "/ledgers/{id}/reconcile/{account_id}/import",
            post(handlers::reconciliation::upload_csv),
        )
        .route(
            "/ledgers/{id}/reconcile/{account_id}/match",
            post(handlers::reconciliation::match_line),
        )
        .route(
            "/ledgers/{id}/reconcile/{account_id}/exclude/{line_id}",
            post(handlers::reconciliation::exclude_line),
        )
        .route(
            "/ledgers/{id}/reconcile/{account_id}/complete",
            post(handlers::reconciliation::complete),
        )
        .route(
            "/ledgers/{id}/reconcile/{account_id}/history",
            get(handlers::reconciliation::history),
        )
        .route(
            "/ledgers/{id}/payments/new",
            get(handlers::payments::new_page).post(handlers::payments::create),
        )
        .route(
            "/ledgers/{id}/payments/register",
            get(handlers::payments::register),
        )
        .route(
            "/ledgers/{id}/templates/new",
            get(handlers::templates::new_page).post(handlers::templates::create),
        )
        .route(
            "/ledgers/{id}/templates/{template_id}",
            get(handlers::templates::show),
        )
        .route(
            "/ledgers/{id}/templates/{template_id}/toggle",
            post(handlers::templates::toggle),
        )
        .route(
            "/ledgers/{id}/templates/{template_id}/delete",
            post(handlers::templates::delete),
        )
        .route(
            "/ledgers/{id}/templates/{template_id}/run",
            post(handlers::templates::run),
        )
        .route("/ledgers/{id}/share", get(handlers::sharing::page))
        .route(
            "/ledgers/{id}/share/invite",
            post(handlers::sharing::invite),
        )
        .route(
            "/ledgers/{id}/share/remove/{member_id}",
            post(handlers::sharing::remove_member),
        )
        .route("/ledgers/{id}/rules", get(handlers::rules::list))
        .route("/ledgers/{id}/rules/new", get(handlers::rules::new_page))
        .route("/ledgers/{id}/rules", post(handlers::rules::create))
        .route(
            "/ledgers/{id}/rules/{rule_id}/toggle",
            post(handlers::rules::toggle),
        )
        .route(
            "/ledgers/{id}/rules/{rule_id}/delete",
            post(handlers::rules::delete),
        )
        .route(
            "/ledgers/{id}/reimbursements",
            get(handlers::reimbursement::list),
        )
        .route(
            "/ledgers/{id}/reimbursements/new",
            get(handlers::reimbursement::new_page),
        )
        .route(
            "/ledgers/{id}/reimbursements",
            post(handlers::reimbursement::create),
        )
        .route(
            "/ledgers/{id}/reimbursements/{claim_id}",
            get(handlers::reimbursement::show),
        )
        .route(
            "/ledgers/{id}/reimbursements/{claim_id}/lines",
            post(handlers::reimbursement::add_line),
        )
        .route(
            "/ledgers/{id}/reimbursements/{claim_id}/submit",
            post(handlers::reimbursement::submit),
        )
        .route(
            "/ledgers/{id}/reimbursements/{claim_id}/approve",
            post(handlers::reimbursement::approve),
        )
        .route(
            "/ledgers/{id}/reimbursements/{claim_id}/reject",
            post(handlers::reimbursement::reject),
        )
        .route(
            "/ledgers/{id}/reimbursements/{claim_id}/pay",
            post(handlers::reimbursement::pay),
        )
        .route(
            "/ledgers/{id}/approval-policies",
            get(handlers::approval_policies::list),
        )
        .route(
            "/ledgers/{id}/approval-policies/new",
            get(handlers::approval_policies::new_page),
        )
        .route(
            "/ledgers/{id}/approval-policies",
            post(handlers::approval_policies::create),
        )
        .route(
            "/ledgers/{id}/approval-policies/{policy_id}/delete",
            post(handlers::approval_policies::delete),
        )
        .route("/invitations/{id}/accept", post(handlers::sharing::accept))
        .route(
            "/invitations/{id}/decline",
            post(handlers::sharing::decline),
        )
        .route("/logout", post(auth::handlers::logout))
        .merge(handlers::admin::admin_routes())
        .route_layer(login_required!(
            Backend,
            login_url = "/login",
            redirect_field = "next"
        ));

    public
        .merge(protected)
        .with_state(state)
        .layer(auth_layer)
        .layer(tower_http::trace::TraceLayer::new_for_http())
}

/// Process startup: load config, connect, migrate, serve HTTP.
pub async fn run() -> anyhow::Result<()> {
    let cfg = config::Config::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(16)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&cfg.database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let storage = FilesystemStore::new(&cfg.documents_dir).await?;

    // Session store has its own schema; install it before the
    // first request lands.
    let session_store = PostgresStore::new(pool.clone());
    session_store.migrate().await?;

    let state = AppState {
        pool: pool.clone(),
        storage,
    };

    // Start the background bank-feed sync worker.
    let sync_interval = std::env::var("BANK_FEEDS_SYNC_INTERVAL_HOURS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(6);
    tokio::spawn(workers::sync::run_sync_loop(state.clone(), sync_interval));

    let app_config = AppConfig::new(cfg.app_secret)?;
    let app = build_router(state, app_config);

    let addr =
        std::net::SocketAddr::from((cfg.app_host.parse::<std::net::IpAddr>()?, cfg.app_port));
    tracing::info!("OpenAccounting listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
