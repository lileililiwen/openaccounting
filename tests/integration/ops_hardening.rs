//! Operational hardening checks (`ops-hardening`).
//!
//! Script-backed docs tests run without a database; the rate-limit,
//! backup, restore-drill, and observability tests below exercise the
//! real Axum router and a fresh PostgreSQL database.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use openaccounting::domain::posting_service::{NewTransaction, PostingService};
use openaccounting::domain::TxnLineInput;
use openaccounting::jobs::restore_verify;
use openaccounting::workers::backup as backup_worker;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_check(script: &str, args: &[&str], cwd: &Path) -> (bool, String) {
    let out = Command::new("python3")
        .arg(repo_root().join("scripts").join(script))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("python3 must be available to run ops checks");
    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), combined)
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

// ── 1.4: SECURITY.md lint ──────────────────────────────────────

#[test]
fn security_lint_fails_on_placeholder_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("SECURITY.md");
    write(&file, "# Security\n\nEmail: [INSERT SECURITY EMAIL]\n");
    let arg = format!("--security-md={}", file.display());
    let (ok, out) = run_check("check_security_contact.py", &[arg.as_str()], dir.path());
    assert!(!ok, "placeholder contact must fail; got:\n{out}");
    assert!(
        out.contains("placeholder"),
        "failure must name it; got:\n{out}"
    );
}

#[test]
fn security_lint_passes_on_real_security_md() {
    let root = repo_root();
    let (ok, out) = run_check("check_security_contact.py", &[], root.as_path());
    assert!(
        ok,
        "real SECURITY.md must name an operative contact; got:\n{out}"
    );
}

// ── PITR doc commands ──────────────────────────────────────────

#[test]
fn doc_commands_fail_on_unknown_binary_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let docs = dir.path().join("docs");
    let target = docs.join("backup-restore.md");
    write(&target, "# Doc\n\n```bash\nfrobnicate --all\n```\n");
    let arg = format!("--docs={}", target.display());
    let (ok, out) = run_check("check_doc_commands.py", &[arg.as_str()], dir.path());
    assert!(!ok, "unknown binary must fail; got:\n{out}");
    assert!(
        out.contains("frobnicate"),
        "failure must name it; got:\n{out}"
    );
}

#[test]
fn doc_commands_pass_on_real_backup_restore_doc() {
    let root = repo_root();
    let (ok, out) = run_check("check_doc_commands.py", &[], root.as_path());
    assert!(
        ok,
        "real PITR/backup doc commands must resolve; got:\n{out}"
    );
}

// ── Spec: login budget → 429 + Retry-After, no lockout ────────

const PASSWORD: &str = "X7!qZ4wN9pLk_3vR";

#[tokio::test]
async fn http_login_throttle_returns_retry_after_without_lockout() {
    let server = TestServer::new().await;
    let email = "throttle-retry@example.com";
    server
        .bootstrap_user("throttle_retry", email, PASSWORD)
        .await;

    // Five wrong passwords trip the per-account throttle; the
    // sixth attempt is rejected with 429 + Retry-After.
    for _ in 0..5 {
        let resp = server
            .client()
            .post(format!("{}/login", server.base_url()))
            .form(&[("email", email), ("password", "wrong-password")])
            .send()
            .await
            .expect("login attempt");
        let _ = resp.text().await.unwrap();
    }
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", "wrong-password")])
        .send()
        .await
        .expect("throttled attempt");
    assert_eq!(resp.status(), 429, "over-budget login must be 429");
    let retry_after = resp
        .headers()
        .get("retry-after")
        .expect("429 must carry Retry-After")
        .to_str()
        .unwrap()
        .parse::<i64>()
        .expect("Retry-After must be seconds");
    assert!(retry_after >= 1, "Retry-After must be positive");

    // Throttling is not a lockout: once the window is cleared the
    // same credentials authenticate normally.
    sqlx::query("DELETE FROM login_attempts WHERE email = $1")
        .bind(email.to_lowercase())
        .execute(&server.db().pool())
        .await
        .unwrap();
    let resp = server
        .client()
        .post(format!("{}/login", server.base_url()))
        .form(&[("email", email), ("password", PASSWORD)])
        .send()
        .await
        .expect("login after clear");
    assert!(
        resp.status().as_u16() < 400,
        "account must not be locked out, got {}",
        resp.status()
    );
}

