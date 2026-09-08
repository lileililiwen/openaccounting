//! Integration tests for security-operations-baseline configuration
//! validation. Tests that production mode rejects placeholder
//! secrets, default database credentials, and insecure settings.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openaccounting::config::AppEnv;

/// Known placeholder secrets that must be rejected in production.
const PLACEHOLDER_SECRETS: &[&str] = &[
    "please-change-me-to-a-long-random-string",
    "dev-only-secret-please-change-in-production-64-chars-min",
    "dev-secret-do-not-use-in-production-please-replace-with-64-random-chars",
    "replace-with-64-random-chars-minimum-for-security",
    "changeme123456789012345678901234567890",
];

/// Default Compose database URLs that must be rejected in production.
const PLACEHOLDER_DB_URLS: &[&str] = &[
    "postgres://openaccounting:openaccounting@localhost/openaccounting",
    "postgres://postgres:postgres@localhost/postgres",
];

/// Strong secrets that should be accepted in production.
const STRONG_SECRETS: &[&str] = &[
    "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
    "Xk9#mP2$vL8@nQ5!wR7&yT3*uE6^iO1(oA4)sD0=fGhIjKlMnOpQrStUvWxYz",
];

/// Strong database URLs that should be accepted in production.
const STRONG_DB_URLS: &[&str] = &[
    "postgres://oa_xK9m:Vx8nQ5wR7@db.example.com:5432/openaccounting",
    "postgres://app_user:s3cur3_p@ssw0rd!@10.0.1.50:5432/production_db",
];

#[test]
fn production_rejects_all_placeholder_secrets() {
    for secret in PLACEHOLDER_SECRETS {
        let result = openaccounting::config::validate_production_secret(secret, AppEnv::Production);
        assert!(
            result.is_err(),
            "production should reject placeholder secret: {secret:?}"
        );
    }
}

#[test]
fn staging_rejects_all_placeholder_secrets() {
    for secret in PLACEHOLDER_SECRETS {
        let result = openaccounting::config::validate_production_secret(secret, AppEnv::Staging);
        assert!(
            result.is_err(),
            "staging should reject placeholder secret: {secret:?}"
        );
    }
}

#[test]
fn development_allows_placeholder_secrets() {
    for secret in PLACEHOLDER_SECRETS {
        let result =
            openaccounting::config::validate_production_secret(secret, AppEnv::Development);
        assert!(
            result.is_ok(),
            "development should allow placeholder secret: {secret:?}"
        );
    }
}

#[test]
fn test_allows_placeholder_secrets() {
    for secret in PLACEHOLDER_SECRETS {
        let result = openaccounting::config::validate_production_secret(secret, AppEnv::Test);
        assert!(
            result.is_ok(),
            "test should allow placeholder secret: {secret:?}"
        );
    }
}

#[test]
fn production_accepts_strong_secrets() {
    for secret in STRONG_SECRETS {
        let result = openaccounting::config::validate_production_secret(secret, AppEnv::Production);
        assert!(
            result.is_ok(),
            "production should accept strong secret: {secret:?}"
        );
    }
}

#[test]
fn production_rejects_default_database_credentials() {
    for url in PLACEHOLDER_DB_URLS {
        let result =
            openaccounting::config::validate_production_database_url(url, AppEnv::Production);
        assert!(
            result.is_err(),
            "production should reject default DB credentials: {url:?}"
        );
    }
}

#[test]
fn staging_rejects_default_database_credentials() {
    for url in PLACEHOLDER_DB_URLS {
        let result = openaccounting::config::validate_production_database_url(url, AppEnv::Staging);
        assert!(
            result.is_err(),
            "staging should reject default DB credentials: {url:?}"
        );
    }
}

#[test]
fn production_accepts_nondefault_database_credentials() {
    for url in STRONG_DB_URLS {
        let result =
            openaccounting::config::validate_production_database_url(url, AppEnv::Production);
        assert!(
            result.is_ok(),
            "production should accept non-default DB credentials: {url:?}"
        );
    }
}

#[test]
fn development_allows_default_database_credentials() {
    for url in PLACEHOLDER_DB_URLS {
        let result =
            openaccounting::config::validate_production_database_url(url, AppEnv::Development);
        assert!(
            result.is_ok(),
            "development should allow default DB credentials: {url:?}"
        );
    }
}

#[test]
fn app_env_production_requires_secure_cookie() {
    assert!(AppEnv::Production.requires_secure_cookie());
    assert!(AppEnv::Staging.requires_secure_cookie());
    assert!(!AppEnv::Development.requires_secure_cookie());
    assert!(!AppEnv::Test.requires_secure_cookie());
}

#[test]
fn app_env_invalid_value_fails() {
    let result = AppEnv::from_str("invalid-environment");
    assert!(result.is_err());
}

#[test]
fn app_env_case_insensitive() {
    assert_eq!(AppEnv::from_str("PRODUCTION").unwrap(), AppEnv::Production);
    assert_eq!(AppEnv::from_str("Staging").unwrap(), AppEnv::Staging);
    assert_eq!(
        AppEnv::from_str("development").unwrap(),
        AppEnv::Development
    );
    assert_eq!(AppEnv::from_str("TEST").unwrap(), AppEnv::Test);
}

#[test]
fn app_env_empty_defaults_to_development() {
    assert_eq!(AppEnv::from_str("").unwrap(), AppEnv::Development);
}
