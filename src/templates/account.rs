use askama::Template;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::auth::User;

#[derive(Template)]
#[template(path = "account/security.html")]
pub struct SecurityPage {
    pub enrolled: bool,
    pub enrolled_at: String,
    pub unused_count: i64,
    pub qr_svg: String,
    pub secret_b32: String,
    pub recovery_codes: Vec<String>,
    pub flash: String,
    pub error: String,
    // Fields used by the shared `partials/_nav.html` include —
    // Askama does not auto-inherit, so every template that
    // includes the nav must expose these even if unused.
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
}

impl SecurityPage {
    pub fn not_enrolled(qr_svg: String, secret_b32: String, user: &User) -> Self {
        Self {
            enrolled: false,
            enrolled_at: String::new(),
            unused_count: 0,
            qr_svg,
            secret_b32,
            recovery_codes: Vec::new(),
            flash: String::new(),
            error: String::new(),
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id: Uuid::nil(),
            ledger_name: String::new(),
            current_section: String::new(),
        }
    }

    pub fn enrolled(
        enrolled_at: String,
        unused_count: i64,
        recovery_codes: Vec<String>,
        user: &User,
    ) -> Self {
        Self {
            enrolled: true,
            enrolled_at,
            unused_count,
            qr_svg: String::new(),
            secret_b32: String::new(),
            recovery_codes,
            flash: String::new(),
            error: String::new(),
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id: Uuid::nil(),
            ledger_name: String::new(),
            current_section: String::new(),
        }
    }

    pub fn with_flash(mut self, flash: impl Into<String>) -> Self {
        self.flash = flash.into();
        self
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = error.into();
        self
    }
}

#[derive(Template)]
#[template(path = "account.html")]
pub struct AccountPage {
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub user: User,
    pub created_at_display: String,
    pub updated_at_display: String,
    pub error: String,
    pub flash: String,
    pub current: String,
    pub new: String,
    pub confirm: String,
}

pub fn fmt_dt(dt: &OffsetDateTime) -> String {
    // Manual format: "YYYY-MM-DD HH:MM"
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        dt.year(),
        dt.month() as u8,
        dt.day(),
        dt.hour(),
        dt.minute(),
    )
}

pub fn fmt_date(dt: &OffsetDateTime) -> String {
    format!("{:04}-{:02}-{:02}", dt.year(), dt.month() as u8, dt.day())
}

impl AccountPage {
    pub fn new(user: User) -> Self {
        let created_at_display = fmt_date(&user.created_at);
        let updated_at_display = fmt_dt(&user.updated_at);
        Self {
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id: Uuid::nil(),
            ledger_name: String::new(),
            current_section: String::new(),
            created_at_display,
            updated_at_display,
            user,
            error: String::new(),
            flash: String::new(),
            current: String::new(),
            new: String::new(),
            confirm: String::new(),
        }
    }

    /// Build a new page carrying the previous form input + an error
    /// banner. Used when validation fails on `POST /account/password`.
    pub fn with_error(user: User, error: impl Into<String>, form: PasswordForm) -> Self {
        let mut p = Self::new(user);
        p.error = error.into();
        p.current = form.current;
        p.new = form.new;
        p.confirm = form.confirm;
        p
    }

    /// Build a new page carrying a success banner.
    pub fn with_flash(user: User, flash: impl Into<String>) -> Self {
        let mut p = Self::new(user);
        p.flash = flash.into();
        p
    }
}

#[derive(Clone, Debug, Default)]
pub struct PasswordForm {
    pub current: String,
    pub new: String,
    pub confirm: String,
}

impl PasswordForm {
    pub fn from_post(current: String, new: String, confirm: String) -> Self {
        Self {
            current,
            new,
            confirm,
        }
    }
}
