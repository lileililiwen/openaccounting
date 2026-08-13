pub fn current_year() -> i32 {
    chrono::Utc::now()
        .format("%Y")
        .to_string()
        .parse()
        .unwrap_or(2024)
}

// `LoginPage` and `RegisterPage` are defined in `src/auth/handlers.rs` because
// they are tightly coupled with the auth handlers. They're not re-exported here.
