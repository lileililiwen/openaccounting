# Add Test Infrastructure — Design

## Module layout

The binary today is `src/main.rs` and declares every module
inline. The refactor splits it:

```
src/
├── main.rs                 # thin shell, calls into lib::run()
├── lib.rs                  # pub mod auth; pub mod handlers; …
└── test_support.rs         # feature = "test-support", TestDb + TestServer
```

`src/lib.rs` exports:

```rust
pub mod auth;
pub mod audit;
pub mod charts;
pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod handlers;
pub mod reports;
pub mod storage;
pub mod templates;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

use axum::Router;

pub struct AppState {
    pub pool: sqlx::PgPool,
    pub storage: storage::FilesystemStore,
}

pub struct AppConfig {
    pub app_secret: String,
}

/// Build the full axum router (public + protected + auth layers).
/// Used by both `main.rs` and `TestServer::new()`.
pub fn build_router(state: AppState, config: AppConfig) -> Router { … }

pub async fn run() -> anyhow::Result<()> { … } // for main.rs
```

## `TestDb` design

```rust
pub struct TestDb {
    admin_url: String,        // e.g. postgres://u:p@h:port/postgres
    test_db_name: String,     // e.g. oa_test_<uuid-no-dashes>
    pool: sqlx::PgPool,
}

impl TestDb {
    pub async fn new() -> Self { … }
    pub fn pool(&self) -> &sqlx::PgPool { &self.pool }
    pub fn name(&self) -> &str { &self.test_db_name }
    pub fn url(&self) -> String {
        // Replace the database in DATABASE_URL with the test name.
        let mut url = self.admin_url.clone();
        // … strip the trailing /<db> and append /<test_db_name>
        url
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        // Spawn a blocking task that connects to the admin URL and
        // runs `DROP DATABASE IF EXISTS <name>` with
        // `FORCE` + `WITH (FORCE)` so it works even if connections
        // are still open. The async runtime is shut down by tokio
        // before `Drop` runs, so we use a `std::thread::spawn`
        // and a sync `postgres` crate call.
    }
}
```

For `Drop`, we use the `postgres` crate's synchronous client to
avoid fighting with the tokio runtime. It is a new dev-dependency.

### `DATABASE_URL` parsing

`TestDb::new()` derives the admin URL by swapping the path segment
to `postgres`:

```
DATABASE_URL=postgres://openaccounting:openaccounting@localhost:5436/openaccounting
            → admin_url=postgres://openaccounting:openaccounting@localhost:5436/postgres
```

The test database name is `oa_test_<uuid-without-dashes>`. The
unique name guarantees parallel-safe tests even though
`cargo test` defaults to multi-threaded (we recommend
`--test-threads=1` in CI).

### Migration

`TestDb::new()` calls `sqlx::migrate!("./migrations").run(&pool)`
on the per-test pool, exactly the same as `main.rs` does on
startup. So the fixture is a faithful model of production.

## `TestServer` design

```rust
pub struct TestServer {
    base_url: String,        // http://127.0.0.1:<random>
    client: reqwest::Client,  // with cookie_store(true), no redirect
    _db: TestDb,
    _handle: tokio::task::JoinHandle<()>,
    _sandbox: tempfile::TempDir, // for DOCUMENTS_DIR
}

impl TestServer {
    pub async fn new() -> Self { … }
    pub fn base_url(&self) -> &str { &self.base_url }
    pub fn client(&self) -> &reqwest::Client { &self.client }
    pub fn db(&self) -> &TestDb { &self._db }

    /// Register, log in, return the session cookie value. The
    /// caller is responsible for setting the `Cookie` header
    /// (the helper returns the full `Set-Cookie` value the
    /// server sent, which is just `oa_session=<value>`).
    pub async fn bootstrap_user(
        &self, username: &str, email: &str, password: &str,
    ) -> String { … }
}
```

### Router entry point

`build_router(state, config)` builds the same router
`main.rs` does today (public + protected + auth layer +
TraceLayer). The only differences:

- `APP_SECRET` comes from `config.app_secret`, not from
  `Config::from_env()`. The caller picks the secret.
- `with_secure(false)` on the session layer (so cookies work over
  HTTP, not just HTTPS).
- Same `with_http_only(true)` and `SameSite::Lax` as today.

### Random port bind

The server binds `127.0.0.1:0` and reads back the assigned port.
The `JoinHandle` is aborted on `Drop`.

### Cookies

`reqwest::Client::builder().cookie_store(true).build()` keeps
cookies in memory. `bootstrap_user` returns the captured
`Set-Cookie` header value as a string the test can pass back
into a `Cookie: oa_session=…` header for explicit tests that
need to control the session.

## `tests/` layout

```
tests/
├── common/
│   └── mod.rs            # pub use openaccounting::test_support::*;
└── integration/
    ├── main.rs           # entry: [[path]] mod common; mod smoke;
    └── smoke.rs          # 2 tests: server boots, public routes
```

`Cargo.toml`:

```toml
[[test]]
name = "integration"
path = "tests/integration/main.rs"

[features]
test-support = []

[dev-dependencies]
reqwest = { version = "0.12", default-features = false,
            features = ["json", "rustls-tls", "cookies"] }
tempfile = "3"
postgres = "0.19"     # for Drop
```

`tests/integration/main.rs`:

```rust
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used,
                         clippy::panic))]

#[path = "../common/mod.rs"]
mod common;

mod smoke;
```

## Tests

### Unit (in `src/test_support.rs`)

- `testdb_creates_unique_db_names` — two `TestDb::new()` calls
  produce different names; dropping one does not affect the
  other.
- `testdb_runs_all_migrations` — `SELECT COUNT(*) FROM ledgers`
  returns 0; `SELECT COUNT(*) FROM users` returns 0; all
  18 migrations are recorded in `_sqlx_migrations`.

### Integration (in `tests/integration/smoke.rs`)

- `smoke_server_boots_and_login_page_responds` — `GET /login`
  returns 200 with a form on it.
- `smoke_register_and_login_round_trip` — POST `/register`, then
  POST `/login`, then GET `/ledgers` returns 200 (proves the
  session cookie works through the full stack).
- `smoke_static_assets_served` — GET `/static/htmx.min.js`
  returns 200 (proves the `ServeDir` is wired).
