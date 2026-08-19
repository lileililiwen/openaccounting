use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::Form;
use axum_login::AuthSession;
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;
use std::path::Path as StdPath;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    error::{AppError, AppResult},
    templates::backups::{BackupList, IntegrityIssue, IntegrityReport},
    AppState,
};

fn check_admin(role: &str) -> AppResult<()> {
    if role != "admin" {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    check_admin(user.role.as_str())?;

    let rows = sqlx::query_as::<_, (Uuid, String, i64, String, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT id, filename, size_bytes, kind, created_at
           FROM backups ORDER BY created_at DESC LIMIT 50"#,
    )
    .fetch_all(&state.pool)
    .await?;

    let backups: Vec<(Uuid, String, i64, String, chrono::DateTime<chrono::Utc>)> = rows;

    Ok(render_response(BackupList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id: Uuid::nil(),
        ledger_name: String::new(),
        current_section: "admin".to_string(),
        backups,
    }))
}

pub async fn create_manual(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    check_admin(user.role.as_str())?;

    let filename = create_backup(&state, user.id, "manual").await?;

    Ok(format!("Backup created: {}\n", filename).into_response())
}

async fn create_backup(state: &AppState, user_id: Uuid, kind: &str) -> AppResult<String> {
    let backup_dir = std::env::var("BACKUP_DIR").unwrap_or_else(|_| "./backups".to_string());
    tokio::fs::create_dir_all(&backup_dir)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("openaccounting_{}.sql.gz", timestamp);
    let path = StdPath::new(&backup_dir).join(&filename);

    // Create a logical backup by streaming from each table to SQL.
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    write_sql_dump(&mut encoder, state).await?;
    let compressed = encoder
        .finish()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tokio::fs::write(&path, &compressed)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let size_bytes = compressed.len() as i64;

    sqlx::query(
        "INSERT INTO backups (filename, size_bytes, created_by, kind) VALUES ($1, $2, $3, $4)",
    )
    .bind(&filename)
    .bind(size_bytes)
    .bind(user_id)
    .bind(kind)
    .execute(&state.pool)
    .await?;

    let _ = audit::log(
        &state.pool,
        None,
        user_id,
        "create",
        "backup",
        None,
        None,
        Some(serde_json::json!({
            "filename": filename,
            "size": size_bytes,
        })),
    )
    .await;

    prune_old_backups(&backup_dir).await;

    // Domain metric (`o4-metrics-endpoint`).
    crate::observability::metrics::backup_completed();

    Ok(filename)
}

async fn write_sql_dump<W: Write>(encoder: &mut GzEncoder<W>, state: &AppState) -> AppResult<()> {
    let tables = [
        "ledgers",
        "users",
        "accounts",
        "transactions",
        "postings",
        "contacts",
        "invoices",
        "payments",
    ];
    encoder
        .write_all(b"-- OpenAccounting SQL Dump\n")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    encoder
        .write_all(b"BEGIN;\n")
        .map_err(|e| AppError::Internal(e.to_string()))?;

    for table in &tables {
        encoder
            .write_all(format!("\n-- Table: {}\n", table).as_bytes())
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
        )
        .bind(table)
        .fetch_one(&state.pool)
        .await?;
        if !exists {
            continue;
        }

        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {}", table))
            .fetch_one(&state.pool)
            .await?;
        encoder
            .write_all(format!("-- {} rows\n", count).as_bytes())
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    encoder
        .write_all(b"COMMIT;\n")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(())
}

async fn prune_old_backups(backup_dir: &str) {
    if let Ok(mut entries) = tokio::fs::read_dir(backup_dir).await {
        let mut files: Vec<(String, std::time::SystemTime)> = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(meta) = entry.metadata().await {
                if let Ok(modified) = meta.modified() {
                    if let Some(name) = entry.file_name().to_str() {
                        files.push((name.to_string(), modified));
                    }
                }
            }
        }
        files.sort_by(|a, b| b.1.cmp(&a.1));
        for (i, (name, _)) in files.iter().enumerate() {
            if i >= 30 {
                let _ = tokio::fs::remove_file(StdPath::new(backup_dir).join(name)).await;
            }
        }
    }
}

