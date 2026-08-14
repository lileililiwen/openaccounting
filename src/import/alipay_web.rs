//! Alipay web "下载Txt格式账单" parser.
//!
//! The export is a `.txt` file (not CSV) with four leading
//! banner rows, then a 12-column header:
//!
//! `交易时间, 交易分类, 交易对方, 对方账号, 商品说明, 收/支, 金额,
//!  收付款方式, 交易状态, 交易订单号, 商家订单号, 备注`
//!
//! The web variant does **not** include a `付款账户` column; the
//! parser does not pretend to surface that field. Direction and
//! drop rules mirror the mobile variant.

use crate::{
    handlers::import::ParsedRow,
    import::{dedup, encoding, ImportPlatform, ParseError, ParseOptions},
};

const HEADER: [&str; 2] = ["交易时间", "交易分类"];
const SETTLED: [&str; 1] = ["交易成功"];

/// Parse an Alipay web bill into canonical rows. Returns the
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
        return Err(ParseError::MissingHeader("交易时间,交易分类".into()));
    };
    let mut out = Vec::new();
    for cells in records.iter().skip(header_idx + 1) {
        if cells.is_empty() || cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        let status = cells.get(8).map(String::as_str).unwrap_or("");
        if !options.include_pending && !SETTLED.contains(&status) {
            continue;
        }
        let direction = cells.get(5).map(String::as_str).unwrap_or("");
        let (debit, credit) = match direction {
            "收入" => (String::new(), parse_amount(&cells[6])?),
            "支出" => (parse_amount(&cells[6])?, String::new()),
            "其他" | "不计收支" if options.include_other => {
                let note = cells.get(11).cloned().unwrap_or_default();
                let amt = parse_amount(&cells[6])?;
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
        let payee = cells.get(2).cloned().unwrap_or_default();
        let date = cells
            .first()
            .cloned()
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        let description = cells.get(4).cloned().unwrap_or_default();
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
            reference: cells.get(9).cloned().unwrap_or_default().into(),
            is_duplicate: false,
            fingerprint,
            platform: ImportPlatform::AlipayWeb,
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

    const SAMPLE: &str = include_str!("../../tests/fixtures/alipay_web_sample.txt");

    #[test]
    fn parses_web_sample() {
        let (rows, _) = parse(SAMPLE.as_bytes(), &ParseOptions::default()).expect("parse");
        assert_eq!(rows.len(), 3, "rows: {rows:#?}");
        assert_eq!(rows[0].debit, "28.00");
        assert_eq!(rows[0].payee.as_deref(), Some("星巴克咖啡"));
        assert_eq!(rows[1].credit, "100.00");
        assert_eq!(rows[1].payee.as_deref(), Some("王老板"));
        assert_eq!(rows[2].debit, "50.00");
        // Web variant never surfaces 付款账户.
        assert!(rows.iter().all(|r| !r.description.contains("付款账户")));
    }

    #[test]
    fn skips_banner_rows() {
        let (rows, _) = parse(SAMPLE.as_bytes(), &ParseOptions::default()).expect("parse");
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn gb18030_web_sample_parses() {
        let (cow, _, _) = encoding_rs::GB18030.encode(SAMPLE);
        let (rows, enc) = parse(&cow, &ParseOptions::default()).expect("parse");
        assert_eq!(enc, "GB18030");
        assert_eq!(rows.len(), 3);
    }
}
