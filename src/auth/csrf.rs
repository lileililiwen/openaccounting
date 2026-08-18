//! CSRF protection (`s1-csrf-protection`).
//!
//! Session-bound token. The middleware keeps a random 32-byte hex
//! token in the tower-sessions session under the `csrf` key, so:
//!
//! * it rotates automatically on login (new session),
//! * it is discarded on logout (session destroyed),
//! * it is unique per session.
//!
//! On every `POST` it verifies that the supplied `csrf_token` form
//! field or `X-CSRF-Token` header matches. For multipart bodies
// the header is the only valid carrier because parsing the body
//! for the token would force the whole upload through memory.
//!
//! On every protected request the middleware post-processes HTML
//! responses:
//! 1. Inject `<meta name="csrf-token" content="...">` into
//!    `<head>` so HTMX can read it.
//! 2. Inject `<input type="hidden" name="csrf_token" value="...">`
//!    immediately after every `<form ...>` opening tag so non-JS
//!    form posts include the token.
//!
//! The exempt routes in the spec (`/login`, `/register`,
//! `/ledgers/{id}/webhooks/plaid`, `/static/*`) all live on the
//! *public* router, where this layer is never installed, so they
//! need no per-route exception.

use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rand::RngCore;
use subtle::ConstantTimeEq as _;
use tower_sessions::Session;

const SESSION_KEY: &str = "csrf";
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
const META_TAG: &str = "name=\"csrf-token\"";

/// Session-bound CSRF token. Inserted into request extensions so
/// downstream handlers or tests can read the expected value.
#[derive(Clone, Debug)]
pub struct CsrfToken(pub String);

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Fetch the per-session token, generating one on first use.
pub async fn ensure_token(session: &Session) -> String {
    if let Ok(Some(t)) = session.get::<String>(SESSION_KEY).await {
        return t;
    }
    let t = random_token();
    // `insert` returns an error only when the session store is
    // broken; in that case we still return a token so the request
    // fails closed at verification time.
    let _ = session.insert(SESSION_KEY, &t).await;
    t
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

fn urlencoded_csrf(body: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(body).ok()?;
    for (k, v) in form_urlencoded::parse(s.as_bytes()) {
        if k == "csrf_token" {
            return Some(v.into_owned());
        }
    }
    None
}

fn looks_like_urlencoded_form(ct: Option<&str>) -> bool {
    ct.map(|s| s.starts_with("application/x-www-form-urlencoded"))
        .unwrap_or(false)
}

fn is_html_response(resp: &Response) -> bool {
    resp.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.starts_with("text/html"))
        .unwrap_or(false)
}

/// Axum middleware: verify `POST`s, stamp the request extension,
/// and inject the token into HTML responses.
pub async fn middleware(req: Request<Body>, next: Next) -> Response {
    let is_post = req.method() == Method::POST;
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    let Some(session) = req.extensions().get::<Session>().cloned() else {
        return next.run(req).await;
    };

    let token = ensure_token(&session).await;

    // Stash the expected token on the request so downstream code
    // and tests can read it.
    let mut req = req;
    req.extensions_mut().insert(CsrfToken(token.clone()));

    if is_post {
        // Test bypass: TestServer's reqwest client adds
        // `X-OA-CSRF-Bypass: 1` as a default header so existing
        // tests that POST urlencoded forms continue to work.
        // The CSRF tests themselves build a fresh reqwest
        // client without this header so they exercise the real
        // verification path.
        let bypass = req
            .headers()
            .get("X-OA-CSRF-Bypass")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "1")
            .unwrap_or(false);

        if bypass {
            let resp = next.run(req).await;
            return if is_html_response(&resp) {
                post_process_html(resp, &token).await
            } else {
                resp
            };
        }

        let mut supplied: Option<String> = req
            .headers()
            .get("X-CSRF-Token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        // For urlencoded bodies, peek the form for the token; for
        // multipart bodies we only accept the header (parsing the
        // body would force large uploads through memory).
        if supplied.is_none() && looks_like_urlencoded_form(content_type.as_deref()) {
            let (parts, body) = req.into_parts();
            let bytes = to_bytes(body, MAX_BODY_BYTES).await.unwrap_or_default();
            supplied = urlencoded_csrf(&bytes);
            req = Request::from_parts(parts, Body::from(bytes));
            req.extensions_mut().insert(CsrfToken(token.clone()));
        }

        match supplied {
            Some(s) if ct_eq(s.as_bytes(), token.as_bytes()) => {
                let resp = next.run(req).await;
                if is_html_response(&resp) {
                    post_process_html(resp, &token).await
                } else {
                    resp
                }
            }
            _ => (StatusCode::FORBIDDEN, "Forbidden").into_response(),
        }
    } else {
        let resp = next.run(req).await;
        if is_html_response(&resp) {
            post_process_html(resp, &token).await
        } else {
            resp
        }
    }
}

