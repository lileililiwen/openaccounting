//! Cookie signing (HMAC-SHA256) with multi-key support for
//! rotation. Implements the `signed-cookies` capability.
//!
//! Why custom instead of `tower-sessions`' built-in `signed`
//! feature? That feature supports exactly one signing key, so
//! rolling `APP_SECRET` would invalidate every live session.
//! The custom layer below takes a list of keys and tries each
//! in order; when an older key verifies, the cookie is
//! transparently re-signed with the current key.
//!
//! Wire format: the `oa_session` cookie value is
//! `<session_id>.<b64url(hmac_sha256(key, session_id))>`. We
//! strip the MAC before handing the request to the inner
//! stack so `tower-sessions` reads a bare `<session_id>`; the
//! MAC is re-attached to the `Set-Cookie` response header so
//! the browser stores the signed value.

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// HKDF info used to derive the per-secret HMAC key. Stable
/// across releases so re-deriving always produces the same key.
const HKDF_INFO: &[u8] = b"oa-session-cookie-hmac-v1";

/// Multi-key signer. Verification tries `current` first, then
/// each entry of `previous` in order. On a `previous` match
/// the cookie is rewritten with the `current` signature.
#[derive(Clone)]
pub struct CookieSigner {
    pub current: Arc<Vec<u8>>,
    pub previous: Arc<Vec<Vec<u8>>>,
}

impl CookieSigner {
    pub fn derive_from(current_secret: &str, previous_secret: Option<&str>) -> Self {
        let current = Arc::new(derive_key(current_secret));
        let previous = Arc::new(
            previous_secret
                .map(derive_key)
                .into_iter()
                .collect::<Vec<_>>(),
        );
        Self { current, previous }
    }

    /// Returns the matching key index. `Some(0)` means the
    /// current key matched; `Some(n)` with `n > 0` means
    /// `previous[n - 1]` matched.
    pub fn verify(&self, session_id: &str, mac_b64: &str) -> Option<usize> {
        let Ok(mac_bytes) = B64.decode(mac_b64) else {
            return None;
        };
        if constant_time_eq(&hmac(&self.current, session_id), &mac_bytes) {
            return Some(0);
        }
        for (i, prev) in self.previous.iter().enumerate() {
            if constant_time_eq(&hmac(prev, session_id), &mac_bytes) {
                return Some(i + 1);
            }
        }
        None
    }

    pub fn sign(&self, session_id: &str) -> String {
        B64.encode(hmac(&self.current, session_id))
    }
}

fn derive_key(secret: &str) -> Vec<u8> {
    let hk = Hkdf::<Sha256>::new(None, secret.as_bytes());
    let mut okm = vec![0u8; 32];
    hk.expand(HKDF_INFO, &mut okm)
        .expect("HKDF expand to 32 bytes is well-defined");
    okm
}

/// HMAC-SHA256 implemented inline so we don't pull the `hmac`
/// crate. RFC 2104: `H(K xor opad || H(K xor ipad || text))`.
fn hmac(key: &[u8], data: &str) -> Vec<u8> {
    const BLOCK: usize = 64;
    // Pad / truncate the key to BLOCK bytes.
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        let h = Sha256::digest(key);
        k[..h.len()].copy_from_slice(&h);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    // inner = SHA256(ipad || data)
    let mut inner_hasher = Sha256::new();
    inner_hasher.update(ipad);
    inner_hasher.update(data.as_bytes());
    let inner = inner_hasher.finalize();
    // outer = SHA256(opad || inner)
    let mut outer_hasher = Sha256::new();
    outer_hasher.update(opad);
    outer_hasher.update(inner);
    outer_hasher.finalize().to_vec()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

pub const COOKIE_NAME: &str = "oa_session";

/// Middleware: verify the `oa_session` cookie's MAC; strip the
/// MAC before handing the request to the inner stack (which
/// expects a bare session id); re-attach the MAC to the
/// outgoing `Set-Cookie` header so the browser stores a
/// signed value.
pub async fn enforce_signed_cookie(
    State(signer): State<Arc<CookieSigner>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let cookie_header = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let Some(cookie_header) = cookie_header else {
        return next.run(req).await;
    };

    let Some(parsed) = parse_session_cookie(&cookie_header) else {
        return next.run(req).await;
    };
    let (session_id, mac) = parsed;

    let matched = signer.verify(&session_id, &mac);
    let Some(idx) = matched else {
        // Tampered or signed by an unknown key. Reject.
        let clear = format!("{}=; Path=/; Max-Age=0", COOKIE_NAME);
        return (
            StatusCode::UNAUTHORIZED,
            [(header::SET_COOKIE, clear)],
            "Invalid session cookie.",
        )
            .into_response();
    };

    // Replace the cookie header with the bare session id
    // (without the .mac suffix) so tower-sessions reads it.
    let new_cookie = format!("{}={}", COOKIE_NAME, session_id);
    if let Ok(hv) = HeaderValue::from_str(&new_cookie) {
        req.headers_mut().insert(header::COOKIE, hv);
    }

    let mut response = next.run(req).await;

    // Re-attach the MAC on the way out so the browser stores a
    // signed value. tower-sessions sets a bare Set-Cookie like
    // `oa_session=<id>; Path=/; ...`; we replace it with
    // `oa_session=<id>.<mac>; ...`.
    if let Some(set_cookie) = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
    {
        if let Some(new_sc) = rewrite_set_cookie(&set_cookie, &signer) {
            if let Ok(hv) = HeaderValue::from_str(&new_sc) {
                response.headers_mut().insert(header::SET_COOKIE, hv);
            }
        }
    }

    response
}

/// Append `.<mac>` to the bare session id in the Set-Cookie
/// header. tower-sessions emits `oa_session=<id>; Path=/; ...`;
/// we need `oa_session=<id>.<mac>; Path=/; ...` so the
/// browser stores a signed value.
fn rewrite_set_cookie(set_cookie: &str, signer: &CookieSigner) -> Option<String> {
    let mut out = String::with_capacity(set_cookie.len() + 64);
    let mut first = true;
    for part in set_cookie.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if !first {
            out.push_str("; ");
        }
        first = false;
        if let Some((name, value)) = part.split_once('=') {
            if name == COOKIE_NAME {
                // Sign the new session id that tower-sessions
                // just emitted. This may differ from the id
                // supplied on the incoming cookie (e.g. if the
                // old id wasn't in the store).
                let mac = signer.sign(value);
                out.push_str(&format!("{}={}.{}", COOKIE_NAME, value, mac));
                continue;
            }
        }
        out.push_str(part);
    }
    Some(out)
}

