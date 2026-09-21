//! Recurring journal templates (`accounting-dimensions`).
//!
//! Templates generate preview drafts per period; posting is explicit
//! (promote the draft) or via the run-due endpoint. Idempotency is a
//! UNIQUE(template_id, period_key) guard: double-fire returns the
//! existing run. Skipped periods generate nothing; later periods
//! continue. Pause halts generation entirely.

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_login::AuthSession;
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    domain::posting_service::{NewTransaction, PostingService, PostingServiceError},
    domain::transaction::TxnLineInput,
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::recurring::{RecurringJournalList, RecurringJournalNew, RunRow, TemplateRow},
    AppState,
};

/// First date of the period after `d` for `frequency`.
pub fn advance_period(d: NaiveDate, frequency: &str) -> NaiveDate {
    match frequency {
        "weekly" => d + chrono::Duration::days(7),
        "quarterly" => add_months(d, 3),
        "yearly" => add_months(d, 12),
        _ => add_months(d, 1), // monthly default
    }
}

fn add_months(d: NaiveDate, n: u32) -> NaiveDate {
    let total = d.month0() + n;
    let year = d.year() + (total / 12) as i32;
    let month = total % 12 + 1;
    let day = d.day().min(last_day_of_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).unwrap_or(d)
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(y, m, 1)
        .unwrap()
        .pred_opt()
        .map(|d| d.day())
        .unwrap_or(28)
}

/// One template line row: account, direction, amount, memo, dims.
type TemplateLineRow = (
    Uuid,
    String,
    Decimal,
    Option<String>,
    Option<Uuid>,
    Option<Uuid>,
);

/// One due-template row for the scheduler pass.
type DueTemplateRow = (
    Uuid,
    String,
    String,
    NaiveDate,
    Option<NaiveDate>,
    Option<i32>,
);