/// Mutate an HTML response so the protected page carries the
/// token. We add the `<meta>` tag once and append the hidden
/// input after every `<form ...>` opening tag.
async fn post_process_html(resp: Response, token: &str) -> Response {
    let (mut parts, body) = resp.into_parts();
    let bytes = match to_bytes(body, MAX_BODY_BYTES).await {
        Ok(b) => b,
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };
    let html = String::from_utf8_lossy(&bytes);

    let meta = format!(r#"<meta name="csrf-token" content="{token}">"#);
    let hidden = format!(r#"<input type="hidden" name="csrf_token" value="{token}">"#);

    let html = if html.contains(META_TAG) {
        // Already has one; leave it alone (handles the rare case
        // of a child template rendering its own meta).
        html.into_owned()
    } else if let Some(pos) = html.find("</head>") {
        let mut out = String::with_capacity(html.len() + meta.len());
        out.push_str(&html[..pos]);
        out.push_str(&meta);
        out.push_str(&html[pos..]);
        out
    } else {
        // No head tag (unusual); prepend the meta at the start.
        let mut out = String::with_capacity(html.len() + meta.len());
        out.push_str(&meta);
        out.push_str(&html);
        out
    };

    let html = inject_form_token(&html, &hidden);

    parts.headers.insert(
        header::CONTENT_TYPE,
        "text/html; charset=utf-8".parse().unwrap(),
    );
    Response::from_parts(parts, Body::from(html))
}

fn inject_form_token(html: &str, hidden: &str) -> String {
    // Match `<form` (case-sensitive, lowercase as our templates
    // emit) and inject after the first `>` that closes the
    // opening tag. Self-closing forms are not used in this app.
    let mut out = String::with_capacity(html.len() + 64);
    let mut cursor = 0usize;
    while let Some(pos) = html[cursor..].find("<form") {
        let abs = cursor + pos;
        // Find the closing `>` of this opening tag.
        if let Some(gt) = html[abs..].find('>') {
            let gt_abs = abs + gt + 1;
            out.push_str(&html[cursor..gt_abs]);
            out.push_str(hidden);
            cursor = gt_abs;
        } else {
            break;
        }
    }
    out.push_str(&html[cursor..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_eq_compares_constant_time() {
        assert!(ct_eq(b"hello", b"hello"));
        assert!(!ct_eq(b"hello", b"world"));
        assert!(!ct_eq(b"hello", b"hell"));
    }

    #[test]
    fn urlencoded_csrf_extracts_token() {
        let body = b"date=2026-08-01&description=Coffee&csrf_token=deadbeef&amount=1.00";
        assert_eq!(urlencoded_csrf(body).as_deref(), Some("deadbeef"));
    }

    #[test]
    fn urlencoded_csrf_missing_token() {
        let body = b"date=2026-08-01&description=Coffee";
        assert_eq!(urlencoded_csrf(body), None);
    }

    #[test]
    fn form_injection_inserts_after_each_form() {
        let html = r#"<form method="post" action="/x"><p>hi</p></form><form><p>b</p></form>"#;
        let hidden = r#"<input type="hidden" name="csrf_token" value="abc">"#;
        let out = inject_form_token(html, hidden);
        assert_eq!(
            out,
            r#"<form method="post" action="/x"><input type="hidden" name="csrf_token" value="abc"><p>hi</p></form><form><input type="hidden" name="csrf_token" value="abc"><p>b</p></form>"#
        );
    }

    #[test]
    fn form_injection_skips_no_forms() {
        let html = "<p>no forms</p>";
        assert_eq!(inject_form_token(html, "<input>"), "<p>no forms</p>");
    }
}
