//! WeChat Pay "personal-account" bill parser.
//!
//! The export (`我 → 服务 → 钱包 → 账单 → 常见问题 → 下载账单 →
//! 用于个人对账`) is a UTF-8 CSV. The first N rows are metadata
//! (微信昵称, 起始时间, 导出类型, 共N笔记录…); the canonical header
//! row starts with `交易时间,交易类型`. Data rows follow, one per
//! transaction, with 11 columns:
//!
//! `交易时间, 交易类型, 交易对方, 商品, 收/支, 金额(元), 支付方式,
//!  当前状态, 交易单号, 商户单号, 备注`
//!
//! Direction is taken from the explicit `收/支` column. Rows
//! whose `当前状态` is not `支付成功` or `收款成功` are dropped
//! (refunds, in-flight payments, etc.).

use crate::{
    handlers::import::ParsedRow,
    import::{dedup, encoding, ImportPlatform, ParseError, ParseOptions},
};

const HEADER: [&str; 2] = ["交易时间", "交易类型"];
/// Statuses that represent a settled transaction. Anything else
/// (已退款, 已全额退款, 支付中, …) is dropped.
const SETTLED: [&str; 2] = ["支付成功", "收款成功"];

/// Parse a WeChat bill into canonical rows.
///
/// `include_pending` keeps rows whose `当前状态` is not settled
/// (default: dropped). WeChat bills have no `其他` direction
/// column, so `include_other` is ignored.
pub fn parse(
    bytes: &[u8],
    options: &ParseOptions,
) -> Result<(Vec<ParsedRow>, &'static str), ParseError> {
    let (text, encoding) = encoding::decode(bytes)?;
    let rows = parse_str(&text, options)?;
    Ok((rows, encoding))
}

fn parse_str(text: &str, options: &ParseOptions) -> Result<Vec<ParsedRow>, ParseError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut records: Vec<Vec<String>> = Vec::new();
    for result in reader.records() {
        let record = result.map_err(|e| ParseError::Csv(e.to_string()))?;
        records.push(record.iter().map(|c| c.trim().to_string()).collect());
    }
    let Some(header_idx) = find_header(&records) else {
        return Err(ParseError::MissingHeader("交易时间,交易类型".into()));
    };
    let mut out = Vec::new();
    for cells in records.iter().skip(header_idx + 1) {
        if cells.is_empty() || cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        let status = cells.get(7).map(String::as_str).unwrap_or("");
        if !options.include_pending && !SETTLED.contains(&status) {
            continue;
        }
        let direction = cells.get(4).map(String::as_str).unwrap_or("/");
        let (debit, credit) = match direction {
            "收入" => (String::new(), parse_amount(&cells[5])?),
            "支出" => (parse_amount(&cells[5])?, String::new()),
            _ => continue, // "/" or empty → row dropped
        };
        let mut payee = cells.get(2).cloned().unwrap_or_default();
        payee = strip_payee_prefix(&payee);
        let date = cells.first().cloned().unwrap_or_default();
        let date = date.split_whitespace().next().unwrap_or("").to_string();
        let description = cells.get(3).cloned().unwrap_or_default();
        let amount_cents = money_cents(&debit)
            .or_else(|| money_cents(&credit))
            .unwrap_or(0);
        let fingerprint = dedup::fingerprint(&date, amount_cents, &payee);
        out.push(ParsedRow {
            date,
            description,
            debit,
            credit,
            account: None,
            payee: Some(payee.clone()),
            reference: cells.get(8).cloned().unwrap_or_default().into(),
            is_duplicate: false,
            fingerprint,
            platform: ImportPlatform::Wechat,
        });
    }
    out.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(out)
}

/// Locate the row whose first two cells are exactly the
/// canonical header. Returns its index in `records`, or `None`.
fn find_header(records: &[Vec<String>]) -> Option<usize> {
    records.iter().position(|cells| {
        cells.first().map(String::as_str) == Some(HEADER[0])
            && cells.get(1).map(String::as_str) == Some(HEADER[1])
    })
}