/// Parse the `<session_id>.<mac>` payload out of the cookie
/// header. Returns `None` when the cookie is absent or
/// malformed.
fn parse_session_cookie(header: &str) -> Option<(String, String)> {
    for part in header.split(';') {
        let trimmed = part.trim();
        let (name, value) = trimmed.split_once('=')?;
        if name == COOKIE_NAME {
            let (sid, mac) = value.split_once('.')?;
            return Some((sid.to_string(), mac.to_string()));
        }
    }
    None
}

// ─── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key_is_stable_and_distinct_per_secret() {
        let a = derive_key("alpha-secret-1234567890abcdefghij");
        let b = derive_key("alpha-secret-1234567890abcdefghij");
        let c = derive_key("beta-secret-1234567890abcdefghijxx");
        assert_eq!(a, b, "same secret → same key");
        assert_ne!(a, c, "different secrets → different keys");
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let s = CookieSigner::derive_from("alpha-secret-1234567890abcdefghij", None);
        let mac = s.sign("session-id-1234");
        assert_eq!(s.verify("session-id-1234", &mac), Some(0));
        assert_eq!(s.verify("session-id-9999", &mac), None);
    }

    #[test]
    fn previous_key_is_accepted_for_rotation() {
        let s = CookieSigner::derive_from(
            "new-secret-1234567890abcdefghijkl",
            Some("old-secret-1234567890abcdefghij"),
        );
        let old_signer = CookieSigner::derive_from("old-secret-1234567890abcdefghij", None);
        let mac = old_signer.sign("sess-abc");
        assert_eq!(s.verify("sess-abc", &mac), Some(1));
        let new_mac = s.sign("sess-abc");
        assert_eq!(s.verify("sess-abc", &new_mac), Some(0));
    }

    #[test]
    fn tampered_mac_is_rejected() {
        let s = CookieSigner::derive_from("alpha-secret-1234567890abcdefghij", None);
        let mut mac = s.sign("sess-abc");
        let mut chars: Vec<char> = mac.chars().collect();
        chars[0] = if chars[0] == 'A' { 'B' } else { 'A' };
        mac = chars.into_iter().collect();
        assert_eq!(s.verify("sess-abc", &mac), None);
    }

    #[test]
    fn parse_session_cookie_extracts_components() {
        let h = "oa_session=abc123.zzz; other=ignored";
        let (sid, mac) = parse_session_cookie(h).unwrap();
        assert_eq!(sid, "abc123");
        assert_eq!(mac, "zzz");
    }

    #[test]
    fn rewrite_set_cookie_appends_mac() {
        let s = CookieSigner::derive_from("alpha-secret-1234567890abcdefghij", None);
        let input = "oa_session=abc123; Path=/; HttpOnly; SameSite=Lax";
        let out = rewrite_set_cookie(input, &s).unwrap();
        assert!(out.starts_with("oa_session=abc123."));
        assert!(out.contains("Path=/"));
        assert!(out.contains("HttpOnly"));
    }
}
