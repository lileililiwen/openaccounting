//! Payee suggestions + retroactive rule application (`payee-learning`).

use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use axum_login::AuthSession;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    domain::{payee_learning, reconciliation_rules},
    error::{AppError, AppResult},
    handlers::ledgers,
    AppState,
};

/// GET /ledgers/{id}/rules/suggest?q=acme — JSON for the entry-form
/// autocomplete: canonical payee, suggested account, confidence.
pub async fn suggest(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let q = params.get("q").cloned().unwrap_or_default();
    let suggestions = payee_learning::suggest(&state.pool, ledger_id, &q, 8).await?;
    Ok(Json(serde_json::json!({
        "data": suggestions.iter().map(|s| serde_json::json!({
            "payee": s.canonical_payee,
            "account_id": s.account_id,
            "hit_count": s.hit_count,
            "confidence": (s.confidence * 100.0).round() / 100.0,
            "auto_fill": s.confidence >= payee_learning::AUTO_FILL_THRESHOLD,
        })).collect::<Vec<_>>(),
    }))
    .into_response())
}

/// Parsed from the raw urlencoded body: checkbox groups repeat
/// `transaction_ids`, which `serde_urlencoded` cannot express.
struct ApplyRulesForm {
    transaction_ids: Vec<String>,
    mode: String,
}

impl ApplyRulesForm {
    fn parse(body: &str) -> Self {
        let mut out = ApplyRulesForm {
            transaction_ids: Vec::new(),
            mode: String::new(),
        };
        for (k, v) in form_urlencoded::parse(body.as_bytes()) {
            match k.as_ref() {
                "transaction_ids" => out.transaction_ids.push(v.into_owned()),
                "mode" => out.mode = v.into_owned(),
                _ => {}
            }
        }
        out
    }
}

/// POST /ledgers/{id}/rules/apply — apply reconciliation rules +
/// payee aliases retroactively to selected transactions. Dry-run
/// returns the proposed diff; apply rewrites the contra posting's
/// account in one audited batch and refuses closed periods.
pub async fn apply_rules(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    body: String,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_owner(&state, user.id, ledger_id).await?;
    let form = ApplyRulesForm::parse(&body);

    let ids: Vec<Uuid> = form
        .transaction_ids
        .iter()
        .filter_map(|s| Uuid::parse_str(s.trim()).ok())
        .collect();
    if ids.is_empty() {
        return Err(AppError::Validation(
            "select at least one transaction".into(),
        ));
    }

    // Closed-period guard: any selected txn in a closed year aborts the
    // whole batch.
    let closed_hit: Option<(i32,)> = sqlx::query_as(
        r#"SELECT cp.period_year FROM transactions t
           JOIN closed_periods cp ON cp.ledger_id = t.ledger_id
                AND cp.period_year = EXTRACT(YEAR FROM t.txn_date)::INT
           WHERE t.id = ANY($1) LIMIT 1"#,
    )
    .bind(&ids)
    .fetch_optional(&state.pool)
    .await?;
    if closed_hit.is_some() {
        return Err(AppError::Conflict(
            "selection includes transactions in a closed period; nothing was changed".into(),
        ));
    }

    let rule_rows: Vec<(Uuid, String, serde_json::Value, serde_json::Value)> = sqlx::query_as(
        "SELECT id, name, predicate, action FROM reconciliation_rules
         WHERE ledger_id = $1 AND kind = 'categorize' AND is_active",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    let rules: Vec<reconciliation_rules::Rule> = rule_rows
        .into_iter()
        .map(|(id, name, predicate, action)| reconciliation_rules::Rule {
            id,
            name,
            kind: reconciliation_rules::RuleKind::Categorize,
            priority: 0,
            predicate,
            action,
            is_active: true,
        })
        .collect();

    // Build the diff: for each transaction, does a rule match its
    // description/payee, and what would the new contra account be?
    let mut diffs: Vec<(Uuid, Option<Uuid>, Uuid, String)> = Vec::new(); // txn, old acct, new acct, rule name
    for txn_id in &ids {
        let row: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT description, COALESCE(payee, '') FROM transactions WHERE id = $1 AND ledger_id = $2",
        )
        .bind(txn_id)
        .bind(ledger_id)
        .fetch_optional(&state.pool)
        .await?;
        let Some((description, payee)) = row else {
            continue;
        };
        let payee = payee.unwrap_or_default();
        let amount_cents: i64 = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM(CASE WHEN p.direction='DEBIT' THEN p.amount ELSE -p.amount END), 0) * 100
               FROM postings p WHERE p.transaction_id = $1"#,
        )
        .bind(txn_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);
        let line = reconciliation_rules::Line {
            description: format!("{description} {payee}"),
            amount_cents,
            currency: String::new(),
        };
        let Some((rule, action)) = reconciliation_rules::first_match(&rules, &line) else {
            continue;
        };
        let new_account = match action {
            reconciliation_rules::AppliedAction::Categorize { gl_account_id } => gl_account_id,
            _ => continue,
        };
        // The contra posting = the income/expense-side leg.
        let old_account: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT p.account_id FROM postings p
               JOIN accounts a ON a.id = p.account_id
               WHERE p.transaction_id = $1 AND a.type IN ('INCOME', 'EXPENSE')
               ORDER BY p.amount DESC LIMIT 1"#,
        )
        .bind(txn_id)
        .fetch_optional(&state.pool)
        .await?;
        if old_account == Some(new_account) {
            continue; // already categorized correctly
        }
        diffs.push((*txn_id, old_account, new_account, rule.name.clone()));
    }

    if form.mode != "apply" {
        return Ok(Json(serde_json::json!({
            "mode": "dry-run",
            "changes": diffs.iter().map(|(t, o, n, rule)| serde_json::json!({
                "transaction_id": t,
                "old_account": o,
                "new_account": n,
                "rule": rule,
            })).collect::<Vec<_>>(),
        }))
        .into_response());
    }

    // Apply in one transaction: every change or none.
    let mut tx = state.pool.begin().await?;
    for (txn_id, _old, new_account, _rule) in &diffs {
        sqlx::query(
            r#"UPDATE postings SET account_id = $2
               WHERE transaction_id = $1
                 AND account_id IN (
                     SELECT p.account_id FROM postings p
                     JOIN accounts a ON a.id = p.account_id
                     WHERE p.transaction_id = $1 AND a.type IN ('INCOME', 'EXPENSE')
                     ORDER BY p.amount DESC LIMIT 1)"#,
        )
        .bind(txn_id)
        .bind(new_account)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "retroactive_apply",
        "transaction",
        None,
        None,
        Some(serde_json::json!({
            "selected": ids.len(),
            "changed": diffs.len(),
        })),
    )
    .await;

    Ok(render_response(
        crate::templates::rules_apply::ApplyResult {
            user_id: user.id,
            username: user.username.clone(),
            user_role: user.role.clone(),
            ledger_id,
            ledger_name: String::new(),
            current_section: "transactions".to_string(),
            changed: diffs.len() as i64,
            selected: ids.len() as i64,
        },
    ))
}
