//! Global per-route rate limiting (`ops-hardening`).
//!
//! A single in-memory token-bucket mechanism covers auth, API,
//! webhook, and import routes without per-handler code. Buckets are
//! keyed by route class + client IP with per-class per-minute
//! budgets (overridable via env). Exhausted buckets reject with
//! HTTP 429 and a `Retry-After` header. Single-node semantics by
//! design; health, readiness, metrics, and static assets are exempt.
//!
//! This complements (not replaces) the DB-backed login throttle in
//! `crate::auth::rate_limit` and the per-token API limiter in
//! `crate::api::rate_limit`, which enforce stricter per-identity
//! policies.

use axum::{
    extract::{ConnectInfo, Request},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// Route classes with a rate-limit budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteClass {
    Auth,
    Api,
    Webhook,
    Import,
}

impl RouteClass {
    /// Classify a request path. Returns `None` for unlimited routes.
    pub fn classify(path: &str) -> Option<Self> {
        if path == "/login"
            || path.starts_with("/login/")
            || path == "/register"
            || path.starts_with("/register/")
            || path == "/logout"
            || path.starts_with("/auth/")
        {
            Some(Self::Auth)
        } else if path.starts_with("/api/") {
            Some(Self::Api)
        } else if path.contains("/webhooks/") {
            Some(Self::Webhook)
        } else if path.contains("/import") {
            Some(Self::Import)
        } else {
            None
        }
    }

    fn env_var(self) -> &'static str {
        match self {
            Self::Auth => "AUTH_RATE_LIMIT_PER_MIN",
            Self::Api => "API_IP_RATE_LIMIT_PER_MIN",
            Self::Webhook => "WEBHOOK_RATE_LIMIT_PER_MIN",
            Self::Import => "IMPORT_RATE_LIMIT_PER_MIN",
        }
    }

    fn default_budget(self) -> u64 {
        match self {
            Self::Auth => 60,
            Self::Api => 600,
            Self::Webhook => 120,
            Self::Import => 60,
        }
    }

    /// Per-minute budget, overridable via env for operators and tests.
    pub fn budget(self) -> u64 {
        std::env::var(self.env_var())
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or_else(|| self.default_budget())
    }
}

/// A single token bucket: `capacity` tokens, refilled linearly at
/// `capacity` tokens per 60 s.
#[derive(Debug)]
pub struct TokenBucket {
    capacity: f64,
    tokens: f64,
    last: Instant,
}

impl TokenBucket {
    pub fn new(budget_per_min: u64) -> Self {
        Self {
            capacity: budget_per_min.max(1) as f64,
            tokens: budget_per_min.max(1) as f64,
            last: Instant::now(),
        }
    }

    /// Consume one token. `Ok(())` when allowed; `Err(seconds)` with
    /// a `Retry-After` delay (always ≥ 1) when the budget is spent.
    pub fn check(&mut self, now: Instant) -> Result<(), u64> {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.capacity / 60.0).min(self.capacity);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            let wait = (1.0 - self.tokens) * 60.0 / self.capacity;
            Err(wait.ceil() as u64 + 1)
        }
    }
}

static BUCKETS: OnceLock<Mutex<HashMap<(RouteClass, String), TokenBucket>>> = OnceLock::new();

fn buckets() -> &'static Mutex<HashMap<(RouteClass, String), TokenBucket>> {
    BUCKETS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Clear all buckets. Test-support only; `TestServer` calls this so
/// every test starts with a fresh window.
#[cfg(feature = "test-support")]
pub fn reset() {
    buckets()
        .lock()
        .expect("rate-limit buckets poisoned")
        .clear();
}

/// Axum middleware: enforce the bucket for the request's route
/// class + client IP. Exempt paths pass through untouched.
pub async fn limit(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_owned();
    // Health, metrics, and static assets are never limited (the
    // metrics handler documents its own exemption).
    if path == "/healthz" || path == "/readyz" || path == "/metrics" || path.starts_with("/static/")
    {
        return next.run(req).await;
    }
    let Some(class) = RouteClass::classify(&path) else {
        return next.run(req).await;
    };
    let ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let verdict = buckets()
        .lock()
        .expect("rate-limit buckets poisoned")
        .entry((class, ip))
        .or_insert_with(|| TokenBucket::new(class.budget()))
        .check(Instant::now());
    match verdict {
        Ok(()) => next.run(req).await,
        Err(retry_after) => (
            StatusCode::TOO_MANY_REQUESTS,
            [(
                axum::http::header::RETRY_AFTER,
                HeaderValue::from_str(&retry_after.to_string())
                    .unwrap_or(HeaderValue::from_static("1")),
            )],
            "Too Many Requests",
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn bucket_allows_budget_then_returns_429_with_retry_after() {
        let mut bucket = TokenBucket::new(3);
        let now = Instant::now();
        assert!(bucket.check(now).is_ok());
        assert!(bucket.check(now).is_ok());
        assert!(bucket.check(now).is_ok());
        // Budget plus one: rejected with a Retry-After delay.
        let err = bucket.check(now).expect_err("4th hit must be throttled");
        assert!(err >= 1, "Retry-After must be positive, got {err}");
    }

    #[test]
    fn bucket_refills_over_time() {
        let mut bucket = TokenBucket::new(60);
        let start = Instant::now();
        for _ in 0..60 {
            assert!(bucket.check(start).is_ok());
        }
        assert!(bucket.check(start).is_err());
        // After a full window the budget is back.
        assert!(bucket.check(start + Duration::from_secs(61)).is_ok());
    }

    #[test]
    fn route_classification_covers_limited_surfaces() {
        assert_eq!(RouteClass::classify("/login"), Some(RouteClass::Auth));
        assert_eq!(RouteClass::classify("/login/2fa"), Some(RouteClass::Auth));
        assert_eq!(RouteClass::classify("/register"), Some(RouteClass::Auth));
        assert_eq!(
            RouteClass::classify("/api/v1/ledgers"),
            Some(RouteClass::Api)
        );
        assert_eq!(
            RouteClass::classify("/ledgers/1/webhooks/plaid"),
            Some(RouteClass::Webhook)
        );
        assert_eq!(
            RouteClass::classify("/ledgers/1/import"),
            Some(RouteClass::Import)
        );
        assert_eq!(RouteClass::classify("/healthz"), None);
        assert_eq!(RouteClass::classify("/ledgers"), None);
    }
}
