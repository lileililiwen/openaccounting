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
    pub database_url: String,
    pub documents_dir: String,
    pub app_env: AppEnv,
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
        let database_url =
            env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?;
        let documents_dir = env::var("DOCUMENTS_DIR").unwrap_or_else(|_| "./data/documents".into());
        let app_env = AppEnv::from_env()?;

        Ok(Self {
            app_host,
            app_port,
            app_secret,
            database_url,
            documents_dir,
            app_env,
        })
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
}
