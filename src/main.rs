#![forbid(unsafe_code)]

use axum::{
    routing::{get, post},
    Router,
};
use axum_login::{login_required, tower_sessions::SessionManagerLayer, AuthManagerLayerBuilder};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tower_sessions_sqlx_store::PostgresStore;

mod auth;
mod audit;
mod charts;
mod config;
mod db;
mod domain;
mod error;
mod handlers;
mod reports;
mod storage;
mod templates;

use auth::Backend;
use config::Config;
use storage::FilesystemStore;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub storage: FilesystemStore,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,openaccounting=debug,sqlx=warn".into()),
        )
        .init();

    let cfg = Config::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(16)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&cfg.database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let storage = FilesystemStore::new(&cfg.documents_dir).await?;

    let session_store = PostgresStore::new(pool.clone());
    session_store.migrate().await?;
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // dev; flip in prod
        .with_http_only(true)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_name("oa_session");

    let backend = Backend { pool: pool.clone() };
    let auth_layer = AuthManagerLayerBuilder::new(backend, session_layer).build();

    let state = AppState {
        pool: pool.clone(),
        storage,
    };

    let public = Router::new()
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
            "/static/{*path}",
            axum::routing::get_service(tower_http::services::ServeDir::new("static")),
        );

    let protected = Router::new()
        .route("/ledgers", get(handlers::ledgers::list))
        .route(
            "/ledgers/new",
            get(handlers::ledgers::new_page).post(handlers::ledgers::create),
        )
        .route("/ledgers/{id}", get(handlers::ledgers::show))
        .route("/ledgers/{id}/dashboard", get(handlers::dashboard::show))
        // Self-service account page
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
        .route(
            "/ledgers/{id}/audit",
            get(handlers::audit::list),
        )
        .route("/logout", post(auth::handlers::logout))
        // Admin routes (require admin role)
        .merge(handlers::admin::admin_routes())
        .route_layer(login_required!(Backend));

    let app = public
        .merge(protected)
        .with_state(state)
        .layer(auth_layer)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let addr =
        std::net::SocketAddr::from((cfg.app_host.parse::<std::net::IpAddr>()?, cfg.app_port));
    tracing::info!("OpenAccounting listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
