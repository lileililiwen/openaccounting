use askama::Template;

pub fn current_year() -> i32 {
    chrono::Utc::now()
        .format("%Y")
        .to_string()
        .parse()
        .unwrap_or(2024)
}

/// The 2FA step page rendered after a password check passes for
/// an enrolled user. The user submits a 6-digit code here to
/// complete the session.
#[derive(Template)]
#[template(path = "auth/login_2fa.html")]
pub struct Login2faPage {
    pub error: String,
    pub next: String,
}

impl Login2faPage {
    pub fn new(next: String) -> Self {
        Self {
            error: String::new(),
            next,
        }
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = error.into();
        self
    }
}

// `LoginPage` and `RegisterPage` are defined in `src/auth/handlers.rs` because
// they are tightly coupled with the auth handlers. They're not re-exported here.
