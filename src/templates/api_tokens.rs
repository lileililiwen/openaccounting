//! Askama templates for the `/account/api-tokens` page.
//!
//! Two pages:
//! - [`ApiTokenListPage`] — list current tokens + create form.
//! - [`ApiTokenShowPage`] — same list + a banner that shows the
//!   freshly-minted plaintext exactly once.

use askama::Template;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Template)]
#[template(path = "api_tokens/list.html")]
pub struct ApiTokenListPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub tokens: Vec<ApiTokenSummary>,
    pub plaintext: Option<String>,
}

#[derive(Template)]
#[template(path = "api_tokens/list.html")]
pub struct ApiTokenShowPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub tokens: Vec<ApiTokenSummary>,
    pub plaintext: Option<String>,
    pub plaintext_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiTokenSummary {
    pub id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl From<crate::auth::api_token::ApiTokenSummary> for ApiTokenSummary {
    fn from(other: crate::auth::api_token::ApiTokenSummary) -> Self {
        Self {
            id: other.id,
            name: other.name,
            token_prefix: other.token_prefix,
            created_at: other.created_at,
            last_used_at: other.last_used_at,
            revoked_at: other.revoked_at,
        }
    }
}
