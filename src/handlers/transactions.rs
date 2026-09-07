use crate::templates::render_response;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_login::AuthSession;
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    audit,
    auth::Backend,
    domain::{Account, Direction, Transaction, TxnLineInput},
    error::{AppError, AppResult},
    handlers::ledgers,
    templates::transactions::{
        TransactionFilter, TransactionForm, TransactionFormLine, TransactionList, TransactionNew,
        TransactionRow, TransactionShow, TransactionShowLine,
    },
    AppState,
};

/// Group accounts by type so the picker can render `<optgroup>`
/// labels (`ux-transaction-entry`). The query already orders by
/// type, so groups arrive in a stable order.
pub fn group_accounts(accounts: Vec<Account>) -> Vec<(String, Vec<Account>)> {
    let mut groups: Vec<(String, Vec<Account>)> = Vec::new();
    for a in accounts {
        let label = a.r#type.clone();
        match groups.iter_mut().find(|(t, _)| *t == label) {
            Some(g) => g.1.push(a),
            None => groups.push((label, vec![a])),
        }
    }
    groups
}

/// An active tax rate available on the entry form (`a14-tax-on-transactions`).
#[derive(Clone, Debug)]
struct ActiveTaxRate {
    id: Uuid,
    name: String,
    rate: Decimal,
    account_id: Uuid,
}

/// Load the ledger's active tax rates for the per-line tax picker.
async fn load_active_tax_rates(
    pool: &sqlx::PgPool,
    ledger_id: Uuid,
) -> AppResult<Vec<ActiveTaxRate>> {
    Ok(sqlx::query_as::<_, (Uuid, String, Decimal, Uuid)>(
        r#"SELECT id, name, rate, account_id FROM tax_rates
           WHERE ledger_id = $1 AND is_active = TRUE
           ORDER BY kind, name"#,
    )
    .bind(ledger_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(id, name, rate, account_id)| ActiveTaxRate {
        id,
        name,
        rate,
        account_id,
    })
    .collect())
}

pub async fn list(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;

    // If the URL has no filter params and the user has a
    // default saved search, apply its query (`u2`).
    // If the URL has no filter params and the user has a
    // default saved search, apply its query (`u2`).
    let q = if q.is_empty() {
        if let Some(default_query) =
            crate::handlers::saved_searches::default_query(&state.pool, user.id).await?
        {
            url_to_query(&default_query)
        } else {
            q
        }
    } else {
        q
    };

    let filter = TransactionFilter {
        from: q.get("from").map(|s| s.to_string()).unwrap_or_default(),
        to: q.get("to").map(|s| s.to_string()).unwrap_or_default(),
        account_id: q
            .get("account_id")
            .map(|s| s.to_string())
            .unwrap_or_default(),
        q: q.get("q").map(|s| s.to_string()).unwrap_or_default(),
    };

    let limit: i64 = q.get("limit").and_then(|s| s.parse().ok()).unwrap_or(100);

    let from_date = NaiveDate::parse_from_str(&filter.from, "%Y-%m-%d").ok();
    let to_date = NaiveDate::parse_from_str(&filter.to, "%Y-%m-%d").ok();
    let _account_uuid = Uuid::parse_str(&filter.account_id).ok();
    let q_pattern = if filter.q.is_empty() {
        None
    } else {
        Some(filter.q.as_str())
    };

    let rows = sqlx::query_as::<_, TransactionRow>(
        r#"
        SELECT t.id, t.txn_date AS date, t.description, COALESCE(t.payee, '') AS payee, t.currency, t.kind,
               COALESCE((SELECT SUM(p.amount) FROM postings p WHERE p.transaction_id=t.id AND p.direction='DEBIT'), 0) AS total,
               (SELECT COUNT(*) FROM documents d WHERE d.transaction_id = t.id) AS doc_count,
               COALESCE((SELECT array_agg(tg.name)
                         FROM transaction_tags tt
                         JOIN tags tg ON tg.id = tt.tag_id
                         WHERE tt.transaction_id = t.id), ARRAY[]::text[]) AS tags
        FROM transactions t
        WHERE t.ledger_id = $1
          AND ($2::date IS NULL OR t.txn_date >= $2)
          AND ($3::date IS NULL OR t.txn_date <= $3)
          AND ($4::text IS NULL OR t.description ILIKE '%' || $4 || '%' OR COALESCE(t.payee,'') ILIKE '%' || $4 || '%')
        ORDER BY t.txn_date DESC, t.created_at DESC
        LIMIT $5
        "#,
    )
    .bind(ledger_id)
    .bind(from_date)
    .bind(to_date)
    .bind(q_pattern)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    // Surface the currently-applied query string so the saved-
    // searches picker can store / re-apply it.
    let current_query = hashmap_to_query_string(&q);

    Ok(render_response(TransactionList {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        transactions: rows,
        filter,
        current_query,
    }))
}

