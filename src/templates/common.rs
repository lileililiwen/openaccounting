//! Shared template-side helpers and re-exports.

pub use crate::handlers::route_context::NavContext;

pub fn current_year() -> i32 {
    chrono::Utc::now()
        .format("%Y")
        .to_string()
        .parse()
        .unwrap_or(2024)
}
