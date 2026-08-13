use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub app_host: String,
    pub app_port: u16,
    pub app_secret: String,
    pub database_url: String,
    pub documents_dir: String,
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

        Ok(Self {
            app_host,
            app_port,
            app_secret,
            database_url,
            documents_dir,
        })
    }
}
