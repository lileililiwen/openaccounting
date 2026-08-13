use askama::Template;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::auth::User;

#[derive(Template)]
#[template(path = "account.html")]
pub struct AccountPage {
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
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

#[allow(dead_code)]
const _UUID_MARKER: Option<Uuid> = None;