pub fn period_key(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

struct TemplateLine {
    account_id: Uuid,
    direction: String,
    amount: Decimal,
    memo: String,
    cost_center_id: Option<Uuid>,
    project_id: Option<Uuid>,
}

/// Parse `lines[N][field]` groups from the raw urlencoded body.
fn parse_template_lines(body: &str) -> Result<Vec<TemplateLine>, AppError> {
    let mut by_index: std::collections::BTreeMap<usize, std::collections::HashMap<String, String>> =
        std::collections::BTreeMap::new();
    for (k, v) in form_urlencoded::parse(body.as_bytes()) {
        if !k.starts_with("lines[") {
            continue;
        }
        let after = &k[6..];
        let Some(close) = after.find("][") else {
            continue;
        };
        let Ok(idx) = after[..close].parse::<usize>() else {
            continue;
        };
        let field = after[close + 2..].trim_end_matches(']');
        by_index
            .entry(idx)
            .or_default()
            .insert(field.to_string(), v.into_owned());
    }
    let mut out = Vec::new();
    for (_, m) in by_index {
        let get = |k: &str| m.get(k).cloned().unwrap_or_default();
        let account_id = Uuid::parse_str(get("account_id").trim())
            .map_err(|_| AppError::Validation("invalid account on a template line".into()))?;
        let amount: Decimal = get("amount")
            .trim()
            .parse()
            .map_err(|_| AppError::Validation("invalid amount on a template line".into()))?;
        if amount <= Decimal::ZERO {
            return Err(AppError::Validation(
                "template line amounts must be positive".into(),
            ));
        }
        let direction = get("direction").trim().to_uppercase();
        if direction != "DEBIT" && direction != "CREDIT" {
            return Err(AppError::Validation(
                "template line direction must be DEBIT or CREDIT".into(),
            ));
        }
        let opt_id = |raw: String| -> Result<Option<Uuid>, AppError> {
            let s = raw.trim();
            if s.is_empty() {
                return Ok(None);
            }
            Uuid::parse_str(s)
                .map(Some)
                .map_err(|_| AppError::Validation("invalid dimension on a template line".into()))
        };
        out.push(TemplateLine {
            account_id,
            direction,
            amount,
            memo: get("memo"),
            cost_center_id: opt_id(get("cost_center_id"))?,
            project_id: opt_id(get("project_id"))?,
        });
    }
    Ok(out
        .into_iter()
        .filter(|l| l.amount > Decimal::ZERO)
        .collect())
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    body: String,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    // Single body extractor: scalars and `lines[N][field]` groups both
    // come from the raw urlencoded body (serde_urlencoded cannot nest).
    let mut scalar: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (k, v) in form_urlencoded::parse(body.as_bytes()) {
        if !k.starts_with("lines[") {
            scalar.entry(k.into_owned()).or_insert(v.into_owned());
        }
    }
    let get = |k: &str| scalar.get(k).cloned().unwrap_or_default();

    let name = get("name");
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }
    let frequency = {
        let f = get("frequency");
        if f.is_empty() {
            "monthly".to_string()
        } else {
            f
        }
    };
    if !["weekly", "monthly", "quarterly", "yearly"].contains(&frequency.as_str()) {
        return Err(AppError::Validation("invalid frequency".into()));
    }
    let start_date = NaiveDate::parse_from_str(get("start_date").trim(), "%Y-%m-%d")
        .map_err(|_| AppError::Validation("invalid start date".into()))?;
    let end_date: Option<NaiveDate> = match get("end_date").trim() {
        "" => None,
        s => Some(
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| AppError::Validation("invalid end date".into()))?,
        ),
    };
    if let Some(e) = end_date {
        if e < start_date {
            return Err(AppError::Validation("end date is before start date".into()));
        }
    }
    let max_occurrences: Option<i32> = match get("max_occurrences").trim() {
        "" => None,
        s => Some(
            s.parse()
                .map_err(|_| AppError::Validation("invalid occurrence cap".into()))?,
        ),
    };
    if let Some(n) = max_occurrences {
        if n <= 0 {
            return Err(AppError::Validation(
                "occurrence cap must be positive".into(),
            ));
        }
    }
    let description = get("description");

    let lines = parse_template_lines(&body)?;
    if lines.len() < 2 {
        return Err(AppError::Validation(
            "a template needs at least two lines".into(),
        ));
    }
    let debits: Decimal = lines
        .iter()
        .filter(|l| l.direction == "DEBIT")
        .map(|l| l.amount)
        .sum();
    let credits: Decimal = lines
        .iter()
        .filter(|l| l.direction == "CREDIT")
        .map(|l| l.amount)
        .sum();
    if debits != credits {
        return Err(AppError::Validation(format!(
            "template lines do not balance (debits={debits}, credits={credits})"
        )));
    }

    let mut tx = state.pool.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO recurring_journal_templates
           (ledger_id, name, description, frequency, start_date, end_date,
            max_occurrences, next_period, created_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $5, $8) RETURNING id"#,
    )
    .bind(ledger_id)
    .bind(name)
    .bind(description.as_str())
    .bind(frequency.as_str())
    .bind(start_date)
    .bind(end_date)
    .bind(max_occurrences)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    for l in &lines {
        sqlx::query(
            r#"INSERT INTO recurring_journal_lines
               (template_id, account_id, direction, amount, memo, cost_center_id, project_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(id)
        .bind(l.account_id)
        .bind(&l.direction)
        .bind(l.amount)
        .bind(if l.memo.is_empty() {
            None
        } else {
            Some(&l.memo)
        })
        .bind(l.cost_center_id)
        .bind(l.project_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "create",
        "recurring_journal_template",
        Some(id),
        None,
        Some(serde_json::json!({ "name": name, "frequency": frequency })),
    )
    .await;

    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/recurring-journals")).into_response())
}

/// Generate (or return) the preview draft for a template period.
/// Idempotent: the UNIQUE(template_id, period_key) guard makes a
/// double-fire return the existing run without a second draft.
#[allow(clippy::too_many_arguments)]
pub async fn preview_for_period(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
    template_id: Uuid,
    actor: Uuid,
    period: NaiveDate,
    description: &str,
) -> Result<(Uuid, bool), AppError> {
    let key = period_key(period);
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM recurring_journal_runs WHERE template_id = $1 AND period_key = $2",
    )
    .bind(template_id)
    .bind(&key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?;
    if let Some((id,)) = existing {
        return Ok((id, false));
    }

    let lines: Vec<TemplateLineRow> = sqlx::query_as(
        "SELECT account_id, direction, amount, memo, cost_center_id, project_id
             FROM recurring_journal_lines WHERE template_id = $1 ORDER BY amount DESC",
    )
    .bind(template_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Db)?;

    let inputs: Vec<TxnLineInput> = lines
        .into_iter()
        .map(
            |(account_id, direction, amount, memo, cc, proj)| TxnLineInput {
                account_id,
                signed_amount: if direction == "DEBIT" {
                    amount
                } else {
                    -amount
                },
                memo,
                tax_rate_id: None,
                foreign: None,
                cost_center_id: cc,
                project_id: proj,
            },
        )
        .collect();

    let created = PostingService::create_draft(
        pool,
        NewTransaction {
            ledger_id,
            txn_date: period,
            description: format!("{description} ({key})"),
            payee: None,
            reference: Some(format!("recurring:{template_id}:{key}")),
            lines: inputs,
            kind: None,
            created_by: actor,
            reverses_id: None,
            number: None,
            tax_links: vec![],
        },
    )
    .await
    .map_err(|e| match e {
        PostingServiceError::Unbalanced { debits, credits } => {
            AppError::Validation(format!("template does not balance ({debits}/{credits})"))
        }
        PostingServiceError::UnknownCostCenter(id) => {
            AppError::Validation(format!("unknown cost center {id}"))
        }
        PostingServiceError::UnknownProject(id) => {
            AppError::Validation(format!("unknown project {id}"))
        }
        PostingServiceError::Db(e) => AppError::Db(e),
        other => AppError::Validation(other.to_string()),
    })?;

    // Race guard: a concurrent fire wins the unique index; return it.
    let run: Option<(Uuid,)> = sqlx::query_as(
        r#"INSERT INTO recurring_journal_runs (template_id, period_key, draft_txn_id, status)
           VALUES ($1, $2, $3, 'preview')
           ON CONFLICT (template_id, period_key) DO NOTHING
           RETURNING id"#,
    )
    .bind(template_id)
    .bind(&key)
    .bind(created.id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?;
    let Some((run_id,)) = run else {
        // Lost the race — delete the orphan draft to avoid phantom
        // previews, then return the winner.
        let _ = PostingService::delete_draft(pool, ledger_id, created.id, actor).await;
        let (winner,): (Uuid,) = sqlx::query_as(
            "SELECT id FROM recurring_journal_runs WHERE template_id = $1 AND period_key = $2",
        )
        .bind(template_id)
        .bind(&key)
        .fetch_one(pool)
        .await
        .map_err(AppError::Db)?;
        return Ok((winner, false));
    };
    Ok((run_id, true))
}

#[derive(Deserialize)]
pub struct PreviewForm {
    pub period: Option<String>,
}

/// POST preview one period as a draft (no posting).
pub async fn preview(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, template_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<PreviewForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let tpl: Option<(String, String, NaiveDate)> = sqlx::query_as(
        "SELECT name, frequency, next_period FROM recurring_journal_templates
         WHERE id = $1 AND ledger_id = $2",
    )
    .bind(template_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((name, _frequency, next_period)) = tpl else {
        return Err(AppError::NotFound);
    };
    let period = match form.period.as_deref().map(str::trim) {
        None | Some("") => next_period,
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| AppError::Validation("invalid period".into()))?,
    };

    let (run_id, created) =
        preview_for_period(&state.pool, ledger_id, template_id, user.id, period, &name).await?;
    if created {
        let _ = audit::log(
            &state.pool,
            Some(ledger_id),
            user.id,
            "preview",
            "recurring_journal_run",
            Some(run_id),
            None,
            Some(serde_json::json!({ "period": period_key(period) })),
        )
        .await;
    }
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/recurring-journals")).into_response())
}

#[derive(Deserialize)]
pub struct SkipForm {
    pub period_key: String,
}

/// POST skip a period: no draft is generated for it; later periods continue.
pub async fn skip(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, template_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<SkipForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let key = form.period_key.trim();
    if key.is_empty() {
        return Err(AppError::Validation("period is required".into()));
    }
    let exists: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM recurring_journal_templates WHERE id = $1 AND ledger_id = $2",
    )
    .bind(template_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    sqlx::query(
        r#"INSERT INTO recurring_journal_runs (template_id, period_key, status)
           VALUES ($1, $2, 'skipped')"#,
    )
    .bind(template_id)
    .bind(key)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db) = &e {
            if db.code().as_deref() == Some("23505") {
                return AppError::Conflict(format!("period {key} already has a run"));
            }
        }
        AppError::Db(e)
    })?;

    // Skipping the next due period advances the cursor past it;
    // other periods are untouched, so later periods continue.
    if let Ok(key_date) = NaiveDate::parse_from_str(key, "%Y-%m-%d") {
        let tpl: Option<(NaiveDate, String)> = sqlx::query_as(
            "SELECT next_period, frequency FROM recurring_journal_templates WHERE id = $1",
        )
        .bind(template_id)
        .fetch_optional(&state.pool)
        .await?;
        if let Some((next_period, frequency)) = tpl {
            if next_period == key_date {
                sqlx::query(
                    "UPDATE recurring_journal_templates SET next_period = $1 WHERE id = $2",
                )
                .bind(advance_period(key_date, &frequency))
                .bind(template_id)
                .execute(&state.pool)
                .await?;
            }
        }
    }

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "skip",
        "recurring_journal_run",
        Some(template_id),
        None,
        Some(serde_json::json!({ "period": key })),
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/recurring-journals")).into_response())
}