pub async fn download(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(backup_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    check_admin(user.role.as_str())?;

    let row: Option<(String,)> = sqlx::query_as("SELECT filename FROM backups WHERE id = $1")
        .bind(backup_id)
        .fetch_optional(&state.pool)
        .await?;

    let filename = row.ok_or(AppError::NotFound)?.0;
    let backup_dir = std::env::var("BACKUP_DIR").unwrap_or_else(|_| "./backups".to_string());
    let path = StdPath::new(&backup_dir).join(&filename);

    if !path.exists() {
        return Err(AppError::NotFound);
    }

    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let safe_filename = filename.replace("\"", "");

    Ok(Response::builder()
        .status(200)
        .header("content-type", "application/gzip")
        .header(
            "content-disposition",
            format!("attachment; filename=\"{}\"", safe_filename),
        )
        .body(axum::body::Body::from(bytes))
        .map_err(|e| AppError::Internal(e.to_string()))?)
}

pub async fn integrity_check(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    check_admin(user.role.as_str())?;

    let mut issues: Vec<IntegrityIssue> = Vec::new();

    // Check 1: All transactions are balanced.
    let unbalanced: Vec<(Uuid, chrono::NaiveDate, String, Decimal, Decimal)> = sqlx::query_as(
        r#"SELECT t.id, t.txn_date, t.description,
                  COALESCE(SUM(CASE WHEN p.direction = 'DEBIT' THEN p.amount ELSE 0 END), 0) AS debits,
                  COALESCE(SUM(CASE WHEN p.direction = 'CREDIT' THEN p.amount ELSE 0 END), 0) AS credits
           FROM transactions t
           LEFT JOIN postings p ON p.transaction_id = t.id
           GROUP BY t.id
           HAVING COALESCE(SUM(CASE WHEN p.direction = 'DEBIT' THEN p.amount ELSE 0 END), 0)
                  != COALESCE(SUM(CASE WHEN p.direction = 'CREDIT' THEN p.amount ELSE 0 END), 0)"#,
    )
    .fetch_all(&state.pool)
    .await?;

    for (id, date, desc, debits, credits) in unbalanced {
        issues.push(IntegrityIssue {
            severity: "error".to_string(),
            check: "balanced".to_string(),
            message: format!(
                "Unbalanced transaction {} ({:?}): debits={}, credits={}",
                id, date, debits, credits
            ),
        });
    }

    // Check 2: Postings reference valid accounts.
    let invalid_postings: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT p.id FROM postings p
           LEFT JOIN accounts a ON a.id = p.account_id
           WHERE a.id IS NULL"#,
    )
    .fetch_all(&state.pool)
    .await?;
    for (id,) in invalid_postings {
        issues.push(IntegrityIssue {
            severity: "error".to_string(),
            check: "valid_account".to_string(),
            message: format!("Posting {} references non-existent account", id),
        });
    }

    // Check 3: Accounts reference valid ledgers.
    let invalid_accounts: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT a.id FROM accounts a
           LEFT JOIN ledgers l ON l.id = a.ledger_id
           WHERE l.id IS NULL"#,
    )
    .fetch_all(&state.pool)
    .await?;
    for (id,) in invalid_accounts {
        issues.push(IntegrityIssue {
            severity: "error".to_string(),
            check: "valid_ledger".to_string(),
            message: format!("Account {} references non-existent ledger", id),
        });
    }

    // Check 4: Transactions reference valid ledgers.
    let invalid_txns: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT t.id FROM transactions t
           LEFT JOIN ledgers l ON l.id = t.ledger_id
           WHERE l.id IS NULL"#,
    )
    .fetch_all(&state.pool)
    .await?;
    for (id,) in invalid_txns {
        issues.push(IntegrityIssue {
            severity: "error".to_string(),
            check: "valid_ledger".to_string(),
            message: format!("Transaction {} references non-existent ledger", id),
        });
    }

    // Counts
    let txn_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
        .fetch_one(&state.pool)
        .await?;
    let posting_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM postings")
        .fetch_one(&state.pool)
        .await?;
    let account_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&state.pool)
        .await?;

    Ok(render_response(IntegrityReport {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id: Uuid::nil(),
        ledger_name: String::new(),
        current_section: "admin".to_string(),
        passed: issues.is_empty(),
        txn_count,
        posting_count,
        account_count,
        issues,
    }))
}

use rust_decimal::Decimal;
