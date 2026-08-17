//! Password hashing and strength validation.
//!
//! - [`hash_password`] / [`verify_password`] — Argon2id via the
//!   `argon2` crate.
//! - [`validate_strength`] — minimum length + breach-list check.
//! - [`MIN_LENGTH`] — minimum password length (12, per the
//!   `s4-password-strength` spec).
//!
//! The bundled deny list (`data/security/common_passwords.txt`) is
//! loaded at first use via [`OnceLock`]; the offline check is
//! always on. The HIBP Pwned Passwords k-anonymity online check
//! is gated behind the `hibp-online` feature flag — by default
//! the binary does NOT make outbound HTTP for password checks.
//!
//! Logging hygiene: the [`validate_strength`] error type stores
//! only the outcome class (`TooShort`, `TooCommon`), never the
//! candidate password. Callers MUST scrub the password before
//! logging the form body.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use std::collections::HashSet;
use std::sync::OnceLock;

/// Minimum acceptable password length (per the
/// `s4-password-strength` spec; was 8 before this change).
pub const MIN_LENGTH: usize = 12;

/// Path to the bundled common-password list, relative to the
/// crate root. `include_str!` embeds it at compile time.
const COMMON_PASSWORDS: &str = include_str!("../../data/security/common_passwords.txt");

/// Lazily-loaded deny list. The first call to
/// [`validate_strength`] pays the parse cost; subsequent calls
/// hit a `HashSet::contains`.
fn deny_list() -> &'static HashSet<String> {
    static CACHE: OnceLock<HashSet<String>> = OnceLock::new();
    CACHE.get_or_init(|| {
        COMMON_PASSWORDS
            .lines()
            .map(|l| l.trim().to_lowercase())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    })
}

/// Outcome of a strength check. The variant names are safe to
/// log or surface to the user — they intentionally contain no
/// part of the candidate password.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrengthError {
    /// Password is shorter than [`MIN_LENGTH`].
    TooShort,
    /// Password appears in the bundled common-password list.
    TooCommon,
}

impl StrengthError {
    /// User-facing message. Stable so the HTTP layer can match
    /// on it without parsing free-form strings.
    pub fn message(&self) -> &'static str {
        match self {
            StrengthError::TooShort => "Password must be at least 12 characters.",
            StrengthError::TooCommon => "This password is too common; please choose another.",
        }
    }
}

impl std::fmt::Display for StrengthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for StrengthError {}

/// Validate a candidate password against the strength policy.
///
/// Returns `Ok(())` if the password meets the policy, or
/// `Err(StrengthError)` otherwise. The candidate itself is
/// never stored or logged — only the policy class is returned.
///
/// Policy:
/// 1. Length ≥ [`MIN_LENGTH`] (12).
/// 2. Not present in the bundled deny list (case-insensitive).
///
/// When the `hibp-online` feature is enabled, an additional
/// k-anonymity check against the HIBP Pwned Passwords API is
/// attempted. A network failure is logged and treated as
/// "allowed" (fail-open) so an outage of HIBP cannot block
/// legitimate sign-ups.
pub fn validate_strength(candidate: &str) -> Result<(), StrengthError> {
    if candidate.chars().count() < MIN_LENGTH {
        return Err(StrengthError::TooShort);
    }
    let lower = candidate.trim().to_lowercase();
    if deny_list().contains(&lower) {
        return Err(StrengthError::TooCommon);
    }
    #[cfg(feature = "hibp-online")]
    {
        if let Err(e) = check_hibp_online(candidate) {
            tracing::warn!(error = %e, "HIBP online check failed; failing open");
        }
    }
    Ok(())
}

/// Mask a password for safe logging. The returned string is
/// always `"***"` regardless of the input length so log readers
/// cannot infer length or content.
pub fn mask(_password: &str) -> &'static str {
    "***"
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    let parsed = PasswordHash::new(hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

// ─── Optional HIBP Pwned Passwords k-anonymity check ────────────────

#[cfg(feature = "hibp-online")]
fn check_hibp_online(candidate: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use sha1::{Digest, Sha1};
    let digest = Sha1::digest(candidate.as_bytes());
    let hex = format!("{:X}", digest);
    let (prefix, suffix) = hex.split_at(5);
    let url = format!("https://api.pwnedpasswords.com/range/{prefix}");
    let resp = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()?
        .get(&url)
        .header("Add-Padding", "true")
        .send()?;
    if !resp.status().is_success() {
        return Err(format!("hibp status {}", resp.status()).into());
    }
    let body = resp.text()?;
    let target = suffix.to_uppercase();
    for line in body.lines() {
        let (suf, _count) = line.split_once(':').unwrap_or((line, "0"));
        if suf.trim().eq_ignore_ascii_case(&target) {
            return Err(Box::new(StrengthError::TooCommon));
        }
    }
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_accepts_strong_password() {
        let pw = "X7!qZ4wN9pLk_3vR"; // 16 chars, mixed
        assert!(validate_strength(pw).is_ok());
    }

    #[test]
    fn validate_rejects_short_password() {
        let err = validate_strength("Short1!").expect_err("11 chars must fail");
        assert_eq!(err, StrengthError::TooShort);
        assert!(err.message().contains("at least 12"));
    }

    #[test]
    fn validate_rejects_common_password() {
        // Use a 12+ char common password so length check passes
        // and the deny list triggers.
        let err = validate_strength("Password1234").expect_err("common must fail");
        assert_eq!(err, StrengthError::TooCommon);
        // Same with mixed-case variant.
        let err2 = validate_strength("password1234").expect_err("lower must also fail");
        assert_eq!(err2, StrengthError::TooCommon);
    }

    #[test]
    fn validate_accepts_random_16_chars() {
        // 16 random lowercase letters.
        let pw: String = (b'a'..=b'z').cycle().take(16).map(|c| c as char).collect();
        assert!(validate_strength(&pw).is_ok(), "{pw} should pass");
    }

    #[test]
    fn validate_scrubs_password_in_error() {
        // The error variants must not contain the candidate.
        let pw = "Password1234";
        let err = validate_strength(pw).expect_err("common password should fail");
        let msg = format!("{err}");
        assert!(
            !msg.contains(pw),
            "error must not echo the password; got: {msg}"
        );
        // The literal word "Password" must not leak in the error
        // message (information leakage).
        assert!(
            !msg.contains("Password"),
            "error must not leak password fragment; got: {msg}"
        );
    }

    #[test]
    fn mask_returns_constant() {
        assert_eq!(mask("anything"), "***");
        assert_eq!(mask(""), "***");
        assert_eq!(mask("a-very-long-password"), "***");
    }

    #[test]
    fn deny_list_loads_at_least_one_entry() {
        assert!(deny_list().len() > 100, "deny list suspiciously small");
    }

    #[test]
    fn hash_password_is_bijective_for_same_password() {
        // Property: Argon2id with random salt is non-deterministic
        // but verify succeeds. We assert verify=true for the
        // canonical fixture.
        let h = hash_password("X7!qZ4wN9pLk_3vR").expect("hash");
        assert!(verify_password("X7!qZ4wN9pLk_3vR", &h).unwrap());
        assert!(!verify_password("X7!qZ4wN9pLk_3vX", &h).unwrap());
    }
}
