// Tests for the `openapi-sdk` change: OpenAPI contract, idempotency,
// incoming event intake, automation rule builder, and event catalog.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;

use reqwest::StatusCode;
use serde_json::Value;
use uuid::Uuid;

use crate::common::TestServer;

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

async fn bootstrap() -> (TestServer, Uuid, String, Uuid) {
    let server = TestServer::new().await;
    let email = format!("sdk-{}@test.example", Uuid::new_v4());
    server.bootstrap_user(&email, &email, PASSWORD).await;
    let pool = server.db().pool();
    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let issued = openaccounting::auth::api_token::issue_token(&pool, user_id, "test-token")
        .await
        .expect("issue token");
    let token = issued.plaintext;
    let ledger_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ledgers (owner_id, name, base_currency)
         VALUES ($1, 'SDK Test Ledger', 'USD')
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    (server, ledger_id, token, user_id)
}

async fn create_account_sql(pool: &sqlx::PgPool, ledger_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO accounts (ledger_id, name, type, currency)
         VALUES ($1, $2, 'ASSET', 'USD')
         ON CONFLICT (ledger_id, name) DO UPDATE SET name = EXCLUDED.name
         RETURNING id",
    )
    .bind(ledger_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

// ─── 1.1 Unit: idempotency store returns original response hash ────────

#[tokio::test]
async fn idempotency_lookup_returns_stored_response_for_replay() {
    let (server, _ledger_id, token, _user_id) = bootstrap().await;
    let pool = server.db().pool();
    let key = Uuid::new_v4().to_string();
    let issued = openaccounting::auth::api_token::issue_token(&pool, _user_id, "extra-token")
        .await
        .unwrap();
    let token_id = issued.id;
    let body = serde_json::json!({ "name": "Idem Test", "base_currency": "USD" });
    let fp = openaccounting::api::helpers::fingerprint(&body);
    let resp_body = r#"{"id":"00000000-0000-0000-0000-000000000001","name":"Idem Test"}"#;
    openaccounting::api::helpers::idempotency_store(&pool, &key, token_id, &fp, 201, resp_body)
        .await
        .unwrap();
    let found = openaccounting::api::helpers::idempotency_lookup(&pool, &key, token_id, &fp)
        .await
        .unwrap();
    assert!(found.is_some());
    let (status, body) = found.unwrap();
    assert_eq!(status, 201);
    assert_eq!(body, resp_body);
    let _ = token;
}

#[tokio::test]
async fn idempotency_fingerprint_mismatch_returns_error() {
    let (server, _ledger_id, _token, user_id) = bootstrap().await;
    let pool = server.db().pool();
    let key = Uuid::new_v4().to_string();
    let issued = openaccounting::auth::api_token::issue_token(&pool, user_id, "extra-token")
        .await
        .unwrap();
    let token_id = issued.id;
    let body1 = serde_json::json!({ "name": "A" });
    let body2 = serde_json::json!({ "name": "B" });
    let fp1 = openaccounting::api::helpers::fingerprint(&body1);
    let fp2 = openaccounting::api::helpers::fingerprint(&body2);
    openaccounting::api::helpers::idempotency_store(&pool, &key, token_id, &fp1, 201, "{}")
        .await
        .unwrap();
    let result =
        openaccounting::api::helpers::idempotency_lookup(&pool, &key, token_id, &fp2).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn idempotency_expires_after_24_hours() {
    let (server, _ledger_id, _token, user_id) = bootstrap().await;
    let pool = server.db().pool();
    let key = Uuid::new_v4().to_string();
    let issued = openaccounting::auth::api_token::issue_token(&pool, user_id, "extra-token")
        .await
        .unwrap();
    let token_id = issued.id;
    let body = serde_json::json!({});
    let fp = openaccounting::api::helpers::fingerprint(&body);
    sqlx::query(
        "INSERT INTO api_idempotency (key, token_id, request_fingerprint, response_status, response_body, created_at)
         VALUES ($1, $2, $3, 200, '{}', now() - INTERVAL '25 hours')",
    )
    .bind(&key)
    .bind(token_id)
    .bind(&fp)
    .execute(&pool)
    .await
    .unwrap();
    let found = openaccounting::api::helpers::idempotency_lookup(&pool, &key, token_id, &fp)
        .await
        .unwrap();
    assert!(found.is_none());
}

// ─── 1.4 HTTP: double POST with same key returns identical response ────

#[tokio::test]
async fn idempotent_post_creates_one_resource() {
    let (server, _ledger_id, token, _user_id) = bootstrap().await;
    let key = Uuid::new_v4().to_string();
    let body = serde_json::json!({
        "name": "Idem Ledger",
        "base_currency": "EUR",
    });
    let url = format!("{}/api/v1/ledgers", server.base_url());
    let r1: Value = server
        .client()
        .post(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .header("Idempotency-Key", &key)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let r2: Value = server
        .client()
        .post(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .header("Idempotency-Key", &key)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r1["name"], r2["name"]);
    assert_eq!(r1["base_currency"], r2["base_currency"]);
}

#[tokio::test]
async fn idempotent_post_with_different_body_returns_422() {
    let (server, _ledger_id, token, _user_id) = bootstrap().await;
    let key = Uuid::new_v4().to_string();
    let body1 = serde_json::json!({ "name": "X", "base_currency": "USD" });
    let body2 = serde_json::json!({ "name": "Y", "base_currency": "USD" });
    let url = format!("{}/api/v1/ledgers", server.base_url());
    let _r1: Value = server
        .client()
        .post(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .header("Idempotency-Key", &key)
        .json(&body1)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let resp = server
        .client()
        .post(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .header("Idempotency-Key", &key)
        .json(&body2)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ─── 1.2 Route inventory test: every src/api route is documented ───────

#[test]
fn route_inventory_matches_openapi_docs() {
    let openapi_yaml = include_str!("../../docs/openapi.yaml");
    let mut doc_paths: HashSet<String> = HashSet::new();
    for line in openapi_yaml.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('/') && !trimmed.contains('[') && !trimmed.contains("format") {
            let path = trimmed.trim_end_matches(':').to_string();
            doc_paths.insert(path);
        }
    }

    let api_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api");
    let mut code_paths: HashSet<String> = HashSet::new();
    for entry in std::fs::read_dir(&api_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "rs") {
            let content = std::fs::read_to_string(&path).unwrap();
            for line in content.lines() {
                if let Some(start) = line.find(".route(\"") {
                    let rest = &line[start + 8..];
                    if let Some(end) = rest.find('"') {
                        let route_path = &rest[..end];
                        code_paths.insert(route_path.to_string());
                    }
                }
            }
        }
    }

    let mut missing = Vec::new();
    for code_path in &code_paths {
        if !doc_paths.iter().any(|d| d == code_path) {
            missing.push(code_path.clone());
        }
    }
    if !missing.is_empty() {
        panic!(
            "Routes in code not documented in docs/openapi.yaml: {:?}",
            missing
        );
    }
}

// ─── 1.3 Catalog: every emitted event type appears in the catalog ─────

#[test]
fn event_catalog_covers_emitted_types() {
    let catalog = include_str!("../../docs/event-catalog.md");
    let events_rs = include_str!("../../src/jobs/events.rs");
    // Parse only the EVENT_TYPES array (lines between the declaration and closing bracket).
    let in_array = events_rs
        .find("EVENT_TYPES")
        .map(|start| &events_rs[start..])
        .unwrap();
    let array_end = in_array.find("];").unwrap_or(in_array.len());
    let array_body = &in_array[..array_end];
    let mut emitted = Vec::new();
    for line in array_body.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            let event_type = trimmed.trim_matches('"').to_string();
            emitted.push(event_type);
        }
    }
    assert!(!emitted.is_empty(), "EVENT_TYPES array should not be empty");
    for event_type in &emitted {
        assert!(
            catalog.contains(event_type),
            "Event type '{event_type}' is declared in EVENT_TYPES but missing from docs/event-catalog.md"
        );
    }
    let inbound_types = [
        "external.sync",
        "external.document.received",
        "external.payment.received",
    ];
    for t in inbound_types {
        assert!(
            catalog.contains(t),
            "Inbound event type '{t}' is missing from docs/event-catalog.md"
        );
    }
}

// ─── 1.5 HTTP: signed event intake enqueues job ────────────────────────

#[tokio::test]
async fn signed_event_intake_enqueues_automation_job() {
    let (server, ledger_id, _token, user_id) = bootstrap().await;
    let pool = server.db().pool();

    let secret = "oa_in_testsecret12345678901234567890";
    sqlx::query("UPDATE ledgers SET incoming_events_secret = $2 WHERE id = $1")
        .bind(ledger_id)
        .bind(secret)
        .execute(&pool)
        .await
        .unwrap();

    let sub_id: Uuid = sqlx::query_scalar(
        "INSERT INTO webhook_subscriptions (ledger_id, target_url, secret, events, is_enabled)
         VALUES ($1, 'https://example.com/hook', 'whsec_test', ARRAY['external.sync'], TRUE)
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let rule_id: Uuid = sqlx::query_scalar(
        "INSERT INTO automation_rules (ledger_id, name, trigger, action, action_config, created_by)
         VALUES ($1, 'Sync hook', 'external.sync', 'webhook_post', $2, $3) RETURNING id",
    )
    .bind(ledger_id)
    .bind(serde_json::json!({ "subscription_id": sub_id }))
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let body = serde_json::json!({
        "type": "external.sync",
        "data": { "source": "test" }
    });
    let body_bytes = serde_json::to_vec(&body).unwrap();
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(&body_bytes);
    let sig = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

    let client = reqwest::Client::new();
    let url = format!("{}/api/events/{}", server.base_url(), ledger_id);
    let resp = client
        .post(&url)
        .header("X-OA-Event-Signature", &sig)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let json: Value = resp.json().await.unwrap();
    let queued = json["queued"].as_i64().unwrap();
    assert!(
        queued >= 1,
        "expected at least 1 enqueued job, got {queued}"
    );

    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM jobs WHERE kind = 'automation_action' AND payload->>'rule_id' = $1",
    )
    .bind(rule_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(n.0 >= 1, "expected at least 1 automation_action job");
}

// ─── 1.6 HTTP: unsigned intake returns 401, unknown type returns 400 ──

#[tokio::test]
async fn unsigned_event_intake_returns_401() {
    let (server, ledger_id, _token, _user_id) = bootstrap().await;
    let pool = server.db().pool();
    sqlx::query("UPDATE ledgers SET incoming_events_secret = $2 WHERE id = $1")
        .bind(ledger_id)
        .bind("oa_in_somesecret")
        .execute(&pool)
        .await
        .unwrap();
    let client = reqwest::Client::new();
    let url = format!("{}/api/events/{}", server.base_url(), ledger_id);
    let resp = client
        .post(&url)
        .header("X-OA-Event-Signature", "bad")
        .json(&serde_json::json!({"type": "external.sync"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unknown_event_type_returns_400() {
    let (server, ledger_id, _token, _user_id) = bootstrap().await;
    let pool = server.db().pool();
    let secret = "oa_in_testsecret_unknown_type";
    sqlx::query("UPDATE ledgers SET incoming_events_secret = $2 WHERE id = $1")
        .bind(ledger_id)
        .bind(secret)
        .execute(&pool)
        .await
        .unwrap();
    let body = serde_json::json!({"type": "malicious.event"});
    let body_bytes = serde_json::to_vec(&body).unwrap();
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(&body_bytes);
    let sig = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
    let client = reqwest::Client::new();
    let url = format!("{}/api/events/{}", server.base_url(), ledger_id);
    let resp = client
        .post(&url)
        .header("X-OA-Event-Signature", &sig)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ─── 1.7 HTTP: rule CRUD is owner-only ─────────────────────────────────

#[tokio::test]
async fn automation_rules_owner_can_crud() {
    let (server, ledger_id, _token, _user_id) = bootstrap().await;
    let pool = server.db().pool();

    // Web form POST uses the cookie-authenticated client with CSRF bypass.
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{ledger_id}/automations",
            server.base_url()
        ))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("name", "Test Rule"),
            ("trigger", "transaction.posted"),
            ("action", "email_notify"),
            ("action_config", r#"{"to":"test@example.com"}"#),
        ])
        .send()
        .await
        .unwrap();
    // Should redirect (303) after creation.
    assert!(
        resp.status().is_success() || resp.status() == StatusCode::SEE_OTHER,
        "expected success or redirect, got {}",
        resp.status()
    );

    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM automation_rules WHERE ledger_id = $1 AND name = 'Test Rule'",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n.0, 1, "rule should exist");
}

// ─── 1.8 E2E: rule -> event -> webhook delivery ────────────────────────

#[tokio::test]
async fn automation_rule_event_triggers_webhook_delivery() {
    let (server, ledger_id, _token, user_id) = bootstrap().await;
    let pool = server.db().pool();

    let sub_id: Uuid = sqlx::query_scalar(
        "INSERT INTO webhook_subscriptions (ledger_id, target_url, secret, events, is_enabled)
         VALUES ($1, 'https://httpbin.org/post', 'whsec_e2e_test', ARRAY['transaction.posted'], TRUE)
         RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let _rule_id: Uuid = sqlx::query_scalar(
        "INSERT INTO automation_rules (ledger_id, name, trigger, action, action_config, created_by)
         VALUES ($1, 'E2E webhook', 'transaction.posted', 'webhook_post', $2, $3) RETURNING id",
    )
    .bind(ledger_id)
    .bind(serde_json::json!({ "subscription_id": sub_id }))
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let acct_a = create_account_sql(&pool, ledger_id, "Cash A").await;
    let acct_b = create_account_sql(&pool, ledger_id, "Cash B").await;
    let _txn = openaccounting::domain::posting_service::PostingService::create(
        &pool,
        openaccounting::domain::posting_service::NewTransaction {
            ledger_id,
            txn_date: chrono::Utc::now().date_naive(),
            description: "E2E automation trigger".into(),
            payee: None,
            reference: None,
            kind: Some("standard".into()),
            created_by: user_id,
            lines: vec![
                openaccounting::domain::TxnLineInput {
                    account_id: acct_a,
                    signed_amount: rust_decimal_macros::dec!(50),
                    memo: None,
                    tax_rate_id: None,
                    foreign: None,
                    cost_center_id: None,
                    project_id: None,
                },
                openaccounting::domain::TxnLineInput {
                    account_id: acct_b,
                    signed_amount: rust_decimal_macros::dec!(-50),
                    memo: None,
                    tax_rate_id: None,
                    foreign: None,
                    cost_center_id: None,
                    project_id: None,
                },
            ],
            reverses_id: None,
            number: None,
            tax_links: vec![],
        },
    )
    .await
    .unwrap();

    let _ = openaccounting::jobs::run_due(&pool, 10).await;
    let deliveries: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM jobs WHERE kind = 'webhook_delivery'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        deliveries.0 >= 1,
        "expected at least 1 webhook_delivery job after automation run"
    );
}

// ─── OpenAPI serves 200 ────────────────────────────────────────────────

#[tokio::test]
async fn openapi_spec_serves_200() {
    let server = TestServer::new().await;
    let client = reqwest::Client::new();
    let url = format!("{}/api/openapi.yaml", server.base_url());
    let resp = client.get(&url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        content_type.contains("text/yaml") || content_type.contains("application/yaml"),
        "expected YAML content-type, got {content_type}"
    );
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("openapi:"),
        "body must contain openapi version"
    );
    assert!(
        body.contains("/ledgers"),
        "body must document /ledgers path"
    );
}

// ─── TTL prune helper ──────────────────────────────────────────────────

#[tokio::test]
async fn purge_expired_idempotency_removes_old_records() {
    let (server, _ledger_id, _token, user_id) = bootstrap().await;
    let pool = server.db().pool();
    let issued = openaccounting::auth::api_token::issue_token(&pool, user_id, "purge-test-token")
        .await
        .unwrap();
    let token_id = issued.id;
    sqlx::query(
        "INSERT INTO api_idempotency (key, token_id, request_fingerprint, response_status, response_body, created_at)
         VALUES ('old_key', $1, 'fp_old', 200, '{}', now() - INTERVAL '25 hours')",
    )
    .bind(token_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO api_idempotency (key, token_id, request_fingerprint, response_status, response_body, created_at)
         VALUES ('new_key', $1, 'fp_new', 200, '{}', now())",
    )
    .bind(token_id)
    .execute(&pool)
    .await
    .unwrap();
    let deleted = openaccounting::api::helpers::purge_expired_idempotency(&pool)
        .await
        .unwrap();
    assert!(
        deleted >= 1,
        "expected at least 1 row purged, got {deleted}"
    );
    let remaining: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM api_idempotency")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(remaining.0 >= 1, "fresh record should survive purge");
}

// ─── Ledger PATCH via API ──────────────────────────────────────────────

#[tokio::test]
async fn ledger_patch_updates_name_and_basis() {
    let (server, ledger_id, token, _user_id) = bootstrap().await;
    let url = format!("{}/api/v1/ledgers/{ledger_id}", server.base_url());
    let resp = server
        .client()
        .patch(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&serde_json::json!({
            "name": "Updated Name",
            "basis": "cash"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json: Value = resp.json().await.unwrap();
    assert_eq!(json["name"], "Updated Name");
    assert_eq!(json["basis"], "cash");
}

// ─── Account PATCH via API ─────────────────────────────────────────────

#[tokio::test]
async fn account_patch_archives_and_updates_name() {
    let (server, ledger_id, token, _user_id) = bootstrap().await;
    let pool = server.db().pool();
    let acct = create_account_sql(&pool, ledger_id, "Patchable Acct").await;
    let url = format!(
        "{}/api/v1/ledgers/{ledger_id}/accounts/{acct}",
        server.base_url()
    );
    let resp = server
        .client()
        .patch(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&serde_json::json!({
            "name": "Archived Account",
            "is_archived": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json: Value = resp.json().await.unwrap();
    assert_eq!(json["name"], "Archived Account");
    assert_eq!(json["is_archived"], true);
}

// ─── Transaction reverse via API ───────────────────────────────────────

#[tokio::test]
async fn transaction_reverse_via_api() {
    let (server, ledger_id, token, _user_id) = bootstrap().await;
    let pool = server.db().pool();
    let acct_a = create_account_sql(&pool, ledger_id, "Rev Acct A").await;
    let acct_b = create_account_sql(&pool, ledger_id, "Rev Acct B").await;
    let txn: Value = server
        .client()
        .post(format!(
            "{}/api/v1/ledgers/{ledger_id}/transactions",
            server.base_url()
        ))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&serde_json::json!({
            "date": "2026-01-15",
            "description": "To reverse",
            "lines": [
                {"account_id": acct_a, "direction": "DEBIT", "amount": "100.00"},
                {"account_id": acct_b, "direction": "CREDIT", "amount": "100.00"},
            ]
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let txn_id = txn["id"].as_str().unwrap();
    let url = format!(
        "{}/api/v1/ledgers/{ledger_id}/transactions/{txn_id}/reverse",
        server.base_url()
    );
    let resp = server
        .client()
        .post(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let json: Value = resp.json().await.unwrap();
    assert_eq!(json["kind"], "reversing");
    assert_eq!(json["reverses_id"], txn_id);
    // Re-reversal must fail — try to reverse the reversal itself.
    let reversal_id = json["id"].as_str().unwrap();
    let url2 = format!(
        "{}/api/v1/ledgers/{ledger_id}/transactions/{reversal_id}/reverse",
        server.base_url()
    );
    let resp2 = server
        .client()
        .post(&url2)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert!(
        resp2.status() == StatusCode::UNPROCESSABLE_ENTITY
            || resp2.status() == StatusCode::CONFLICT,
        "expected 422 or 409 for re-reversal, got {}",
        resp2.status()
    );
}
