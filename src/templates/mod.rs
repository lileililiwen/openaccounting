pub mod account;
pub mod accounts;
pub mod admin;
pub mod audit;
pub mod auth;
pub mod common;
pub mod contacts;
pub mod dashboard;
pub mod documents;
pub mod error;
pub mod import;
pub mod into_response;
pub mod invoices;
pub mod ledgers;
pub mod reports;
pub mod sharing;
pub mod templates;
pub mod transactions;

pub use into_response::render_response;

use askama::Template;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexPage;

pub fn fmt_money(d: &Decimal, currency: &str) -> String {
    let s = format!("{:.2}", d);
    format!("{} {}", currency, s)
}

pub fn fmt_date(d: &NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub fn fmt_datetime(d: &DateTime<Utc>) -> String {
    d.format("%Y-%m-%d %H:%M").to_string()
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", cut)
    }
}

pub fn account_type_class(t: &str) -> &'static str {
    match t {
        "ASSET" => "text-emerald-700",
        "LIABILITY" => "text-rose-700",
        "EQUITY" => "text-indigo-700",
        "INCOME" => "text-sky-700",
        "EXPENSE" => "text-amber-700",
        _ => "text-slate-700",
    }
}
