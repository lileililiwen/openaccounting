//! Archivable PDF download + machine export handlers
//! (`compliance-exports`).
//!
//! Routes:
//! - `GET /ledgers/{id}/reports/{kind}.pdf` — PDF for the five
//!   core reports. Owner / accountant / auditor / editor with
//!   period access.
//! - `GET /ledgers/{id}/invoices/{invoice_id}.pdf` — invoice PDF.
//! - `GET /ledgers/{id}/export/{format}?from=…&to=…` — SAF-T
//!   lite, XBRL-GL, DATEV machine exports.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Form;
use axum_login::AuthSession;
use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::{
    auth::Backend,
    export::{datev, pdf, saf_t, xbrl},
    handlers::ledgers,
    AppState,
};

/// Query parameters for period-bounded machine exports.
#[derive(Debug, Deserialize, Default)]
pub struct PeriodQuery {
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

fn parse_period(q: &PeriodQuery) -> AppResult<(NaiveDate, NaiveDate)> {
    let from: NaiveDate = q
        .from
        .as_deref()
        .unwrap_or("1900-01-01")
        .parse()
        .map_err(|_| AppError::Validation("from must be YYYY-MM-DD".into()))?;
    let to: NaiveDate =
        q.to.as_deref()
            .unwrap_or("2999-12-31")
            .parse()
            .map_err(|_| AppError::Validation("to must be YYYY-MM-DD".into()))?;
    Ok((from, to))
}

/// `GET /ledgers/{id}/reports/trial-balance.pdf?from=…&to=…`
pub async fn report_pdf_trial(
    auth: AuthSession<Backend>,
    state: State<AppState>,
    Path(ledger_id): Path<Uuid>,
    q: axum::extract::Query<PeriodQuery>,
) -> AppResult<Response> {
    report_pdf_inner(auth, state, ledger_id, "trial-balance", q.0).await
}

pub async fn report_pdf_balance_sheet(
    auth: AuthSession<Backend>,
    state: State<AppState>,
    Path(ledger_id): Path<Uuid>,
    q: axum::extract::Query<PeriodQuery>,
) -> AppResult<Response> {
    report_pdf_inner(auth, state, ledger_id, "balance-sheet", q.0).await
}

pub async fn report_pdf_income_statement(
    auth: AuthSession<Backend>,
    state: State<AppState>,
    Path(ledger_id): Path<Uuid>,
    q: axum::extract::Query<PeriodQuery>,
) -> AppResult<Response> {
    report_pdf_inner(auth, state, ledger_id, "income-statement", q.0).await
}

pub async fn report_pdf_cash_flow(
    auth: AuthSession<Backend>,
    state: State<AppState>,
    Path(ledger_id): Path<Uuid>,
    q: axum::extract::Query<PeriodQuery>,
) -> AppResult<Response> {
    report_pdf_inner(auth, state, ledger_id, "cash-flow", q.0).await
}

pub async fn report_pdf_general_ledger(
    auth: AuthSession<Backend>,
    state: State<AppState>,
    Path(ledger_id): Path<Uuid>,
    q: axum::extract::Query<PeriodQuery>,
) -> AppResult<Response> {
    report_pdf_inner(auth, state, ledger_id, "general-ledger", q.0).await
}

async fn report_pdf_inner(
    auth: AuthSession<Backend>,
    state: State<AppState>,
    ledger_id: Uuid,
    kind: &str,
    q: PeriodQuery,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let (ledger, _role) = ledgers::ensure_access(&state, user.id, ledger_id).await?;
    let (from, to) = parse_period(&q)?;
    let (table, period_label, title) =
        build_report_table(&state, ledger_id, kind, from, to).await?;
    let notes = load_notes(&state, ledger_id, period_label_for_export(kind, from, to)).await;
    let meta = pdf::PdfMeta::from_env(
        format!("{title} — {}", ledger.name),
        ledger.name.clone(),
        period_label,
    );
    let bytes = pdf::render_table(&meta, &table, notes.as_deref())
        .map_err(|e| AppError::Internal(format!("pdf render: {e}")))?;
    Ok(pdf_response(bytes, &format!("{kind}.pdf")))
}

/// `GET /ledgers/{id}/invoices/{invoice_id}.pdf`
pub async fn invoice_pdf(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, invoice_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_access(&state, user.id, ledger_id).await?;
    let row: Option<(
        Option<String>,
        String,
        chrono::NaiveDate,
        chrono::NaiveDate,
        rust_decimal::Decimal,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT i.invoice_number, i.status, i.invoice_date, i.due_date, i.total, i.kind,
                    i.doc_kind, c.name
             FROM invoices i
             LEFT JOIN contacts c ON c.id = i.contact_id
             WHERE i.id = $1 AND i.ledger_id = $2",
    )
    .bind(invoice_id)
    .bind(ledger_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::Internal(format!("db: {e}")))?;
    let Some((number, status, invoice_date, due_date, total, kind, doc_kind, contact)) = row else {
        return Err(AppError::NotFound);
    };
    let contact = contact.unwrap_or_default();
    let number = number.unwrap_or_else(|| "(no number)".into());
    let table = pdf::PdfTable {
        columns: vec!["Field".into(), "Value".into()],
        rows: vec![
            vec![
                pdf::PdfCell {
                    text: "Invoice".into(),
                    href: None,
                },
                pdf::PdfCell {
                    text: number.clone(),
                    href: None,
                },
            ],
            vec![
                pdf::PdfCell {
                    text: "Status".into(),
                    href: None,
                },
                pdf::PdfCell {
                    text: status,
                    href: None,
                },
            ],
            vec![
                pdf::PdfCell {
                    text: "Date".into(),
                    href: None,
                },
                pdf::PdfCell {
                    text: invoice_date.to_string(),
                    href: None,
                },
            ],
            vec![
                pdf::PdfCell {
                    text: "Due".into(),
                    href: None,
                },
                pdf::PdfCell {
                    text: due_date.to_string(),
                    href: None,
                },
            ],
            vec![
                pdf::PdfCell {
                    text: "Kind".into(),
                    href: None,
                },
                pdf::PdfCell {
                    text: kind,
                    href: None,
                },
            ],
            vec![
                pdf::PdfCell {
                    text: "Doc kind".into(),
                    href: None,
                },
                pdf::PdfCell {
                    text: doc_kind.unwrap_or_default(),
                    href: None,
                },
            ],
            vec![
                pdf::PdfCell {
                    text: "Contact".into(),
                    href: None,
                },
                pdf::PdfCell {
                    text: contact,
                    href: None,
                },
            ],
            vec![
                pdf::PdfCell {
                    text: "Total".into(),
                    href: None,
                },
                pdf::PdfCell {
                    text: format!("{:.2}", total),
                    href: None,
                },
            ],
        ],
    };
    let meta = pdf::PdfMeta::from_env(format!("Invoice {number}"), "", invoice_date.to_string());
    let bytes = pdf::render_table(&meta, &table, None)
        .map_err(|e| AppError::Internal(format!("pdf render: {e}")))?;
    Ok(pdf_response(bytes, &format!("invoice-{number}.pdf")))
}

/// `GET /ledgers/{id}/export/{format}?from=…&to=…`
pub async fn export_machine(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path((ledger_id, format)): Path<(Uuid, String)>,
    Query(q): Query<PeriodQuery>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    let snap = crate::export::LedgerSnapshot::load(&state.pool, ledger_id)
        .await
        .map_err(|e| AppError::Internal(format!("snapshot: {e}")))?;
    let (from, to) = parse_period(&q)?;
    match format.as_str() {
        "saf-t" => Ok(text_response(
            saf_t::render(&snap, from, to),
            "application/xml; charset=utf-8",
            "ledger.saf-t.xml",
        )),
        "xbrl-gl" => Ok(text_response(
            xbrl::render(&snap, from, to),
            "application/xml; charset=utf-8",
            "ledger.xbrl.xml",
        )),
        "datev" => Ok(text_response(
            datev::render(&snap, from, to),
            "text/csv; charset=utf-8",
            "ledger.datev.csv",
        )),
        _ => Err(AppError::Validation(format!(
            "unsupported format '{format}' (use saf-t, xbrl-gl, datev)"
        ))),
    }
}

fn pdf_response(bytes: Vec<u8>, filename: &str) -> Response {
    let mut resp = (StatusCode::OK, bytes).into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    resp.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .unwrap_or(HeaderValue::from_static("attachment")),
    );
    resp.headers_mut().insert(
        "X-OA-Report-Version",
        HeaderValue::from_static(env!("CARGO_PKG_VERSION")),
    );
    resp
}