/// POST pause / resume a template.
pub async fn set_paused(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, template_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let paused = matches!(
        form.get("paused").map(String::as_str),
        Some("1") | Some("true")
    );
    sqlx::query(
        "UPDATE recurring_journal_templates SET is_paused = $1 WHERE id = $2 AND ledger_id = $3",
    )
    .bind(paused)
    .bind(template_id)
    .bind(ledger_id)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/recurring-journals")).into_response())
}

/// POST promote a preview run's draft to a posted journal.
pub async fn post_run(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let run: Option<(Uuid, Option<Uuid>, Option<Uuid>, String)> = sqlx::query_as(
        r#"SELECT r.template_id, r.draft_txn_id, r.posted_txn_id, r.status
           FROM recurring_journal_runs r
           JOIN recurring_journal_templates t ON t.id = r.template_id
           WHERE r.id = $1 AND t.ledger_id = $2"#,
    )
    .bind(run_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((_tpl, draft_txn, posted_txn, status)) = run else {
        return Err(AppError::NotFound);
    };
    if status == "posted" || posted_txn.is_some() {
        return Ok(
            Redirect::to(&format!("/ledgers/{ledger_id}/recurring-journals")).into_response(),
        );
    }
    let Some(draft_id) = draft_txn else {
        return Err(AppError::Validation("run has no draft to post".into()));
    };

    let created = PostingService::post_draft(&state.pool, ledger_id, draft_id, user.id)
        .await
        .map_err(|e| match e {
            PostingServiceError::PeriodClosed { year, .. } => AppError::Validation(format!(
                "Period {year} is closed. Cannot post into closed periods."
            )),
            other => AppError::Validation(other.to_string()),
        })?;
    sqlx::query(
        "UPDATE recurring_journal_runs SET posted_txn_id = $1, status = 'posted' WHERE id = $2",
    )
    .bind(created.id)
    .bind(run_id)
    .execute(&state.pool)
    .await?;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/recurring-journals")).into_response())
}

/// POST run all due templates once (scheduler entry point, also HTTP).
/// Each due period generates at most one preview draft — the unique run
/// guard makes double-fire safe.
pub async fn run_due(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let today = chrono::Utc::now().date_naive();

    let templates: Vec<DueTemplateRow> = sqlx::query_as(
        r#"SELECT id, name, frequency, next_period, end_date, max_occurrences
               FROM recurring_journal_templates
               WHERE ledger_id = $1 AND is_paused = FALSE AND next_period <= $2"#,
    )
    .bind(ledger_id)
    .bind(today)
    .fetch_all(&state.pool)
    .await?;

    let mut generated = 0;
    for (id, name, frequency, mut period, end_date, max_occ) in templates {
        // Occurrence cap counts generated (non-skipped) runs.
        if let Some(cap) = max_occ {
            let (n,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM recurring_journal_runs
                 WHERE template_id = $1 AND status != 'skipped'",
            )
            .bind(id)
            .fetch_one(&state.pool)
            .await?;
            if n >= cap as i64 {
                continue;
            }
        }
        // Catch up at most 12 periods per fire (mirrors template_scan).
        for _ in 0..12 {
            if period > today {
                break;
            }
            if let Some(e) = end_date {
                if period > e {
                    break;
                }
            }
            let key = period_key(period);
            let existing: Option<(String,)> = sqlx::query_as(
                "SELECT status FROM recurring_journal_runs WHERE template_id = $1 AND period_key = $2",
            )
            .bind(id)
            .bind(&key)
            .fetch_optional(&state.pool)
            .await?;
            match existing.map(|(s,)| s).as_deref() {
                // Idempotent: already handled (preview/posted) or
                // skipped — advance past it either way.
                Some(_) => {
                    period = advance_period(period, &frequency);
                }
                None => {
                    let (_run, created) =
                        preview_for_period(&state.pool, ledger_id, id, user.id, period, &name)
                            .await?;
                    if created {
                        generated += 1;
                    }
                    period = advance_period(period, &frequency);
                }
            }
        }
        sqlx::query("UPDATE recurring_journal_templates SET next_period = $1 WHERE id = $2")
            .bind(period)
            .bind(id)
            .execute(&state.pool)
            .await?;
    }

    let _ = audit::log(
        &state.pool,
        Some(ledger_id),
        user.id,
        "run_due",
        "recurring_journal_template",
        None,
        None,
        Some(serde_json::json!({ "generated": generated })),
    )
    .await;
    Ok(Redirect::to(&format!("/ledgers/{ledger_id}/recurring-journals")).into_response())
}

/// GET template list with runs.
pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let templates = sqlx::query_as::<_, TemplateRow>(
        r#"SELECT id, name, frequency, start_date, next_period, is_paused
           FROM recurring_journal_templates WHERE ledger_id = $1 ORDER BY name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    let runs = sqlx::query_as::<_, RunRow>(
        r#"SELECT r.id, r.template_id, t.name AS template_name, r.period_key,
                  r.draft_txn_id, r.posted_txn_id, r.status
           FROM recurring_journal_runs r
           JOIN recurring_journal_templates t ON t.id = r.template_id
           WHERE t.ledger_id = $1 ORDER BY r.period_key DESC LIMIT 200"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(crate::templates::render_response(RecurringJournalList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        templates,
        runs,
    }))
}

/// GET new-template form (accounts + dimensions for the line rows).
pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    let accounts = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM accounts WHERE ledger_id = $1 AND is_archived = FALSE ORDER BY name",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    let cost_centers = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM cost_centers WHERE ledger_id = $1 ORDER BY name",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    let projects = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM projects WHERE ledger_id = $1 ORDER BY name",
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(crate::templates::render_response(RecurringJournalNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        accounts,
        cost_centers,
        projects,
        start_date: chrono::Utc::now().date_naive(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_period_steps_each_frequency() {
        let d = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        assert_eq!(
            advance_period(d, "monthly"),
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        );
        assert_eq!(
            advance_period(d, "quarterly"),
            NaiveDate::from_ymd_opt(2026, 4, 30).unwrap()
        );
        assert_eq!(
            advance_period(d, "yearly"),
            NaiveDate::from_ymd_opt(2027, 1, 31).unwrap()
        );
        assert_eq!(
            advance_period(d, "weekly"),
            NaiveDate::from_ymd_opt(2026, 2, 7).unwrap()
        );
    }
}
