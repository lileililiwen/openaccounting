use std::env;

/// Runtime environment. Drives cookie security policy and other
/// production-vs-development defaults per
/// `openspec/changes/s5-secure-cookie-enforcement/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppEnv {
    Production,
    Staging,
    Development,
    Test,
}

impl AppEnv {
    /// Parse from the `APP_ENV` env var. Empty / unset →
    /// [`AppEnv::Development`]. Unknown → `Err`.
    pub fn from_env() -> anyhow::Result<Self> {
        let raw = env::var("APP_ENV").unwrap_or_default();
        Self::from_str(&raw)
    }

    /// Parse a literal. Lower-cased; empty → Development.
    pub fn from_str(raw: &str) -> anyhow::Result<Self> {
        match raw.trim().to_lowercase().as_str() {
            "" => Ok(AppEnv::Development),
            "production" | "prod" => Ok(AppEnv::Production),
            "staging" | "stage" => Ok(AppEnv::Staging),
            "development" | "dev" => Ok(AppEnv::Development),
            "test" => Ok(AppEnv::Test),
            other => anyhow::bail!(
                "APP_ENV={other:?} is not a recognised value \
                 (allowed: production, staging, development, test)"
            ),
        }
    }

    /// True for environments where the session cookie MUST be
    /// marked `Secure` (i.e. only served over HTTPS).
    pub fn requires_secure_cookie(&self) -> bool {
        matches!(self, AppEnv::Production | AppEnv::Staging)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AppEnv::Production => "production",
            AppEnv::Staging => "staging",
            AppEnv::Development => "development",
            AppEnv::Test => "test",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub app_host: String,
    pub app_port: u16,
    pub app_secret: String,
    /// Previous APP_SECRET for cookie-signing key rotation.
    /// When set, sessions signed with this secret are still
    /// accepted; on the next response they are re-signed with
    /// the current `app_secret`. Documented in the
    /// `signed-cookies` spec.
    pub app_secret_previous: Option<String>,
    pub database_url: String,
    pub documents_dir: String,
    pub app_env: AppEnv,
    /// Whether the Prometheus `/metrics` endpoint is registered
    /// (`o4-metrics-endpoint`). Parsed from `METRICS_ENABLED`
    /// (default: true).
    pub metrics_enabled: bool,
    /// Maximum upload body size in bytes
    /// (`s10-upload-validation`). Parsed from
    /// `UPLOAD_MAX_BYTES`; defaults to 25 MiB. Enforced at the
    /// router layer via `tower_http::limit::DefaultBodyLimit` so
    /// over-cap requests are rejected with HTTP 413 before the
    /// handler reads any body bytes.
    pub upload_max_bytes: usize,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let _ = dotenvy::dotenv();

        let app_host = env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".into());
        let app_port = env::var("APP_PORT")
            .unwrap_or_else(|_| "3000".into())
            .parse::<u16>()?;
        let app_secret = env::var("APP_SECRET")
            .unwrap_or_else(|_| "dev-only-secret-please-change-in-production-64-chars-min".into());
        if app_secret.len() < 32 {
            anyhow::bail!("APP_SECRET must be at least 32 characters");
        }
        let app_secret_previous = env::var("APP_SECRET_PREVIOUS")
            .ok()
            .filter(|s| !s.is_empty() && s.len() >= 32);
        let database_url =
            env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?;
        // `d5-sqlite-option`: accept `sqlite://<path>` and refuse
        // anything other than Postgres or SQLite.
        match database_url_scheme(&database_url) {
            DbScheme::Postgres | DbScheme::Sqlite => {}
            DbScheme::Other(s) => {
                anyhow::bail!(
                    "DATABASE_URL must use the `postgres://` or `sqlite://` scheme (got {s}://)"
                );
            }
        }
        let documents_dir = env::var("DOCUMENTS_DIR").unwrap_or_else(|_| "./data/documents".into());
        let app_env = AppEnv::from_env()?;
        let metrics_enabled = env::var("METRICS_ENABLED")
            .map(|v| !v.eq_ignore_ascii_case("false") && v != "0")
            .unwrap_or(true);
        let upload_max_bytes = env::var("UPLOAD_MAX_BYTES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(crate::upload::DEFAULT_MAX_BYTES);
        if upload_max_bytes == 0 {
            anyhow::bail!("UPLOAD_MAX_BYTES must be > 0");
        }

        Ok(Self {
            app_host,
            app_port,
            app_secret,
            app_secret_previous,
            database_url,
            documents_dir,
            app_env,
            metrics_enabled,
            upload_max_bytes,
        })
    }
}

/// Supported `DATABASE_URL` schemes (`d5-sqlite-option`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DbScheme {
    Postgres,
    Sqlite,
    Other(&'static str),
}

/// Detect the database scheme from a URL. Anything that does
/// not match `postgres://`, `postgresql://`, or `sqlite://`
/// returns `Other`.
pub fn database_url_scheme(url: &str) -> DbScheme {
    if let Some(rest) = url.strip_prefix("postgres://") {
        let _ = rest;
        DbScheme::Postgres
    } else if let Some(_) = url.strip_prefix("postgresql://") {
        DbScheme::Postgres
    } else if url.starts_with("sqlite://") {
        DbScheme::Sqlite
    } else if let Some(idx) = url.find("://") {
        // Map unknown schemes into `Other` without leaking the
        // rest of the URL (which may carry credentials).
        let scheme = &url[..idx];
        // Const-friendly lifetime: `&'static str` lifetime
        // requires a string that lives forever. We leak a small
        // boxed slice for each unknown scheme we see; this is
        // called once per process so the leak is negligible.
        let leaked: &'static str = Box::leak(scheme.to_string().into_boxed_str());
        DbScheme::Other(leaked)
    } else {
        DbScheme::Other("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_rejects_unknown_app_env() {
        let err = AppEnv::from_str("foo").expect_err("unknown must fail");
        let msg = format!("{err}");
        assert!(
            msg.contains("APP_ENV") && msg.contains("foo"),
            "msg must name the bad value; got: {msg}"
        );
    }

    #[test]
    fn app_env_defaults_to_development() {
        assert_eq!(AppEnv::from_str("").unwrap(), AppEnv::Development);
        assert_eq!(AppEnv::from_str("   ").unwrap(), AppEnv::Development);
    }

    #[test]
    fn app_env_is_case_insensitive() {
        assert_eq!(AppEnv::from_str("PRODUCTION").unwrap(), AppEnv::Production);
        assert_eq!(AppEnv::from_str("Test").unwrap(), AppEnv::Test);
        assert_eq!(AppEnv::from_str("Staging").unwrap(), AppEnv::Staging);
    }

    #[test]
    fn secure_cookie_required_in_prod_and_staging() {
        assert!(AppEnv::Production.requires_secure_cookie());
        assert!(AppEnv::Staging.requires_secure_cookie());
        assert!(!AppEnv::Development.requires_secure_cookie());
        assert!(!AppEnv::Test.requires_secure_cookie());
    }

    #[test]
    fn database_url_scheme_classifier() {
        assert_eq!(
            database_url_scheme("postgres://u:p@h/d"),
            DbScheme::Postgres
        );
        assert_eq!(
            database_url_scheme("postgresql://u:p@h/d"),
            DbScheme::Postgres
        );
        assert_eq!(database_url_scheme("sqlite:///tmp/oa.db"), DbScheme::Sqlite);
        let mysql = database_url_scheme("mysql://u@h/d");
        assert!(matches!(mysql, DbScheme::Other(_)));
        let bad = database_url_scheme("not-a-url");
        assert!(matches!(bad, DbScheme::Other(_)));
    }
}