fn text_response(body: String, content_type: &'static str, filename: &'static str) -> Response {
    let mut resp = (StatusCode::OK, body).into_response();
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    resp.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .unwrap_or(HeaderValue::from_static("attachment")),
    );
    resp
}

fn period_label_for_export(kind: &str, from: NaiveDate, to: NaiveDate) -> String {
    format!("{kind}:{from}..{to}")
}

/// Build a flat PDF table for one of the five supported reports.
/// Returns (table, period_label, title). Comparative column is
/// computed by re-running the query with prior-period dates (for
/// income statement and balance sheet); other reports are point-in-time
/// or already period-bounded.
async fn build_report_table(
    state: &AppState,
    ledger_id: Uuid,
    kind: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> AppResult<(pdf::PdfTable, String, String)> {
    let title = match kind {
        "trial-balance" => "Trial Balance",
        "balance-sheet" => "Balance Sheet",
        "income-statement" => "Income Statement",
        "cash-flow" => "Cash Flow",
        "general-ledger" => "General Ledger",
        _ => {
            return Err(AppError::Validation(format!(
                "unsupported report '{kind}' (use trial-balance, balance-sheet, income-statement, cash-flow, general-ledger)"
            )));
        }
    };
    let period_label = format!("{from} → {to}");
    let mut columns = vec!["Account".into()];
    if matches!(kind, "income-statement" | "balance-sheet") {
        columns.push("Current".into());
        columns.push("Prior".into());
        columns.push("Δ".into());
        columns.push("Drill-down".into());
    } else {
        columns.push("Amount".into());
    }
    let mut rows: Vec<Vec<pdf::PdfCell>> = Vec::new();

    match kind {
        "trial-balance" => {
            let rows_data: Vec<(Uuid, String, rust_decimal::Decimal, rust_decimal::Decimal)> =
                sqlx::query_as(
                    "SELECT a.id, a.name,
                            COALESCE(SUM(CASE WHEN p.direction = 'DEBIT' THEN p.amount ELSE 0 END), 0),
                            COALESCE(SUM(CASE WHEN p.direction = 'CREDIT' THEN p.amount ELSE 0 END), 0)
                     FROM accounts a
                     LEFT JOIN postings p ON p.account_id = a.id
                     LEFT JOIN transactions t ON t.id = p.transaction_id
                        AND t.kind NOT IN ('draft','pending')
                        AND t.txn_date <= $2
                     WHERE a.ledger_id = $1 AND a.is_archived = FALSE
                     GROUP BY a.id, a.name
                     HAVING COALESCE(SUM(p.amount), 0) <> 0
                     ORDER BY a.name",
                )
                .bind(ledger_id)
                .bind(to)
                .fetch_all(&state.pool)
                .await
                .map_err(|e| AppError::Internal(format!("db: {e}")))?;
            for (id, name, debit, credit) in rows_data {
                let net = debit - credit;
                let href = format!("/ledgers/{ledger_id}/reports/general-ledger?account_id={id}");
                rows.push(vec![
                    pdf::PdfCell {
                        text: name,
                        href: Some(href),
                    },
                    pdf::PdfCell {
                        text: format!("{:.2}", net),
                        href: None,
                    },
                ]);
            }
        }
        "balance-sheet" | "income-statement" => {
            let period_len = (to - from).num_days();
            let prior_from = from - chrono::Duration::days(period_len + 1);
            let prior_to = from - chrono::Duration::days(1);
            let rows_data: Vec<(Uuid, String, rust_decimal::Decimal)> = sqlx::query_as(
                "SELECT a.id, a.name,
                        COALESCE(SUM(CASE WHEN p.direction = 'DEBIT' THEN p.amount ELSE -p.amount END), 0) AS net
                 FROM accounts a
                 LEFT JOIN postings p ON p.account_id = a.id
                 LEFT JOIN transactions t ON t.id = p.transaction_id
                    AND t.kind NOT IN ('draft','pending')
                    AND t.txn_date BETWEEN $2 AND $3
                 WHERE a.ledger_id = $1 AND a.is_archived = FALSE
                 GROUP BY a.id, a.name
                 ORDER BY a.name",
            )
            .bind(ledger_id)
            .bind(from)
            .bind(to)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::Internal(format!("db: {e}")))?;
            let prior_rows: Vec<(Uuid, rust_decimal::Decimal)> = sqlx::query_as(
                "SELECT a.id,
                        COALESCE(SUM(CASE WHEN p.direction = 'DEBIT' THEN p.amount ELSE -p.amount END), 0) AS net
                 FROM accounts a
                 LEFT JOIN postings p ON p.account_id = a.id
                 LEFT JOIN transactions t ON t.id = p.transaction_id
                    AND t.kind NOT IN ('draft','pending')
                    AND t.txn_date BETWEEN $2 AND $3
                 WHERE a.ledger_id = $1 AND a.is_archived = FALSE
                 GROUP BY a.id",
            )
            .bind(ledger_id)
            .bind(prior_from)
            .bind(prior_to)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::Internal(format!("db: {e}")))?;
            let prior_map: std::collections::HashMap<Uuid, rust_decimal::Decimal> =
                prior_rows.into_iter().collect();
            for (id, name, current) in rows_data {
                let prior = prior_map.get(&id).copied().unwrap_or_default();
                let delta = current - prior;
                let href = format!(
                    "/ledgers/{ledger_id}/reports/general-ledger?account_id={id}&from={from}&to={to}"
                );
                rows.push(vec![
                    pdf::PdfCell {
                        text: name,
                        href: None,
                    },
                    pdf::PdfCell {
                        text: format!("{:.2}", current),
                        href: None,
                    },
                    pdf::PdfCell {
                        text: format!("{:.2}", prior),
                        href: None,
                    },
                    pdf::PdfCell {
                        text: format!("{:+.2}", delta),
                        href: None,
                    },
                    pdf::PdfCell {
                        text: "→ GL".into(),
                        href: Some(href),
                    },
                ]);
            }
        }
        "cash-flow" => {
            let rows_data: Vec<(String, rust_decimal::Decimal)> = sqlx::query_as(
                "SELECT a.name,
                        COALESCE(SUM(CASE WHEN p.direction = 'DEBIT' THEN p.amount ELSE -p.amount END), 0)
                 FROM accounts a
                 JOIN postings p ON p.account_id = a.id
                 JOIN transactions t ON t.id = p.transaction_id
                 WHERE a.ledger_id = $1
                   AND a.type IN ('ASSET','LIABILITY','INCOME','EXPENSE')
                   AND t.kind NOT IN ('draft','pending')
                   AND t.txn_date BETWEEN $2 AND $3
                 GROUP BY a.name
                 ORDER BY a.name",
            )
            .bind(ledger_id)
            .bind(from)
            .bind(to)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::Internal(format!("db: {e}")))?;
            for (name, amt) in rows_data {
                rows.push(vec![
                    pdf::PdfCell {
                        text: name,
                        href: None,
                    },
                    pdf::PdfCell {
                        text: format!("{:.2}", amt),
                        href: None,
                    },
                ]);
            }
        }
        "general-ledger" => {
            let rows_data: Vec<(
                Uuid,
                chrono::NaiveDate,
                String,
                String,
                rust_decimal::Decimal,
            )> = sqlx::query_as(
                "SELECT t.id, t.txn_date, t.description, p.direction, p.amount
                     FROM transactions t
                     JOIN postings p ON p.transaction_id = t.id
                     WHERE t.ledger_id = $1
                       AND t.kind NOT IN ('draft','pending')
                       AND t.txn_date BETWEEN $2 AND $3
                     ORDER BY t.txn_date, t.id
                     LIMIT 200",
            )
            .bind(ledger_id)
            .bind(from)
            .bind(to)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::Internal(format!("db: {e}")))?;
            for (txn_id, date, desc, dir, amount) in rows_data {
                let href = format!("/ledgers/{ledger_id}/transactions/{txn_id}");
                rows.push(vec![
                    pdf::PdfCell {
                        text: date.to_string(),
                        href: Some(href.clone()),
                    },
                    pdf::PdfCell {
                        text: format!("{dir} {desc}"),
                        href: None,
                    },
                    pdf::PdfCell {
                        text: format!("{:.2}", amount),
                        href: None,
                    },
                ]);
            }
            columns = vec!["Date".into(), "Description".into(), "Amount".into()];
        }
        _ => unreachable!(),
    }

    Ok((
        pdf::PdfTable { columns, rows },
        period_label,
        title.to_string(),
    ))
}

