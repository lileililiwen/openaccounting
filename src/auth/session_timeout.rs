//! Session idle + absolute timeouts.
//!
//! On every request the middleware reads `last_seen_at` and
//! `created_at` from the session payload (set by [`SessionGuard`]
//! when the user authenticates). Two windows are enforced:
//!
//! * **Idle**: 30 minutes since `last_seen_at`.
//! * **Absolute**: 12 hours since `created_at`.
//!
//! If either window has elapsed the session is flushed and the
//! request is redirected to `/login?next=…&expired=1`. The
//! `expired=1` flag is rendered as a flash by `login_page`.
//!
//! Both windows are tunable for tests via the env vars
//! `SESSION_IDLE_SECONDS` and `SESSION_ABSOLUTE_SECONDS`.
//!
//! Public API:
//! * [`SessionGuard::new`] / [`SessionGuard::idle`] /
//!   [`SessionGuard::absolute`] — builder for the middleware
//!   (mainly for tests).
//! * [`mark_authenticated`] — call after a successful login to
//!   stamp `created_at` / `last_seen_at`.
//! * [`enforce_timeout`] — middleware body.

use std::time::Duration;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use time::OffsetDateTime;
use tower_sessions::Session;

/// Default idle window: 30 minutes.
pub const DEFAULT_IDLE_SECONDS: u64 = 30 * 60;
/// Default absolute window: 12 hours.
pub const DEFAULT_ABSOLUTE_SECONDS: u64 = 12 * 60 * 60;

pub const SESSION_KEY_CREATED_AT: &str = "session_created_at";
pub const SESSION_KEY_LAST_SEEN_AT: &str = "session_last_seen_at";

/// Configure the middleware. The middleware itself is
/// installed in `build_router` via
/// `axum::middleware::from_fn_with_state`.
#[derive(Clone, Copy, Debug)]
pub struct SessionGuard {
    pub idle: Duration,
    pub absolute: Duration,
}

impl SessionGuard {
    /// Defaults: 30 min idle, 12 h absolute.
    pub fn new() -> Self {
        Self {
            idle: Duration::from_secs(DEFAULT_IDLE_SECONDS),
            absolute: Duration::from_secs(DEFAULT_ABSOLUTE_SECONDS),
        }
    }

    pub fn idle(mut self, d: Duration) -> Self {
        self.idle = d;
        self
    }

    pub fn absolute(mut self, d: Duration) -> Self {
        self.absolute = d;
        self
    }

    /// Build from env vars, falling back to defaults.
    pub fn from_env() -> Self {
        let idle = std::env::var("SESSION_IDLE_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(DEFAULT_IDLE_SECONDS));
        let absolute = std::env::var("SESSION_ABSOLUTE_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(DEFAULT_ABSOLUTE_SECONDS));
        Self { idle, absolute }
    }
}

impl Default for SessionGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Stamp `created_at` and `last_seen_at` on the session. Call
/// this from `auth::handlers::login_submit` (and the 2FA
/// completion handler) right before `auth.login(&user)`.
pub async fn mark_authenticated(session: &Session) {
    let now = OffsetDateTime::now_utc();
    session
        .insert(SESSION_KEY_CREATED_AT, now)
        .await
        .expect("session insert");
    session
        .insert(SESSION_KEY_LAST_SEEN_AT, now)
        .await
        .expect("session insert");
}

/// Middleware: check both timeouts, refresh `last_seen_at` on
/// successful pass-through, and clear the session on expiry.
pub async fn enforce_timeout(
    State(guard): State<SessionGuard>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let Some(session) = req.extensions().get::<Session>().cloned() else {
        // Session extractor is missing — let downstream 401 it.
        return next.run(req).await;
    };
    let now = OffsetDateTime::now_utc();
    let last_seen: Option<OffsetDateTime> =
        session.get(SESSION_KEY_LAST_SEEN_AT).await.unwrap_or(None);
    let created: Option<OffsetDateTime> = session.get(SESSION_KEY_CREATED_AT).await.unwrap_or(None);

    let idle_expired = last_seen
        .map(|t| (now - t).whole_milliseconds() > guard.idle.as_millis() as i128)
        .unwrap_or(false);
    let absolute_expired = created
        .map(|t| (now - t).whole_milliseconds() > guard.absolute.as_millis() as i128)
        .unwrap_or(false);

    if idle_expired || absolute_expired {
        tracing::warn!(
            idle = idle_expired,
            absolute = absolute_expired,
            "session expired; redirecting to /login"
        );
        // Drop the session.
        session.flush().await.ok();
        // Extract `next` from the URL so we can preserve it.
        let path_and_query = req
            .uri()
            .path_and_query()
            .map(|p| p.to_string())
            .unwrap_or_else(|| "/".to_string());
        let location = format!("/login?next={}&expired=1", urlencode(&path_and_query));
        return (StatusCode::SEE_OTHER, [(header::LOCATION, location)], "").into_response();
    }

    // Refresh last_seen_at only if it's stale enough to matter
    // (avoid spamming the session store on every byte of a single
    // page). Threshold: only refresh if more than half the idle
    // window has elapsed.
    let should_refresh = match last_seen {
        Some(t) => (now - t).whole_milliseconds() >= guard.idle.as_millis() as i128 / 2,
        None => true,
    };
    if should_refresh {
        session.insert(SESSION_KEY_LAST_SEEN_AT, now).await.ok();
    }
    next.run(req).await
}

fn urlencode(s: &str) -> String {
    // Minimal RFC 3986 percent-encoding; we only need to escape
    // characters that would break URL parsing.
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// ─── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_guard_defaults_match_spec() {
        let g = SessionGuard::new();
        assert_eq!(g.idle.as_secs(), 1800);
        assert_eq!(g.absolute.as_secs(), 43_200);
    }

    #[test]
    fn urlencode_passes_through_safe_chars() {
        assert_eq!(urlencode("/ledgers/abc?x=1"), "/ledgers/abc%3Fx%3D1");
        assert_eq!(urlencode("/safe/path_123"), "/safe/path_123");
    }
}
