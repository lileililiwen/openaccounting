//! Route context: a small value-type that carries the data the
//! `partials/_nav.html`, `partials/_breadcrumb.html`, and
//! `partials/_sidebar.html` partials need to render.
//!
//! `u11-menu-navigation` requires the active state to be driven by
//! the *top-level section*, not the URL path, so child views
//! (e.g. `/ledgers/{id}/transactions/{txn_id}`) keep the parent's
//! active item. This is enforced by [`Section::from_path`] which
//! maps a URL path to a top-level [`Section`].

use uuid::Uuid;

use crate::auth::User;
use crate::domain::Ledger;

/// The top-level navigation sections exposed in
/// `partials/_nav.html` and `partials/_sidebar.html`. The
/// canonical order matches the spec: **Record → Plan → Analyze →
/// Admin**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Dashboard,
    Transactions,
    Accounts,
    Documents,
    BankFeeds,
    Reports,
    Import,
    ImportWechat,
    ImportAlipay,
    Expenses,
    Approvals,
    Admin,
    Ledgers,
    /// "No section" — used on auth pages, the ledgers list, the
    /// account settings page, etc.
    None,
}

impl Section {
    /// The canonical string used in the `current_section` field
    /// and matched against `is_active(link_section)`.
    pub fn as_str(self) -> &'static str {
        match self {
            Section::Dashboard => "dashboard",
            Section::Transactions => "transactions",
            Section::Accounts => "accounts",
            Section::Documents => "documents",
            Section::BankFeeds => "bank-feeds",
            Section::Reports => "reports",
            Section::Import => "import",
            Section::ImportWechat => "import-wechat",
            Section::ImportAlipay => "import-alipay",
            Section::Expenses => "expenses",
            Section::Approvals => "approvals",
            Section::Admin => "admin",
            Section::Ledgers => "ledgers",
            Section::None => "",
        }
    }

    /// Human-readable label used in the breadcrumb and the
    /// sidebar section headers.
    pub fn label(self) -> &'static str {
        match self {
            Section::Dashboard => "Dashboard",
            Section::Transactions => "Transactions",
            Section::Accounts => "Accounts",
            Section::Documents => "Documents",
            Section::BankFeeds => "Bank Feeds",
            Section::Reports => "Reports",
            Section::Import => "Import",
            Section::ImportWechat => "WeChat",
            Section::ImportAlipay => "Alipay",
            Section::Expenses => "Expenses",
            Section::Approvals => "Approvals",
            Section::Admin => "Admin",
            Section::Ledgers => "Ledgers",
            Section::None => "",
        }
    }

    /// Map a URL path to a top-level section. Child views keep
    /// the parent's section (e.g. `/ledgers/{id}/transactions/{tid}`
    /// → `Some(Section::Transactions)`).
    pub fn from_path(path: &str) -> Option<Section> {
        // Normalise: strip query string and trailing slash.
        let p = path.split('?').next().unwrap_or(path);
        let p = p.trim_end_matches('/');
        if p.is_empty() {
            return Some(Section::None);
        }
        // Top-level non-ledger pages.
        if p == "/ledgers" {
            return Some(Section::Ledgers);
        }
        if p.starts_with("/admin") {
            return Some(Section::Admin);
        }
        if p == "/account" || p.starts_with("/account/") {
            return Some(Section::None);
        }
        // Per-ledger routes: match the first segment after `/ledgers/{id}/`.
        let rest = p.strip_prefix("/ledgers/")?;
        // rest = "{id}/..." or just "{id}".
        let after_id = rest.split_once('/').map(|(_, tail)| tail).unwrap_or("");
        if after_id.is_empty() {
            // `/ledgers/{id}` — the ledger overview page.
            return Some(Section::None);
        }
        let head = after_id.split('/').next().unwrap_or("");
        match head {
            "dashboard" => Some(Section::Dashboard),
            "transactions" => Some(Section::Transactions),
            "accounts" => Some(Section::Accounts),
            "documents" => Some(Section::Documents),
            "bank-feeds" => Some(Section::BankFeeds),
            "reports" => Some(Section::Reports),
            "import" => {
                // The WeChat / Alipay import pages have their own
                // top-level entries. The generic import page
                // (`/ledgers/{id}/import`) maps to Section::Import.
                if after_id.starts_with("import/wechat") || after_id == "import/wechat" {
                    Some(Section::ImportWechat)
                } else if after_id.starts_with("import/alipay") || after_id == "import/alipay" {
                    Some(Section::ImportAlipay)
                } else {
                    Some(Section::Import)
                }
            }
            "reimbursements" => Some(Section::Expenses),
            "approval-policies" => Some(Section::Approvals),
            _ => None,
        }
    }
}

/// A bag of fields the nav / breadcrumb / sidebar partials need
/// to render. Every page struct that includes the nav includes
/// one of these as a single `pub nav: NavContext` field.
#[derive(Debug, Clone)]
pub struct NavContext {
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
}