async fn load_notes(state: &AppState, ledger_id: Uuid, period_key: String) -> Option<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT body FROM period_notes WHERE ledger_id = $1 AND period_key = $2",
    )
    .bind(ledger_id)
    .bind(period_key)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten()
}

// ── Report notes ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct NotesForm {
    pub period_key: String,
    pub body: String,
}

pub async fn notes_save(
    auth: AuthSession<Backend>,
    State(state): State<AppState>,
    Path(ledger_id): Path<Uuid>,
    Form(form): Form<NotesForm>,
) -> AppResult<Response> {
    let user = auth.user.as_ref().ok_or(AppError::Unauthorized)?;
    let _ = ledgers::ensure_writer(&state, user.id, ledger_id).await?;
    sqlx::query(
        "INSERT INTO period_notes (ledger_id, period_key, body, created_by, created_at, updated_at)
         VALUES ($1, $2, $3, $4, now(), now())
         ON CONFLICT (ledger_id, period_key) DO UPDATE
            SET body = EXCLUDED.body, updated_at = now()",
    )
    .bind(ledger_id)
    .bind(&form.period_key)
    .bind(&form.body)
    .bind(user.id)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::Internal(format!("db: {e}")))?;
    Ok((
        StatusCode::SEE_OTHER,
        [(
            header::LOCATION,
            HeaderValue::from_str(&format!("/ledgers/{ledger_id}/reports/income-statement"))
                .unwrap_or(HeaderValue::from_static("/")),
        )],
    )
        .into_response())
}

pub async fn notes_load(
    state: &AppState,
    ledger_id: Uuid,
    period_key: &str,
) -> Option<(String, chrono::DateTime<chrono::Utc>, Uuid)> {
    sqlx::query_as::<_, (String, chrono::DateTime<chrono::Utc>, Uuid)>(
        "SELECT body, created_at, created_by FROM period_notes
         WHERE ledger_id = $1 AND period_key = $2",
    )
    .bind(ledger_id)
    .bind(period_key)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten()
}