/// Strip the literal `发给` prefix from a payee (WeChat adds it
/// to red-packet recipients).
fn strip_payee_prefix(payee: &str) -> String {
    payee.strip_prefix("发给").unwrap_or(payee).to_string()
}

fn parse_amount(raw: &str) -> Result<String, ParseError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(String::new());
    }
    let trimmed = raw.trim_start_matches('+').trim_start_matches('-');
    let d: rust_decimal::Decimal = trimmed
        .parse()
        .map_err(|_| ParseError::BadAmount(raw.to_string()))?;
    if d <= rust_decimal::Decimal::ZERO {
        return Ok(String::new());
    }
    Ok(format!("{d:.2}"))
}

fn money_cents(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let d: rust_decimal::Decimal = s.parse().ok()?;
    (d * rust_decimal::Decimal::from(100))
        .round()
        .try_into()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../../tests/fixtures/wechat_personal_sample.csv");

    #[test]
    fn parses_sample_with_three_rows() {
        let (rows, _enc) = parse(SAMPLE.as_bytes(), &ParseOptions::default()).expect("parse");
        assert_eq!(rows.len(), 3, "rows: {rows:#?}");
        // 星巴克: 支出 28.00, payee stripped of nothing.
        assert_eq!(rows[0].debit, "28.00");
        assert_eq!(rows[0].credit, "");
        assert_eq!(rows[0].payee.as_deref(), Some("星巴克咖啡"));
        // 发给小红 → payee stripped of 发给.
        assert_eq!(rows[1].payee.as_deref(), Some("小红"));
        // 收入 100.00.
        assert_eq!(rows[2].credit, "100.00");
        // Sorted ascending by date.
        assert!(rows[0].date <= rows[1].date && rows[1].date <= rows[2].date);
    }

    #[test]
    fn finds_header_after_any_number_of_metadata_rows() {
        let header = "交易时间,交易类型,交易对方,商品,收/支,金额(元),支付方式,当前状态,交易单号,商户单号,备注";
        for n in [0usize, 1, 16, 30] {
            let mut text = String::new();
            for _ in 0..n {
                text.push_str("微信昵称：[test]\n");
            }
            text.push_str(header);
            text.push('\n');
            text.push_str(
                "2026-08-01 09:15:00,商户消费,星巴克咖啡,拿铁,支出,28.00,零钱,支付成功,1,,\n",
            );
            let (rows, _) = parse(text.as_bytes(), &ParseOptions::default()).expect("parse");
            assert_eq!(rows.len(), 1, "metadata rows = {n}");
        }
    }

    #[test]
    fn drops_non_settled_status() {
        let text = "\
交易时间,交易类型,交易对方,商品,收/支,金额(元),支付方式,当前状态,交易单号,商户单号,备注
2026-08-01 09:15:00,商户消费,星巴克,拿铁,支出,28.00,零钱,支付成功,1,,
2026-08-02 09:15:00,商户消费,退款店,商品,支出,50.00,零钱,已退款,2,,
";
        let (rows, _) = parse(text.as_bytes(), &ParseOptions::default()).expect("parse");
        assert_eq!(rows.len(), 1);
        // With include_pending, the refunded row is kept.
        let (rows, _) = parse(
            text.as_bytes(),
            &ParseOptions {
                include_pending: true,
                ..ParseOptions::default()
            },
        )
        .expect("parse");
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn gb18030_sample_parses_identically() {
        // Encode the sample header + one row as GBK.
        let gbk = {
            let (cow, _, _) = encoding_rs::GB18030.encode(
                "交易时间,交易类型,交易对方,商品,收/支,金额(元),支付方式,当前状态,交易单号,商户单号,备注\n\
                 2026-08-01 09:15:00,商户消费,星巴克咖啡,拿铁,支出,28.00,零钱,支付成功,1,,\n",
            );
            cow.into_owned()
        };
        let (rows, enc) = parse(&gbk, &ParseOptions::default()).expect("parse");
        assert_eq!(enc, "GB18030");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].debit, "28.00");
        assert_eq!(rows[0].payee.as_deref(), Some("星巴克咖啡"));
    }
}