/// Render a `HashMap<String, String>` as the `key=value&…`
/// fragment used by the transactions list endpoint.
fn hashmap_to_query_string(q: &std::collections::HashMap<String, String>) -> String {
    let mut parts: Vec<String> = q
        .iter()
        .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
        .collect();
    parts.sort();
    parts.join("&")
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else {
            for b in c.to_string().as_bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// Parse a raw query string into a `HashMap<String, String>`.
fn url_to_query(s: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for pair in s.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            out.insert(k.to_string(), v.to_string());
        }
    }
    out
}

pub async fn new_page(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let accounts = sqlx::query_as::<_, Account>(
        r#"SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
           FROM accounts WHERE ledger_id = $1 AND is_archived = FALSE
           ORDER BY type, code NULLS LAST, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;
    let tax_rates: Vec<(Uuid, String, String)> = load_active_tax_rates(&state.pool, ledger_id)
        .await?
        .iter()
        .map(|t| {
            let pct = (t.rate * Decimal::new(100, 0)).round_dp(0);
            (t.id, format!("{} ({}%)", t.name, pct), t.rate.to_string())
        })
        .collect();
    Ok(render_response(TransactionNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        currency: ledger.base_currency.clone(),
        account_groups: group_accounts(accounts),
        error: String::new(),
        bind_doc: params.get("bind_doc").cloned().unwrap_or_default(),
        form: TransactionForm::empty(),
        tax_rates,
    }))
}

