//! Upload validation: size cap, content sniff, and accepted-MIME
//! policy.
//!
//! Three concerns live here, all governed by the `upload-validation`
//! spec (`s10-upload-validation`):
//!
//! 1. **Body-size cap.** Enforced at the router layer by
//!    [`tower_http::limit::DefaultBodyLimit`]. The cap is
//!    [`DEFAULT_MAX_BYTES`] (25 MiB) by default and is overridable
//!    at startup via [`Config::upload_max_bytes`].
//! 2. **MIME validation.** Call [`validate`] after reading the
//!    file bytes: it sniffs the first 4 KB with the `infer`
//!    crate, compares against the multipart-declared
//!    `Content-Type`, and either accepts, rejects (with a
//!    caller-supplied 400), or rewrites the stored MIME for the
//!    office / archive formats where browsers mislabel
//!    (XLSX, DOCX, …).
//! 3. **Per-user quota warning.** Call
//!    [`maybe_warn_quota`] after a successful upload; if the
//!    uploader has exceeded 1 GiB in the last 24 hours, a
//!    structured WARN log line is emitted.
//!
//! All helpers are pure (no I/O beyond the SQL query the caller
//! passes into `maybe_warn_quota`). Unit tests live in
//! `#[cfg(test)] mod tests` at the bottom of this file.

use std::sync::OnceLock;

/// Default upload body cap: 25 MiB. Override via the
/// `UPLOAD_MAX_BYTES` env var at startup.
pub const DEFAULT_MAX_BYTES: usize = 25 * 1024 * 1024;

/// Default per-user quota warning threshold: 1 GiB / 24 h.
pub const QUOTA_WARN_BYTES: i64 = 1024 * 1024 * 1024;

/// Quota window: 24 hours back from `now()`.
pub const QUOTA_WINDOW_HOURS: i64 = 24;

/// MIME types accepted on the multipart declaration even when
/// sniff is ambiguous or fails. CSV is the canonical example —
/// the `infer` crate cannot reliably distinguish CSV from any
/// other 7-bit ASCII file shorter than 4 KB.
pub const ACCEPTED_ON_DECLARATION: &[&str] = &["text/csv"];

/// MIME types trusted when sniff returns `None`. Plain text has
/// no magic number; legitimate `.txt` uploads will fail sniff
/// and we want to allow them.
pub const TRUSTED_WHEN_UNKNOWN: &[&str] = &["text/plain"];

/// Static map of (file extension, expected sniffed MIME) for
/// office and archive formats where browsers / OSes mislabel the
/// upload. When the extension matches AND sniff returns the
/// expected MIME, the sniffed MIME wins over the declared
/// `Content-Type` (per the spec scenario "real XLSX labeled as
/// `text/plain`").
pub const EXTENSION_WINS: &[(&str, &str)] = &[
    (
        "xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ),
    (
        "docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ),
    (
        "pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ),
    ("ods", "application/vnd.oasis.opendocument.spreadsheet"),
    ("odt", "application/vnd.oasis.opendocument.text"),
    ("odp", "application/vnd.oasis.opendocument.presentation"),
];

/// Error returned by [`validate`].
#[derive(Debug, PartialEq, Eq)]
pub enum UploadError {
    /// The body was empty.
    Empty,
    /// The sniffed MIME disagrees with the declared MIME and the
    /// exception rules do not apply.
    MimeMismatch { declared: String, sniffed: String },
    /// Sniff returned `None` and the declared type is not in
    /// [`TRUSTED_WHEN_UNKNOWN`].
    Unknown,
}

impl UploadError {
    /// Human-readable message safe to return to the user.
    pub fn message(&self) -> String {
        match self {
            Self::Empty => "Empty upload".into(),
            Self::MimeMismatch { declared, sniffed } => format!(
                "Content does not match declared type (declared {declared:?}, sniffed {sniffed:?})"
            ),
            Self::Unknown => "Could not determine content type".into(),
        }
    }
}

/// Sniff the first bytes of the upload body. Returns the canonical
/// MIME if the `infer` crate recognises a signature.
pub fn sniff(buf: &[u8]) -> Option<&'static str> {
    infer::get(buf).map(|t| t.mime_type())
}

/// Validate an upload. Returns the MIME type that should be
/// persisted (which may differ from `declared` for the office /
/// archive extensions in [`EXTENSION_WINS`]). The caller is
/// expected to surface [`UploadError::message`] as a 400 response
/// body.
///
/// `declared` is the multipart `Content-Type`. `filename` is the
/// multipart `filename` parameter (used only for the
/// extension-wins override). `buf` is the full file bytes; the
/// sniff library only inspects the first ~4 KB but we want the
/// size check (`is_empty`) and the caller's quota logic to use
/// the same buffer.
pub fn validate<'a>(
    declared: &'a str,
    filename: Option<&str>,
    buf: &[u8],
) -> Result<&'a str, UploadError> {
    if buf.is_empty() {
        return Err(UploadError::Empty);
    }

    if ACCEPTED_ON_DECLARATION.contains(&declared) {
        return Ok("text/csv");
    }

    let sniffed = sniff(buf);

    // Office / archive formats: extension-wins override.
    if let Some(fname) = filename {
        if let Some(ext) = extension_of(fname) {
            if let Some(&(_, expected)) = EXTENSION_WINS.iter().find(|(e, _)| *e == ext) {
                if sniffed == Some(expected) {
                    return Ok(expected);
                }
            }
        }
    }

    match sniffed {
        Some(mime) if mime == declared => Ok(mime),
        Some(sniffed_mime) => Err(UploadError::MimeMismatch {
            declared: declared.into(),
            sniffed: sniffed_mime.into(),
        }),
        None => {
            if TRUSTED_WHEN_UNKNOWN.contains(&declared) {
                Ok(declared)
            } else {
                Err(UploadError::Unknown)
            }
        }
    }
}

