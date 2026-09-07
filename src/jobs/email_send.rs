//! Email notification channel (`automation-platform`).
//!
//! SMTP is configured through the environment:
//!   - `SMTP_URL`  — `smtp://user:pass@host:587` (STARTTLS),
//!                   `smtps://user:pass@host:465` (implicit TLS), or
//!                   `smtp://host:25` (plain, for local relays)
//!   - `SMTP_FROM` — `OpenAccounting <no-reply@example.com>`
//! When unset, the channel reports "not configured" and callers skip
//! it without failing.

use lettre::message::{Mailbox, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use super::JobError;

/// Whether an SMTP relay is configured.
pub fn configured() -> bool {
    std::env::var("SMTP_URL").is_ok_and(|v| !v.trim().is_empty())
}

struct SmtpConfig {
    scheme: String,
    host: String,
    port: u16,
    user: Option<String>,
    pass: Option<String>,
}

fn config_from_env() -> Result<SmtpConfig, JobError> {
    let raw = std::env::var("SMTP_URL")
        .map_err(|_| JobError::Failed("SMTP_URL not set".into()))?
        .trim()
        .to_string();
    // smtp[s]://[user[:pass]@]host[:port]
    let (scheme, rest) = raw
        .split_once("://")
        .ok_or_else(|| JobError::Failed("SMTP_URL missing '://'".into()))?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "smtp" && scheme != "smtps" {
        return Err(JobError::Failed(format!(
            "unsupported SMTP_URL scheme '{scheme}'"
        )));
    }
    let (userinfo, hostport) = match rest.rsplit_once('@') {
        Some((u, h)) => (Some(u), h),
        None => (None, rest),
    };
    let (user, pass) = match userinfo {
        Some(ui) => match ui.split_once(':') {
            Some((u, p)) => (
                percent_decode(u).filter(|s| !s.is_empty()),
                percent_decode(p),
            ),
            None => (percent_decode(ui), None),
        },
        None => (None, None),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse::<u16>()
                .map_err(|_| JobError::Failed(format!("bad port '{p}'")))?,
        ),
        None => (
            hostport.to_string(),
            if scheme == "smtps" { 465 } else { 587 },
        ),
    };
    if host.is_empty() {
        return Err(JobError::Failed("SMTP_URL missing host".into()));
    }
    Ok(SmtpConfig {
        scheme,
        host,
        port,
        user,
        pass,
    })
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() + 1 && i + 2 < bytes.len() + 1 {
            let hex = bytes.get(i + 1..i + 3)?;
            let hi = (hex[0] as char).to_digit(16)?;
            let lo = (hex[1] as char).to_digit(16)?;
            out.push((hi * 16 + lo) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn build_transport(cfg: &SmtpConfig) -> Result<AsyncSmtpTransport<Tokio1Executor>, JobError> {
    let builder = match (cfg.scheme.as_str(), cfg.port) {
        ("smtps", _) => AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host)
            .map_err(|e| JobError::Failed(e.to_string()))?,
        ("smtp", 25) => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host),
        _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)
            .map_err(|e| JobError::Failed(e.to_string()))?,
    };
    let builder = match (&cfg.user, &cfg.pass) {
        (Some(u), Some(p)) => builder.credentials(Credentials::new(u.clone(), p.clone())),
        _ => builder,
    };
    Ok(builder.port(cfg.port).build())
}

/// Send one email via the `email_send` job payload:
/// `{ "to": "...", "subject": "...", "html": "...", "text": "..." }`.
pub async fn send(_pool: &sqlx::PgPool, payload: &serde_json::Value) -> Result<(), JobError> {
    let to: String = super::payload_field(payload, "to")?;
    let subject: String = super::payload_field(payload, "subject")?;
    let html: String = payload
        .get("html")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let text: String = payload
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    send_now(&to, &subject, &text, &html).await
}

/// Send immediately.
pub async fn send_now(to: &str, subject: &str, text: &str, html: &str) -> Result<(), JobError> {
    let from_raw =
        std::env::var("SMTP_FROM").unwrap_or_else(|_| "OpenAccounting <no-reply@localhost>".into());
    let from: Mailbox = from_raw
        .parse()
        .map_err(|e| JobError::Failed(format!("bad SMTP_FROM: {e}")))?;
    let to_mbox: Mailbox = to
        .trim()
        .parse()
        .map_err(|e| JobError::Failed(format!("bad recipient '{to}': {e}")))?;

    let email = Message::builder()
        .from(from)
        .to(to_mbox)
        .subject(subject)
        .singlepart(if html.is_empty() {
            SinglePart::plain(text.to_string())
        } else {
            SinglePart::html(html.to_string())
        })
        .map_err(|e| JobError::Failed(e.to_string()))?;

    let cfg = config_from_env()?;
    let mailer = build_transport(&cfg)?;
    mailer
        .send(email)
        .await
        .map_err(|e| JobError::Failed(e.to_string()))?;
    Ok(())
}