#[derive(Deserialize)]
pub struct NewTxnForm {
    pub date: String,
    pub description: String,
    pub payee: Option<String>,
    pub reference: Option<String>,
    /// Form fields for postings arrive as
    /// `lines[N][account_id]`, `lines[N][direction]`, `lines[N][amount]`,
    /// `lines[N][memo]`. Axum's Form extractor uses serde_urlencoded, which
    /// does not flatten bracketed keys into a nested struct. We catch-all
    /// into a `HashMap<String, String>` and parse manually below.
    #[serde(flatten, default)]
    pub extra: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub struct ParsedLine {
    pub account_id: String,
    pub direction: String,
    pub amount: String,
    pub memo: String,
    /// Optional active tax rate id (`a14-tax-on-transactions`).
    pub tax_rate_id: String,
}

fn parse_lines(raw: &std::collections::HashMap<String, String>) -> Vec<ParsedLine> {
    // Group fields by the integer in `lines[N][field]`.
    let mut by_index: std::collections::BTreeMap<usize, ParsedLine> =
        std::collections::BTreeMap::new();
    for (key, value) in raw {
        // key is e.g. "lines[0][account_id]"
        if !key.starts_with("lines[") {
            continue;
        }
        let after = &key[6..]; // "0][account_id]"
        let Some(close) = after.find("][") else {
            continue;
        };
        let idx_str = &after[..close];
        let Ok(idx) = idx_str.parse::<usize>() else {
            continue;
        };
        let field = &after[close + 2..];
        let field = field.trim_end_matches(']');
        let entry = by_index.entry(idx).or_default();
        match field {
            "account_id" => entry.account_id = value.clone(),
            "direction" => entry.direction = value.clone(),
            "amount" => entry.amount = value.clone(),
            "memo" => entry.memo = value.clone(),
            "tax_rate_id" => entry.tax_rate_id = value.clone(),
            _ => {}
        }
    }
    by_index.into_values().collect()
}

/// Public re-export so the edit handler can parse the same
/// `lines[N][field]=…` form body.
pub fn parse_lines_for_edit(raw: &std::collections::HashMap<String, String>) -> Vec<ParsedLine> {
    parse_lines(raw)
}

/// Upper bound for the create-request body. The request-body layer
/// already caps uploads (`upload_max_bytes`); this is a defensive
/// ceiling for the in-memory read.
const MAX_CREATE_BODY: usize = 96 * 1024 * 1024;

/// A file extracted from a multipart create request
/// (`a12-transaction-entry-ease`).
pub struct PendingUpload {
    pub filename: String,
    pub declared: String,
    pub bytes: Vec<u8>,
}

/// Parse a `multipart/form-data` create body into form fields plus
/// the uploaded document files.
async fn parse_multipart_create(
    bytes: &[u8],
    content_type: &str,
) -> AppResult<(
    std::collections::HashMap<String, String>,
    Vec<PendingUpload>,
)> {
    let boundary = content_type
        .split("boundary=")
        .nth(1)
        .unwrap_or("")
        .trim_matches('"')
        .to_string();
    if boundary.is_empty() {
        return Err(AppError::Validation("Invalid multipart boundary".into()));
    }
    let mut mp = multer::Multipart::new(
        futures::stream::once(futures::future::ready(Ok::<_, std::convert::Infallible>(
            axum::body::Bytes::copy_from_slice(bytes),
        ))),
        boundary,
    );
    let mut fields: std::collections::HashMap<String, String> = Default::default();
    let mut files: Vec<PendingUpload> = Vec::new();
    while let Some(mut field) = mp
        .next_field()
        .await
        .map_err(|e| AppError::Multipart(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if field.file_name().is_some() {
            let filename = field.file_name().unwrap_or("upload").to_string();
            let declared = field
                .content_type()
                .map(|m| m.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let mut buf: Vec<u8> = Vec::new();
            let mut total = 0usize;
            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|e| AppError::Multipart(e.to_string()))?
            {
                total += chunk.len();
                if total > crate::upload::DEFAULT_MAX_BYTES {
                    return Err(AppError::Validation(format!(
                        "File too large (max {} bytes)",
                        crate::upload::DEFAULT_MAX_BYTES
                    )));
                }
                buf.extend_from_slice(&chunk);
            }
            files.push(PendingUpload {
                filename,
                declared,
                bytes: buf,
            });
        } else {
            let text = field
                .text()
                .await
                .map_err(|e| AppError::Multipart(e.to_string()))?;
            fields.insert(name, text);
        }
    }
    Ok((fields, files))
}

pub async fn create(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    req: axum::extract::Request,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let accounts = sqlx::query_as::<_, Account>(
        r#"SELECT id, ledger_id, parent_id, name, code, type, subtype, currency, is_archived, description, created_at, updated_at
           FROM accounts WHERE ledger_id = $1 AND is_archived = FALSE ORDER BY type, code, name"#,
    )
    .bind(ledger_id)
    .fetch_all(&state.pool)
    .await?;

    // The body is either `application/x-www-form-urlencoded` (no
    // documents) or `multipart/form-data` (inline document attach,
    // `a12-transaction-entry-ease`). Parse both.
    let content_type = req
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = axum::body::to_bytes(req.into_body(), MAX_CREATE_BODY)
        .await
        .map_err(|e| AppError::Internal(format!("read create body: {e}")))?;

    // Parse the raw form body ourselves: `lines[N][field]=value` is not
    // representable as a nested serde struct, so we read the bytes, split on
    // '&', and group by the integer in the key.
    let (raw, uploads) = if content_type.starts_with("multipart/form-data") {
        parse_multipart_create(&bytes, &content_type).await?
    } else {
        (
            form_urlencoded::parse(&bytes)
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect(),
            Vec::new(),
        )
    };
    let form = NewTxnForm {
        date: raw.get("date").cloned().unwrap_or_default(),
        description: raw.get("description").cloned().unwrap_or_default(),
        payee: raw.get("payee").cloned(),
        reference: raw.get("reference").cloned(),
        extra: raw,
    };
    let bind_doc = form.extra.get("bind_doc").cloned().unwrap_or_default();

    // Active tax rates for the per-line picker; also used to expand
    // tax legs below (`a14-tax-on-transactions`).
    let active_tax_rates = load_active_tax_rates(&state.pool, ledger_id).await?;
    let tax_rates: Vec<(Uuid, String, String)> = active_tax_rates
        .iter()
        .map(|t| {
            let pct = (t.rate * Decimal::new(100, 0)).round_dp(0);
            (t.id, format!("{} ({}%)", t.name, pct), t.rate.to_string())
        })
        .collect();

    let make_error = |msg: String| TransactionNew {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name.clone(),
        current_section: "transactions".to_string(),
        currency: ledger.base_currency.clone(),
        account_groups: group_accounts(accounts.clone()),
        error: msg,
        bind_doc: bind_doc.clone(),
        form: TransactionForm {
            date: form.date.clone(),
            description: form.description.clone(),
            payee: form.payee.clone().unwrap_or_default(),
            reference: form.reference.clone().unwrap_or_default(),
            number: form.extra.get("number").cloned().unwrap_or_default(),
            lines: parse_lines(&form.extra)
                .into_iter()
                .map(|l| TransactionFormLine {
                    account_id: l.account_id,
                    amount: l.amount,
                    direction: l.direction,
                    memo: l.memo,
                    tax_rate_id: l.tax_rate_id,
                })
                .collect(),
        },
        tax_rates: tax_rates.clone(),
    };

    // `a8-draft-transactions`: "Save as draft" submits with
    // action=draft; everything else (the default submit, JS
    // submit, etc.) creates a posted transaction.
    let save_as_draft = matches!(form.extra.get("action").map(String::as_str), Some("draft"));

    if form.extra.is_empty() {
        return Ok(render_response(make_error(
            "A transaction needs at least two postings.".into(),
        )));
    }
    let parsed = parse_lines(&form.extra);
    if parsed.len() < 2 {
        return Ok(render_response(make_error(
            "A transaction needs at least two postings.".into(),
        )));
    }
    let date = match NaiveDate::parse_from_str(&form.date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            return Ok(render_response(make_error(
                "Invalid date format (YYYY-MM-DD)".into(),
            )))
        }
    };
    let description = form.description.trim();
    if description.is_empty() {
        return Ok(render_response(make_error(
            "Description is required".into(),
        )));
    }

    // Optional user-supplied transaction number. The service
    // resolves to an auto-generated `YYYY-NNNNNN` when this
    // is absent.
    let mut inputs: Vec<TxnLineInput> = Vec::new();
    for l in &parsed {
        let account_id = match Uuid::parse_str(&l.account_id) {
            Ok(id) => id,
            Err(_) => {
                return Ok(render_response(make_error(
                    "Invalid account on a posting".into(),
                )))
            }
        };
        let amount: Decimal = match l.amount.trim().parse() {
            Ok(a) if a > Decimal::ZERO => a,
            _ => {
                return Ok(render_response(make_error(
                    "Each posting must have a positive amount".into(),
                )))
            }
        };
        let direction = match l.direction.as_str() {
            "DEBIT" => Direction::Debit,
            "CREDIT" => Direction::Credit,
            _ => {
                return Ok(render_response(make_error(
                    "Each posting must be DEBIT or CREDIT".into(),
                )))
            }
        };
        let tax_rate_id = if l.tax_rate_id.trim().is_empty() {
            None
        } else {
            match Uuid::parse_str(l.tax_rate_id.trim()) {
                Ok(id) => Some(id),
                Err(_) => {
                    return Ok(render_response(make_error(
                        "Invalid tax rate on a posting".into(),
                    )))
                }
            }
        };
        inputs.push(TxnLineInput {
            account_id,
            signed_amount: match direction {
                Direction::Debit => amount,
                Direction::Credit => -amount,
            },
            memo: if l.memo.is_empty() {
                None
            } else {
                Some(l.memo.clone())
            },
            tax_rate_id,
            foreign: None,
        });
    }

    // `ux-transaction-entry`: refuse an entry that moves money
    // within a single account (same account on both sides) —
    // balanced but economically meaningless. Runs on the user's
    // lines only, before tax legs are appended.
    {
        use std::collections::HashMap;
        let mut by_account: HashMap<Uuid, (bool, bool)> = HashMap::new();
        for i in &inputs {
            let e = by_account.entry(i.account_id).or_insert((false, false));
            if i.signed_amount.is_sign_positive() {
                e.0 = true;
            } else if i.signed_amount.is_sign_negative() {
                e.1 = true;
            }
        }
        if let Some((account_id, _)) = by_account.iter().find(|(_, (d, c))| *d && *c) {
            let name = accounts
                .iter()
                .find(|a| a.id == *account_id)
                .map(|a| a.name.as_str())
                .unwrap_or("this account");
            return Ok(render_response(make_error(format!(
                "This entry moves money within \"{name}\" (the same account on both sides) — pick a different account for one of the lines."
            ))));
        }
    }

    // `a14-tax-on-transactions`: expand each taxed line into a tax leg
    // on the same side, and record the linkage for `posting_taxes`.
    let mut tax_links: Vec<crate::domain::posting_service::TaxLink> = Vec::new();
    {
        let taxed: Vec<(usize, Uuid)> = inputs
            .iter()
            .enumerate()
            .filter_map(|(i, l)| l.tax_rate_id.map(|id| (i, id)))
            .collect();
        for (i, rate_id) in taxed {
            let rate = match active_tax_rates.iter().find(|r| r.id == rate_id) {
                Some(r) => r,
                None => {
                    return Ok(render_response(make_error(
                        "Unknown or inactive tax rate selected.".into(),
                    )))
                }
            };
            let base = inputs[i].signed_amount.abs();
            let tax_amount = (base * rate.rate).round_dp(2);
            let tax_signed = if inputs[i].signed_amount >= Decimal::ZERO {
                tax_amount
            } else {
                -tax_amount
            };
            inputs.push(TxnLineInput {
                account_id: rate.account_id,
                signed_amount: tax_signed,
                memo: Some(format!("{} tax", rate.name)),
                tax_rate_id: None,
                foreign: None,
            });
            tax_links.push(crate::domain::posting_service::TaxLink {
                posting_idx: i,
                tax_rate_id: rate_id,
                base_amount: base,
                tax_amount,
            });
        }
    }

    // Validate balance in code before insert (DB trigger will also enforce).
    // This runs after tax legs are appended, so the entry must balance
    // including tax.
    let total: Decimal = inputs.iter().map(|i| i.signed_amount).sum();
    if total != Decimal::ZERO {
        return Ok(render_response(make_error(format!(
            "Postings do not balance: net is {} (debits must equal credits).",
            total
        ))));
    }

    // Route the actual write through `PostingService`
    // (`a3-posting-service`). The service handles the
    // closed-period check, balance validation, atomic
    // insertion, ledger FOR UPDATE lock, and audit log.
    let new_txn = crate::domain::posting_service::NewTransaction {
        ledger_id,
        txn_date: date,
        description: description.to_string(),
        payee: form
            .payee
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        reference: form
            .reference
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        kind: Some("standard".to_string()),
        created_by: user.id,
        lines: inputs.clone(),
        reverses_id: None,
        number: form.extra.get("number").cloned().filter(|s| !s.is_empty()),
        tax_links,
    };
    let service_call = if save_as_draft {
        crate::domain::posting_service::PostingService::create_draft(&state.pool, new_txn).await
    } else {
        crate::domain::posting_service::PostingService::create(&state.pool, new_txn).await
    };
    let created = match service_call {
        Ok(c) => c,
        Err(crate::domain::posting_service::PostingServiceError::Unbalanced {
            debits,
            credits,
        }) => {
            return Ok(render_response(make_error(format!(
                "Postings do not balance (debits={debits}, credits={credits})"
            ))));
        }
        Err(crate::domain::posting_service::PostingServiceError::PeriodClosed { year, .. }) => {
            return Err(AppError::Validation(format!(
                "Period {year} is closed. Cannot post transactions to closed periods."
            )));
        }
        Err(crate::domain::posting_service::PostingServiceError::LedgerNotFound) => {
            return Err(AppError::NotFound);
        }
        Err(crate::domain::posting_service::PostingServiceError::UnknownAccount(id)) => {
            return Err(AppError::Validation(format!("unknown account {id}")));
        }
        Err(crate::domain::posting_service::PostingServiceError::WrongLedger(id)) => {
            return Err(AppError::Validation(format!(
                "account belongs to a different ledger"
            )));
        }
        Err(crate::domain::posting_service::PostingServiceError::DuplicateNumber {
            year,
            number,
        }) => {
            return Err(AppError::Conflict(format!(
                "Transaction number already used in {year}: {number}"
            )));
        }
        Err(crate::domain::posting_service::PostingServiceError::MissingFxRate(msg)) => {
            return Ok(render_response(make_error(msg)));
        }
        Err(crate::domain::posting_service::PostingServiceError::Db(e)) => {
            return Err(AppError::Db(e));
        }
    };

    // Domain metric (`o4-metrics-endpoint`).
    crate::observability::metrics::postings_created(inputs.len() as u64);

    // Inline documents (`a12-transaction-entry-ease`): attach the
    // uploaded files to the freshly created transaction. A failure
    // here must not lose the transaction — the entry is already
    // saved; only the attachment is abandoned.
    for up in &uploads {
        if let Err(e) = crate::handlers::documents::save_inline_document(
            &state,
            ledger_id,
            created.id,
            user,
            &up.filename,
            &up.declared,
            &up.bytes,
        )
        .await
        {
            tracing::warn!(txn_id = %created.id, error = %e, "inline document attach failed");
        }
    }

    // `a13-document-inbox`: when the form was opened from the bind
    // page (?bind_doc=<id>), attach that unbound document to the
    // newly created transaction.
    if let Ok(doc_id) = bind_doc.parse::<Uuid>() {
        if let Err(e) =
            crate::handlers::documents::bind_document(&state, ledger_id, doc_id, created.id, user)
                .await
        {
            tracing::warn!(txn_id = %created.id, doc_id = %doc_id, error = %e, "bind-from-create failed");
        }
    }

    let target = if save_as_draft {
        format!("/ledgers/{}/drafts", ledger_id)
    } else {
        format!("/ledgers/{}/transactions/{}", ledger_id, created.id)
    };
    Ok(Redirect::to(&target).into_response())
}

pub async fn show(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, txn_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let ledger = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let txn = sqlx::query_as::<_, Transaction>(
        r#"SELECT id, ledger_id, txn_date, description, payee, reference, currency, kind, contact_id, invoice_id, template_id, created_by, number, created_at, updated_at
           FROM transactions WHERE id = $1 AND ledger_id = $2"#,
    )
    .bind(txn_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let lines = sqlx::query_as::<_, (String, String, String, Decimal, Option<String>)>(
        r#"SELECT a.name, a.type, p.direction, p.amount, p.memo
           FROM postings p JOIN accounts a ON a.id = p.account_id
           WHERE p.transaction_id = $1
           ORDER BY p.direction, a.name"#,
    )
    .bind(txn_id)
    .fetch_all(&state.pool)
    .await?;
    let lines: Vec<TransactionShowLine> = lines
        .into_iter()
        .map(|(n, t, d, a, m)| TransactionShowLine {
            account_name: n,
            account_type: t,
            direction: d,
            amount: a,
            memo: m.unwrap_or_default(),
        })
        .collect();

    let documents = sqlx::query_as::<_, crate::domain::Document>(
        r#"SELECT id, transaction_id, ledger_id, filename, stored_filename, mime_type, size_bytes, uploaded_by, uploaded_at, category
           FROM documents WHERE transaction_id = $1 ORDER BY uploaded_at"#,
    )
    .bind(txn_id)
    .fetch_all(&state.pool)
    .await?;

    let tags: Vec<String> = sqlx::query_scalar(
        r#"SELECT tg.name FROM transaction_tags tt
           JOIN tags tg ON tg.id = tt.tag_id WHERE tt.transaction_id = $1 ORDER BY tg.name"#,
    )
    .bind(txn_id)
    .fetch_all(&state.pool)
    .await?;

    let template_description: String = if let Some(tid) = txn.template_id {
        sqlx::query_scalar("SELECT description FROM transaction_templates WHERE id = $1")
            .bind(tid)
            .fetch_optional(&state.pool)
            .await?
            .unwrap_or_default()
    } else {
        String::new()
    };

    Ok(render_response(TransactionShow {
        user_id: user.id,
        username: user.username.clone(),
        user_role: user.role.clone(),
        ledger_id,
        ledger_name: ledger.name,
        current_section: "transactions".to_string(),
        txn_id: txn.id,
        date: txn.txn_date,
        description: txn.description,
        payee: txn.payee.unwrap_or_default(),
        reference: txn.reference.unwrap_or_default(),
        currency: txn.currency,
        lines,
        documents,
        tags,
        flash: String::new(),
        template_id: txn.template_id,
        template_description,
    }))
}
