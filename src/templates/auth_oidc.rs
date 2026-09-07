use askama::Template;
use uuid::Uuid;

/// Admin OIDC configuration page.
#[derive(Template)]
#[template(path = "auth_oidc/admin.html")]
pub struct OidcAdminPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub issuer_url: String,
    pub client_id: String,
    /// Whether a secret exists (shown as a mask, never the value).
    pub has_secret: bool,
    pub provisioning: String,
    pub sso_only: bool,
    pub saved: bool,
}
