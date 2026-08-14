//! Alipay mobile "交易流水证明" bill parser.
//!
//! The export (`我的 → 账单 → ⋯ → 开具交易流水证明 → 用于个人对账 →
//! 申请`) is a GB18030 CSV. The block is preceded by a banner
//! row (`----支付宝（中国）网络技术有限公司 电子客户回单----`) and a
//! company row; the canonical header starts with `交易号,商家订单号`
//! and has 16 columns:
//!
//! `交易号, 商家订单号, 交易创建时间, 付款时间, 最近修改时间,
//!  交易来源地, 类型, 交易对方, 商品名称, 金额（元）, 收/支,
//!  交易状态, 服务费（元）, 成功退款（元）, 备注, 资金状态`
//!
//! Direction comes from `收/支`. `其他` / `不计收支` rows are
//! dropped unless `include_other` is set, in which case the
//! `备注` cell infers the sign (`退款` → CREDIT, `余额宝-单次转入`
//! → DEBIT). Rows whose `资金状态` is not `已收入` / `已支出` are
//! dropped unless `include_pending` is set.

use crate::{
    handlers::import::ParsedRow,
    import::{dedup, encoding, ImportPlatform, ParseError, ParseOptions},
};

const HEADER: [&str; 2] = ["交易号", "商家订单号"];
const SETTLED: [&str; 2] = ["已收入", "已支出"];

/// Parse an Alipay mobile bill into canonical rows. Returns the
/// rows and the detected encoding.
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
        return Err(ParseError::MissingHeader("交易号,商家订单号".into()));
    };
    let mut out = Vec::new();
    for cells in records.iter().skip(header_idx + 1) {
        if cells.is_empty() || cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        let fund_status = cells.get(15).map(String::as_str).unwrap_or("");
        if !options.include_pending && !SETTLED.contains(&fund_status) {
            continue;
        }
        let direction = cells.get(10).map(String::as_str).unwrap_or("");
        let (debit, credit) = match direction {
            "收入" => (String::new(), parse_amount(&cells[9])?),
            "支出" => (parse_amount(&cells[9])?, String::new()),
            "其他" | "不计收支" if options.include_other => {
                let note = cells.get(14).cloned().unwrap_or_default();
                let amt = parse_amount(&cells[9])?;
                if note.contains("退款") {
                    (String::new(), amt)
                } else if note.contains("余额宝-单次转入") {
                    (amt, String::new())
                } else {
                    continue;
                }
            }
            _ => continue,
        };
        let payee = cells.get(7).cloned().unwrap_or_default();
        let date = cells
            .get(2)
            .cloned()
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        let description = cells.get(8).cloned().unwrap_or_default();
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
            reference: cells.first().cloned().unwrap_or_default().into(),
            is_duplicate: false,
            fingerprint,
            platform: ImportPlatform::AlipayMobile,
        });
    }
    out.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(out)
}

fn find_header(records: &[Vec<String>]) -> Option<usize> {
    records.iter().position(|cells| {
        cells.first().map(String::as_str) == Some(HEADER[0])
            && cells.get(1).map(String::as_str) == Some(HEADER[1])
    })
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

    const SAMPLE: &str = include_str!("../../tests/fixtures/alipay_mobile_sample.csv");

    #[test]
    fn parses_one_row_by_default() {
        let (rows, _) = parse(SAMPLE.as_bytes(), &ParseOptions::default()).expect("parse");
        assert_eq!(rows.len(), 1, "rows: {rows:#?}");
        assert_eq!(rows[0].debit, "28.00");
        assert_eq!(rows[0].credit, "");
        assert_eq!(rows[0].payee.as_deref(), Some("星巴克咖啡"));
    }

    #[test]
    fn infers_credit_on_refund_when_toggled() {
        let (rows, _) = parse(
            SAMPLE.as_bytes(),
            &ParseOptions {
                include_other: true,
                ..ParseOptions::default()
            },
        )
        .expect("parse");
        assert_eq!(rows.len(), 2, "rows: {rows:#?}");
        let refund = rows
            .iter()
            .find(|r| r.payee.as_deref() == Some("淘宝商家"))
            .expect("refund row");
        assert_eq!(refund.credit, "100.00");
        assert_eq!(refund.debit, "");
    }

    #[test]
    fn drops_pending_fund_status_unless_toggled() {
        // Row 3 has 资金状态=待确认 and is dropped by default.
        let (rows, _) = parse(SAMPLE.as_bytes(), &ParseOptions::default()).expect("parse");
        assert!(rows.iter().all(|r| r.payee.as_deref() != Some("中国移动")));
        let (rows, _) = parse(
            SAMPLE.as_bytes(),
            &ParseOptions {
                include_pending: true,
                ..ParseOptions::default()
            },
        )
        .expect("parse");
        assert!(
            rows.iter().any(|r| r.payee.as_deref() == Some("中国移动")),
            "pending row kept when toggled: {rows:#?}"
        );
    }
}
