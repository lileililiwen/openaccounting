//! Bank-statement import: format-agnostic entry point that
//! dispatches to the OFX / QIF / MT940 parsers and re-exports
//! the row type from the existing CSV importer.

pub mod alipay_mobile;
pub mod alipay_web;
pub mod beancount;
pub mod csv;
pub mod dedup;
pub mod encoding;
pub mod hledger;
pub mod mt940;
pub mod ofx;
pub mod pta;
pub mod qif;
pub mod sniff;
pub mod wechat;

pub use mt940::Mt940;
pub use pta::{ImportReport, PtaFormat, PtaPosting, PtaTxn};
pub use sniff::Format;

/// The platform a batch of parsed rows came from. Drives the
/// audit action and the preview page the upload handler renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
pub enum ImportPlatform {
    #[default]
    Csv,
    Ofx,
    Qif,
    Mt940,
    Wechat,
    AlipayMobile,
    AlipayWeb,
}

impl ImportPlatform {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImportPlatform::Csv => "csv",
            ImportPlatform::Ofx => "ofx",
            ImportPlatform::Qif => "qif",
            ImportPlatform::Mt940 => "mt940",
            ImportPlatform::Wechat => "wechat",
            ImportPlatform::AlipayMobile => "alipay_mobile",
            ImportPlatform::AlipayWeb => "alipay_web",
        }
    }
}

/// A parse failure. `UnsupportedEncoding` carries the byte
/// offset of the offending sequence; the `Csv` variant wraps
/// malformed-row errors from a specific line.
#[derive(Debug)]
pub enum ParseError {
    /// The byte buffer is neither valid UTF-8 nor valid
    /// GB18030 (the superset of GBK / GB2312).
    UnsupportedEncoding,
    /// The canonical header row was not found before the file
    /// ended.
    MissingHeader(String),
    /// A value could not be parsed as a positive decimal
    /// amount. Carries the raw cell text.
    BadAmount(String),
    /// A timestamp could not be parsed as a date. Carries the
    /// raw cell text.
    BadDate(String),
    /// Malformed CSV row. Carries the `csv::Error` message.
    Csv(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnsupportedEncoding => {
                write!(f, "unsupported encoding (not UTF-8 or GB18030)")
            }
            ParseError::MissingHeader(expected) => {
                write!(f, "could not find header row starting with '{expected}'")
            }
            ParseError::BadAmount(v) => write!(f, "bad amount '{v}'"),
            ParseError::BadDate(v) => write!(f, "bad date '{v}'"),
            ParseError::Csv(msg) => write!(f, "csv: {msg}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse options shared by the Alipay variants. The WeChat
/// parser only exposes `include_pending`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ParseOptions {
    /// Include `其他` / `不计收支` rows, inferring direction
    /// from the `备注` cell (`退款` → CREDIT, `余额宝-单次转入`
    /// → DEBIT).
    pub include_other: bool,
    /// Keep rows whose status is not the settled state
    /// (`支付成功` / `收款成功`, or `已收入` / `已支出`).
    pub include_pending: bool,
}

/// Re-exported so the shared 7-column row shape carries
/// through from preview to commit for every importer.
pub use crate::handlers::import::ParsedRow;

pub mod statement;