// ── 1.3 / 1.6: backup → restore → verify drill ────────────────

// (imports live at module top; drill helpers + tests follow the
// metrics test below — see end of file for the drill section.)

#[tokio::test]
async fn http_metrics_valid_with_trace_ids() {
    let server = TestServer::new().await;
    // Seed the global recorder (blank per process) before scraping.
    let _ = server
        .client()
        .get(format!("{}/healthz", server.base_url()))
        .send()
        .await
        .expect("GET /healthz to seed recorder");
    let resp = server
        .client()
        .get(format!("{}/metrics", server.base_url()))
        .send()
        .await
        .expect("GET /metrics");
    assert_eq!(resp.status(), 200, "/metrics must be public");
    let request_id = resp
        .headers()
        .get("x-request-id")
        .cloned()
        .expect("trace-correlated x-request-id header");
    assert!(!request_id.is_empty(), "x-request-id must be non-empty");
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("# TYPE http_requests_total counter"),
        "/metrics must stay Prometheus-valid; got:\n{body}"
    );
}

// ── Drill helpers + 1.3 / 1.6 tests ────────────────────────────

/// Drill fixture: user + ledger + balanced transaction + one
/// document file. Returns (user_id, ledger_id, doc_file_count).
async fn drill_fixture(server: &TestServer, tag: &str) -> (Uuid, Uuid, u64) {
    let pool = server.db().pool();
    let email = format!("drill-{tag}@example.com");
    server
        .bootstrap_user(&format!("drill-{tag}"), &email, PASSWORD)
        .await;
    let (uid,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&pool)
        .await
        .unwrap();
    let (ledger_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO ledgers (owner_id, name, base_currency) VALUES ($1, 'Drill', 'USD') RETURNING id",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (cash,): (Uuid,) = sqlx::query_as(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency) VALUES ($1, 'Cash', 'ASSET', 'CURRENT_ASSET', 'USD') RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (sales,): (Uuid,) = sqlx::query_as(
        "INSERT INTO accounts (ledger_id, name, type, subtype, currency) VALUES ($1, 'Sales', 'INCOME', 'OPERATING_INCOME', 'USD') RETURNING id",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let created = PostingService::create(
        &pool,
        NewTransaction {
            ledger_id,
            txn_date: chrono::NaiveDate::from_ymd_opt(2026, 5, 4).unwrap(),
            description: "drill sale".into(),
            payee: None,
            reference: None,
            kind: None,
            created_by: uid,
            reverses_id: None,
            number: None,
            tax_links: vec![],
            lines: vec![
                TxnLineInput {
                    account_id: cash,
                    signed_amount: rust_decimal::Decimal::new(10000, 2),
                    memo: None,
                    tax_rate_id: None,
                    foreign: None,
                },
                TxnLineInput {
                    account_id: sales,
                    signed_amount: rust_decimal::Decimal::new(-10000, 2),
                    memo: None,
                    tax_rate_id: None,
                    foreign: None,
                },
            ],
        },
    )
    .await
    .expect("balanced posting");

    // One document row + one real file under DOCUMENTS_DIR.
    let docs_root = std::env::var("DOCUMENTS_DIR").unwrap();
    std::fs::create_dir_all(&docs_root).unwrap();
    std::fs::write(
        std::path::Path::new(&docs_root).join("drill-receipt.txt"),
        b"receipt",
    )
    .unwrap();
    sqlx::query(
        "INSERT INTO documents (transaction_id, filename, stored_filename, mime_type, size_bytes, uploaded_by)
         VALUES ($1, 'drill-receipt.txt', 'drill-receipt.txt', 'text/plain', 7, $2)",
    )
    .bind(created.id)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();
    (uid, ledger_id, 1)
}

fn drill_backup_cfg(dir: &std::path::Path) -> backup_worker::BackupConfig {
    backup_worker::BackupConfig {
        cron_expr: "0 0 2 * * * *".into(),
        keep: 7,
        dir: dir.to_path_buf(),
        pg_dump_bin: "pg_dump".into(),
        target: backup_worker::BackupTargetKind::Dir,
    }
}

async fn run_drill_backup(
    server: &TestServer,
    backup_dir: &tempfile::TempDir,
) -> (String, std::path::PathBuf) {
    let pool = server.db().pool();
    // pg_dump reads DATABASE_URL, not the pool: point it at this
    // test's database so the dump matches the fixture. Restored
    // immediately after the backup call.
    let real_url = std::env::var("DATABASE_URL").unwrap();
    std::env::set_var(
        "DATABASE_URL",
        swap_db_segment(&real_url, server.db().name()),
    );
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO backup_runs (kind, scheduled_for, status) VALUES ('manual', now(), 'running') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let cfg = drill_backup_cfg(backup_dir.path());
    let result = backup_worker::run_backup_with_pool(&pool, &cfg, run_id, chrono::Utc::now()).await;
    std::env::set_var("DATABASE_URL", &real_url);
    let filename = result.expect("backup should succeed");
    let tarball = backup_dir.path().join(&filename);
    assert!(tarball.exists(), "tarball must exist");
    (filename, tarball)
}

fn swap_db_segment(url: &str, db: &str) -> String {
    match url.rsplit_once('/') {
        Some((head, _)) => format!("{head}/{db}"),
        None => url.to_string(),
    }
}

#[tokio::test]
async fn backup_restore_verification_job_checks_invariant_and_docs() {
    let docs_dir = tempfile::tempdir().unwrap();
    std::env::set_var("DOCUMENTS_DIR", docs_dir.path());
    let server = TestServer::new().await;
    let (_uid, _ledger, _docs) = drill_fixture(&server, "verify").await;
    let backup_dir = tempfile::tempdir().unwrap();
    let (_filename, _tarball) = run_drill_backup(&server, &backup_dir).await;
    std::env::set_var("BACKUP_DIR", backup_dir.path());

    let report = restore_verify::verify_latest(&server.db().pool())
        .await
        .expect("verification must not error")
        .expect("a successful backup exists");
    assert!(report.postings >= 2, "restored postings counted");
    assert_eq!(
        report.documents_db as u64, report.documents_files,
        "document rows must match restored files"
    );
}

#[tokio::test]
async fn backup_destroy_restore_drill_meets_documented_rto() {
    let drill_started = std::time::Instant::now();
    let docs_dir = tempfile::tempdir().unwrap();
    std::env::set_var("DOCUMENTS_DIR", docs_dir.path());
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let (_uid, _ledger, doc_files) = drill_fixture(&server, "rto").await;
    let backup_dir = tempfile::tempdir().unwrap();
    let (_filename, tarball) = run_drill_backup(&server, &backup_dir).await;

    // Destroy: wipe the live books.
    sqlx::query("DELETE FROM postings")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM transactions")
        .execute(&pool)
        .await
        .unwrap();
    let remaining: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM postings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining.0, 0, "destroy step must wipe postings");

    // Restore into a scratch database + scratch documents dir.
    let database_url = std::env::var("DATABASE_URL").unwrap();
    let scratch = format!("oa_drill_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE \"{scratch}\""))
        .execute(&pool)
        .await
        .unwrap();
    let scratch_url = swap_db_segment(&database_url, &scratch);
    let restore_docs = tempfile::tempdir().unwrap();
    let stats = backup_worker::restore(&tarball, &scratch_url, restore_docs.path())
        .await
        .expect("restore must succeed");

    // Verify the restored copy: invariant + document count.
    let scratch_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&scratch_url)
        .await
        .unwrap();
    let net: Option<rust_decimal::Decimal> = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CASE WHEN direction = 'DEBIT' THEN amount ELSE -amount END), 0) FROM postings",
    )
    .fetch_one(&scratch_pool)
    .await
    .unwrap();
    assert_eq!(
        net,
        Some(rust_decimal::Decimal::ZERO),
        "restored books must balance"
    );
    assert_eq!(
        stats.doc_files, doc_files,
        "restored document files must match"
    );
    scratch_pool.close().await;
    sqlx::query(&format!("DROP DATABASE IF EXISTS \"{scratch}\""))
        .execute(&pool)
        .await
        .unwrap();

    // Documented RTO is 1 hour for databases ≤ 1 GB; the fixture
    // drill must complete two orders of magnitude inside it.
    let elapsed = drill_started.elapsed();
    assert!(
        elapsed.as_secs() < 300,
        "drill must meet RTO, took {elapsed:?}"
    );
}