impl NavContext {
    /// Build a `NavContext` from a logged-in user and the
    /// current section. `ledger` is `Some` when the request is
    /// inside a ledger and `None` for top-level pages
    /// (`/ledgers`, `/account`, `/admin`, etc.).
    pub fn new(user: &User, ledger: Option<&Ledger>, section: Section) -> Self {
        let (ledger_id, ledger_name) = match ledger {
            Some(l) => (l.id, l.name.clone()),
            None => (Uuid::nil(), String::new()),
        };
        Self {
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name,
            current_section: section.as_str().to_string(),
        }
    }

    /// `true` when the given link's section is the current
    /// section. The template uses this to emit `is-active` on
    /// the active nav item and the sliding indicator target.
    pub fn is_active(&self, link_section: &str) -> bool {
        // Empty current section means "no section" — nothing is
        // active (Requirement 1: no flash of wrong active state).
        if self.current_section.is_empty() {
            return false;
        }
        self.current_section == link_section
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_from_path_top_level_pages() {
        assert_eq!(Section::from_path("/ledgers"), Some(Section::Ledgers));
        assert_eq!(Section::from_path("/admin"), Some(Section::Admin));
        assert_eq!(Section::from_path("/admin/users"), Some(Section::Admin));
        assert_eq!(Section::from_path("/account"), Some(Section::None));
        assert_eq!(Section::from_path("/account/security"), Some(Section::None));
        assert_eq!(Section::from_path(""), Some(Section::None));
    }

    #[test]
    fn section_from_path_ledger_overview() {
        // The bare `/ledgers/{id}` is the ledger overview page;
        // it's a "no section" page even though the URL is
        // per-ledger.
        assert_eq!(
            Section::from_path("/ledgers/00000000-0000-0000-0000-000000000000"),
            Some(Section::None)
        );
    }

    #[test]
    fn section_from_path_sections() {
        let id = "00000000-0000-0000-0000-000000000000";
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/dashboard")),
            Some(Section::Dashboard)
        );
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/transactions")),
            Some(Section::Transactions)
        );
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/accounts")),
            Some(Section::Accounts)
        );
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/documents")),
            Some(Section::Documents)
        );
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/bank-feeds")),
            Some(Section::BankFeeds)
        );
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/reports")),
            Some(Section::Reports)
        );
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/import")),
            Some(Section::Import)
        );
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/reimbursements")),
            Some(Section::Expenses)
        );
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/approval-policies")),
            Some(Section::Approvals)
        );
    }

    #[test]
    fn section_from_path_import_subroutes() {
        let id = "00000000-0000-0000-0000-000000000000";
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/import/wechat")),
            Some(Section::ImportWechat)
        );
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/import/alipay")),
            Some(Section::ImportAlipay)
        );
    }

    /// Anti-regression: child views keep the parent's section
    /// active (Requirement 8, the Frappe v16 sidebar
    /// auto-switching bug).
    #[test]
    fn section_from_path_child_views_keep_parent_section() {
        let id = "00000000-0000-0000-0000-000000000000";
        let txn = "11111111-1111-1111-1111-111111111111";
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/transactions/{txn}")),
            Some(Section::Transactions)
        );
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/transactions/{txn}/edit")),
            Some(Section::Transactions)
        );
        let acct = "22222222-2222-2222-2222-222222222222";
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/accounts/{acct}")),
            Some(Section::Accounts)
        );
        let doc = "33333333-3333-3333-3333-333333333333";
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/documents/{doc}/download")),
            Some(Section::Documents)
        );
    }

    #[test]
    fn section_from_path_strips_query_string() {
        let id = "00000000-0000-0000-0000-000000000000";
        assert_eq!(
            Section::from_path(&format!("/ledgers/{id}/transactions?from=2026-01-01")),
            Some(Section::Transactions)
        );
    }

    #[test]
    fn section_as_str_round_trip() {
        let s = Section::Transactions;
        assert_eq!(s.as_str(), "transactions");
        assert_eq!(s.label(), "Transactions");
    }

    #[test]
    fn is_active_matches_current_section() {
        let user = sample_user();
        let nav = NavContext::new(&user, None, Section::Transactions);
        assert!(nav.is_active("transactions"));
        assert!(!nav.is_active("dashboard"));
        assert!(!nav.is_active(""));
    }

    #[test]
    fn is_active_is_false_when_no_section() {
        let user = sample_user();
        let nav = NavContext::new(&user, None, Section::None);
        // Nothing is active when the page is not inside a
        // section (auth pages, /ledgers, /account, ...).
        assert!(!nav.is_active("transactions"));
        assert!(!nav.is_active("ledgers"));
    }

    fn sample_user() -> User {
        User {
            id: Uuid::nil(),
            email: "alice@example.com".into(),
            username: "alice".into(),
            display_name: None,
            role: "admin".into(),
            hashed_password: String::new(),
            is_active: true,
            created_at: time::OffsetDateTime::now_utc(),
            updated_at: time::OffsetDateTime::now_utc(),
            theme: "system".into(),
            locale: "en".into(),
        }
    }
}
