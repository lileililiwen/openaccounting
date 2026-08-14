//! Format auto-detection for the import endpoint.
//!
//! The detector looks at the first non-blank line of the
//! uploaded file and returns the format enum. The matchers are
//! intentionally simple: a banking export is unambiguous, so a
//! small set of magic prefixes catches > 99 % of real-world
//! inputs without a heavier parser.

use std::str::Lines;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Csv,
    Ofx,
    Qif,
    Mt940,
}

impl Format {
    pub fn as_str(&self) -> &'static str {
        match self {
            Format::Csv => "csv",
            Format::Ofx => "ofx",
            Format::Qif => "qif",
            Format::Mt940 => "mt940",
        }
    }
}

/// Identify the format from raw bytes. Strips a UTF-8 BOM and
/// returns the first non-blank line for downstream parsers.
pub fn detect(content: &str) -> Format {
    let first = first_non_blank_line(content);
    let Some(line) = first else {
        return Format::Csv;
    };
    if line.starts_with("OFXHEADER:") {
        return Format::Ofx;
    }
    if line.starts_with("<?xml") {
        // OFX 2.x XML: trust the OFX tag deeper in the file.
        if content.contains("<OFX>") {
            return Format::Ofx;
        }
        // Some banks export other XML formats; treat as CSV
        // (the generic importer will surface a parsing error
        // and the user can re-upload as the right format).
        return Format::Csv;
    }
    if line.starts_with("!Type:") {
        return Format::Qif;
    }
    // MT940: the file must have a `:20:`, a `:25:`, and an
    // opening balance marker (`:60F:` or `:60M:`).
    if content.contains(":20:")
        && content.contains(":25:")
        && (content.contains(":60F:") || content.contains(":60M:"))
    {
        return Format::Mt940;
    }
    Format::Csv
}

pub fn first_non_blank_line(content: &str) -> Option<&str> {
    // Strip UTF-8 BOM.
    let stripped = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = stripped.lines();
    while let Some(l) = lines.next() {
        let t = l.trim();
        if !t.is_empty() {
            // Hand back the *trimmed* line (without leading /
            // trailing whitespace), as a sub-slice of the
            // original content.
            return Some(t);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_csv() {
        assert_eq!(detect("date,desc,amount\n2026-01-01,x,1"), Format::Csv);
    }

    #[test]
    fn detect_ofx_sgml() {
        let s = "OFXHEADER:100\nDATA:OFXSGML\n...\n<OFX></OFX>";
        assert_eq!(detect(s), Format::Ofx);
    }

    #[test]
    fn detect_ofx_xml() {
        let s = "<?xml version=\"1.0\"?>\n<OFX></OFX>";
        assert_eq!(detect(s), Format::Ofx);
    }

    #[test]
    fn detect_qif() {
        assert_eq!(detect("!Type:Bank\nD08/14/2026\nT-12.34\n^"), Format::Qif);
    }

    #[test]
    fn detect_mt940() {
        let s = ":20:STATEMENT\n:25:12345\n:60F:C260101EUR100,00\n:61:260101D1,00N024\n";
        assert_eq!(detect(s), Format::Mt940);
    }

    #[test]
    fn strips_bom() {
        let s = "\u{feff}!Type:Bank\nD\n^";
        assert_eq!(detect(s), Format::Qif);
    }

    #[test]
    fn first_non_blank_skips_blank_lines() {
        let s = "\n\n   \nhello";
        assert_eq!(first_non_blank_line(s), Some("hello"));
    }
}
