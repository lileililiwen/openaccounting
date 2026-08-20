//! Setup checklist for a ledger (`a16-onboarding-quickstart`).
//!
//! Pure read-only: every milestone is derived from existing rows, so
//! the checklist is always accurate and needs no write path or schema.

use axum::extract::{Path, State};
use axum::response::Response;
use axum_login::AuthSession;
use uuid::Uuid;

use crate::{
    auth::Backend,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::{onboarding::SetupPage, render_response},
    AppState,
};

pub use crate::templates::onboarding::{SetupStatus, SetupStep};

/// Compute the five setup milestones for a ledger.
pub async fn compute_setup_status(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
    owner_id: Uuid,
) -> AppResult<SetupStatus> {
    async fn exists(pool: &sqlx::PgPool, q: &str, ledger_id: Uuid, owner_id: Option<Uuid>) -> AppResult<bool> {
        let mut qb = sqlx::query_scalar(q).bind(ledger_id);
        if let Some(owner) = owner_id {
            qb = qb.bind(owner);
        }
        let n: i64 = qb.fetch_one(pool).await?;
        Ok(n > 0)
    }

    let opening_done = exists(
        pool,
        "SELECT COUNT(*) FROM transactions
         WHERE ledger_id = $1 AND description = 'Opening balances' AND kind != 'draft'",
        ledger_id,
        None,
    )
    .await?;
    let first_txn_done = exists(
        pool,
        "SELECT COUNT(*) FROM transactions
         WHERE ledger_id = $1 AND kind != 'draft' AND description != 'Opening balances'",
        ledger_id,
        None,
    )
    .await?;
    let documents_done = exists(
        pool,
        "SELECT COUNT(*) FROM documents d
         WHERE d.ledger_id = $1
            OR EXISTS (SELECT 1 FROM transactions t WHERE t.id = d.transaction_id AND t.ledger_id = $1)",
        ledger_id,
        None,
    )
    .await?;
    let bank_feed_done = exists(
        pool,
        "SELECT COUNT(*) FROM bank_feed_links WHERE ledger_id = $1",
        ledger_id,
        None,
    )
    .await?;
    let members_done = exists(
        pool,
        "SELECT COUNT(*) FROM ledger_members WHERE ledger_id = $1 AND user_id <> $2",
        ledger_id,
        Some(owner_id),
    )
    .await?;
    let invites_done = exists(
        pool,
        "SELECT COUNT(*) FROM ledger_invitations WHERE ledger_id = $1 AND status = 'pending'",
        ledger_id,
        None,
    )
    .await?;

    let steps = vec![
        SetupStep {
            id: "opening_balances".into(),
            label: "Set opening balances".into(),
            description: "Bring your starting money, debts, and equity into the books.".into(),
            done: opening_done,
            href: format!("/ledgers/{ledger_id}/opening-balances"),
        },
        SetupStep {
            id: "first_transaction".into(),
            label: "Record your first transaction".into(),
            description: "Enter an expense, a sale, or a transfer.".into(),
            done: first_txn_done,
            href: format!("/ledgers/{ledger_id}/transactions/new"),
        },
        SetupStep {
            id: "documents".into(),
            label: "Attach a receipt or invoice".into(),
            description: "Upload proof for a transaction.".into(),
            done: documents_done,
            href: format!("/ledgers/{ledger_id}/documents"),
        },
        SetupStep {
            id: "bank_feed".into(),
            label: "Link a bank account".into(),
            description: "Connect a bank feed or import a statement.".into(),
            done: bank_feed_done,
            href: format!("/ledgers/{ledger_id}/bank-feeds"),
        },
        SetupStep {
            id: "collaborator".into(),
            label: "Invite a collaborator".into(),
            description: "Share the books with your team or accountant.".into(),
            done: members_done || invites_done,
            href: format!("/ledgers/{ledger_id}/share"),
        },
    ];

    let done = steps.iter().filter(|s| s.done).count();
    let total = steps.len();
    Ok(SetupStatus {
        steps,
        done,
        total,
        complete: done == total,
    })
}

pub async fn setup_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let status = compute_setup_status(&state.pool, ledger_id, user.id).await?;
    Ok(render_response(SetupPage {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "dashboard".to_string(),
        status,
    }))
}
