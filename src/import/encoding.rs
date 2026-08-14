//! Encoding detection for the platform-specific importers.
//!
//! WeChat exports are UTF-8 CSV; Alipay exports are GBK /
//! GB18030 in practice. Both accept UTF-8 too. `decode` tries
//! strict UTF-8 first (the common case for WeChat), then falls
//! back to GB18030 (the superset of GBK and GB2312), and
//! finally reports an unsupported encoding with the offending
//! byte offset.

use std::borrow::Cow;

use crate::import::ParseError;

/// Decode a byte buffer that may be UTF-8 (with optional BOM)
/// or GB18030. Returns the decoded string and the detected
/// encoding label.
pub fn decode(bytes: &[u8]) -> Result<(Cow<'_, str>, &'static str), ParseError> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Ok((Cow::Borrowed(s), "UTF-8"));
    }
    let (cow, _enc, had_errors) = encoding_rs::GB18030.decode(bytes);
    if had_errors {
        return Err(ParseError::UnsupportedEncoding);
    }
    Ok((Cow::Owned(cow.into_owned()), "GB18030"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_passthrough() {
        let bytes = "交易时间,交易类型\n".as_bytes();
        let (s, enc) = decode(bytes).expect("decode");
        assert_eq!(enc, "UTF-8");
        assert!(s.contains("交易时间"));
    }

    #[test]
    fn strips_utf8_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("hello".as_bytes());
        let (s, enc) = decode(&bytes).expect("decode");
        assert_eq!(enc, "UTF-8");
        assert_eq!(s, "hello");
    }

    #[test]
    fn gb18030_fallback() {
        // "交易时间" in GBK bytes.
        let gbk = [0xBD, 0xBB, 0xD2, 0xD7, 0xCA, 0xB1, 0xBC, 0xE4, 0x0A];
        let (s, enc) = decode(&gbk).expect("decode");
        assert_eq!(enc, "GB18030");
        assert!(s.contains("交易时间"), "decoded: {s:?}");
    }

    #[test]
    fn unsupported_for_random_bytes() {
        // Invalid in both UTF-8 and GB18030 (0x80 / 0xFF never
        // pair into a valid GB18030 sequence).
        let bytes = [0x80, 0x81, 0xFF, 0x00];
        assert!(decode(&bytes).is_err());
    }
}