/// Lower-cased extension of a filename, or `None` if there is no
/// `.` in the name.
fn extension_of(filename: &str) -> Option<String> {
    let idx = filename.rfind('.')?;
    let ext = &filename[idx + 1..];
    if ext.is_empty() {
        None
    } else {
        Some(ext.to_ascii_lowercase())
    }
}

/// Pre-computed set of bytes written to a 30-byte body to provoke
/// the 413 path. Avoids allocating a 30 MiB Vec in tests.
#[allow(dead_code)]
pub fn fake_pdf_header() -> &'static [u8] {
    // `%PDF-1.4\n` magic + padding.
    static HEADER: OnceLock<Vec<u8>> = OnceLock::new();
    HEADER
        .get_or_init(|| {
            let mut v = b"%PDF-1.4\n".to_vec();
            v.resize(b"%PDF-1.4\n".len() + 64, b' ');
            v
        })
        .as_slice()
}

/// Check whether the user has exceeded [`QUOTA_WARN_BYTES`] in
/// the last [`QUOTA_WINDOW_HOURS`] hours, and emit a WARN log
/// line if so. `just_uploaded_bytes` is the size of the upload
/// that just succeeded.
///
/// This helper is called from the document upload handler; it
/// runs a single aggregate query against `documents` (no schema
/// change required). The function logs and returns; it never
/// errors out the upload.
pub async fn maybe_warn_quota(pool: &sqlx::PgPool, user_id: uuid::Uuid, just_uploaded_bytes: i64) {
    let total: Result<i64, _> = sqlx::query_scalar(
        "SELECT COALESCE(SUM(size_bytes), 0)::BIGINT FROM documents
         WHERE uploaded_by = $1 AND uploaded_at > now() - INTERVAL '24 hours'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await;
    let Ok(total) = total else {
        // Quota check is observability, not correctness — never
        // fail the upload because of it.
        return;
    };
    if total >= QUOTA_WARN_BYTES {
        tracing::warn!(
            user_id = %user_id,
            bytes_24h = total,
            last_upload = just_uploaded_bytes,
            "user exceeded upload quota (1 GiB / 24 h)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_pdf_returns_application_pdf() {
        let bytes = b"%PDF-1.4\n%\xc0\xc1\xc2\xc3\n";
        assert_eq!(sniff(bytes), Some("application/pdf"));
    }

    #[test]
    fn sniff_html_renamed_pdf_detects_html() {
        // HTML signature: `<!DOCTYPE html>` (case-insensitive) or
        // `<html>` etc. We use a real HTML header so `infer`
        // classifies it as text/html.
        let bytes = b"<!DOCTYPE html><html><body>oops</body></html>";
        assert_eq!(sniff(bytes), Some("text/html"));
    }

    #[test]
    fn sniff_csv_returns_none_for_short_input() {
        // `infer` does not have a CSV matcher — short CSVs must
        // be accepted on declaration.
        assert_eq!(sniff(b"date,amount\n2026-01-01,100\n"), None);
    }

    #[test]
    fn validate_real_pdf_declared_as_pdf_succeeds() {
        let bytes = b"%PDF-1.4\n%\xc0\xc1\xc2\xc3\n";
        let mime = validate("application/pdf", Some("receipt.pdf"), bytes).unwrap();
        assert_eq!(mime, "application/pdf");
    }

    #[test]
    fn validate_html_renamed_as_pdf_rejected() {
        let bytes = b"<!DOCTYPE html><html><body>x</body></html>";
        let err = validate("application/pdf", Some("evil.pdf"), bytes).unwrap_err();
        assert_eq!(
            err,
            UploadError::MimeMismatch {
                declared: "application/pdf".into(),
                sniffed: "text/html".into(),
            }
        );
    }

    #[test]
    fn validate_csv_accepted_on_declaration() {
        let bytes = b"date,amount\n2026-01-01,100\n";
        let mime = validate("text/csv", Some("ledger.csv"), bytes).unwrap();
        assert_eq!(mime, "text/csv");
    }

    #[test]
    fn validate_empty_rejected() {
        let err = validate("application/pdf", Some("empty.pdf"), b"").unwrap_err();
        assert_eq!(err, UploadError::Empty);
    }

    #[test]
    fn validate_xlsx_labeled_text_plain_rewrites_to_xlsx() {
        // Minimal byte sequence that the `infer` crate recognises
        // as XLSX: ZIP magic at offset 0 plus the OOXML directory
        // marker `xl/` at offset 0x1E (30). The matcher does not
        // need the rest of the OOXML container for classification.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PK\x03\x04");
        bytes.extend_from_slice(&[0u8; 26]); // fill to offset 30
        bytes.extend_from_slice(b"xl/");
        // Sanity: sniff sees xlsx.
        assert_eq!(
            sniff(&bytes),
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        );
        let mime = validate("text/plain", Some("sheet.xlsx"), &bytes).unwrap();
        assert_eq!(
            mime,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        );
    }

    #[test]
    fn validate_xlsx_labeled_text_plain_without_extension_rejected() {
        // Same bytes, but no `.xlsx` extension — sniff sees xlsx
        // but the extension-wins override doesn't fire. Declared
        // `text/plain` still mismatches, so we reject.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PK\x03\x04");
        bytes.extend_from_slice(&[0u8; 26]);
        bytes.extend_from_slice(b"xl/");
        let err = validate("text/plain", Some("sheet"), &bytes).unwrap_err();
        assert!(matches!(err, UploadError::MimeMismatch { .. }));
    }

    #[test]
    fn validate_plain_text_trusted_when_sniff_unknown() {
        // Sniff returns None for plain text. Without an extension
        // hint, declared `text/plain` is still trusted.
        let mime = validate("text/plain", Some("notes.txt"), b"hello world").unwrap();
        assert_eq!(mime, "text/plain");
    }

    #[test]
    fn validate_unknown_type_rejected() {
        let bytes = b"\x00\x01\x02\x03 random binary";
        let err = validate("application/x-frobnicate", Some("x.bin"), bytes).unwrap_err();
        // Could be `Unknown` (if infer returns None) or
        // `MimeMismatch` (if infer returns something else).
        // The important thing is that it isn't accepted.
        assert!(matches!(
            err,
            UploadError::Unknown | UploadError::MimeMismatch { .. }
        ));
    }

    #[test]
    fn extension_of_lowercases_and_skips_empty() {
        assert_eq!(extension_of("FOO.PDF"), Some("pdf".into()));
        assert_eq!(extension_of("noext"), None);
        assert_eq!(extension_of("trailing."), None);
        assert_eq!(extension_of("a.b.c.xlsx"), Some("xlsx".into()));
    }
}
