//! Tesseract OCR engine adapter.
//!
//! Shells out to the `tesseract` binary on PATH. PDF inputs are
//! first converted to PNG via `pdftoppm` (from `poppler-utils`);
//! if the tool is absent, [`OcrError::PdfToolingMissing`] is
//! returned and the upload itself is unaffected.

use std::path::PathBuf;

use async_trait::async_trait;
use tokio::process::Command;

use super::{extract_merchant, parse_amount, parse_date, OcrEngine, OcrError, OcrResult};

/// OCR engine that drives the local `tesseract` binary.
pub struct TesseractEngine {
    /// Path to the `tesseract` executable (default: `tesseract`
    /// which resolves via PATH).
    pub bin: PathBuf,
    /// Tesseract language string (e.g. `"eng"` or `"eng+chi_sim"`).
    pub lang: String,
}

impl Default for TesseractEngine {
    fn default() -> Self {
        Self {
            bin: PathBuf::from("tesseract"),
            lang: "eng".to_string(),
        }
    }
}

#[async_trait]
impl OcrEngine for TesseractEngine {
    async fn extract(&self, bytes: &[u8], mime: &str) -> Result<OcrResult, OcrError> {
        // For PDF inputs: convert first page to PNG via pdftoppm.
        let image_bytes: Vec<u8> = if mime == "application/pdf" {
            pdf_to_png(bytes).await?
        } else {
            bytes.to_vec()
        };

        // Write the image to a temp file.
        let dir = std::env::temp_dir().join(format!("oa_ocr_{}", uuid::Uuid::new_v4().simple()));
        tokio::fs::create_dir_all(&dir).await?;
        let input_path = dir.join("input.img");
        tokio::fs::write(&input_path, &image_bytes).await?;

        // Run: tesseract <input> stdout -l <lang> tsv
        let output = Command::new(&self.bin)
            .arg(&input_path)
            .arg("stdout")
            .arg("-l")
            .arg(&self.lang)
            .arg("tsv")
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    OcrError::TesseractMissing
                } else {
                    OcrError::Io(e)
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(OcrError::ProcessFailed(stderr));
        }

        let tsv = String::from_utf8_lossy(&output.stdout).to_string();
        let (raw_text, confidence) = parse_tsv(&tsv);

        let amount = parse_amount(&raw_text);
        let txn_date = parse_date(&raw_text);
        let merchant = extract_merchant(&raw_text);

        Ok(OcrResult {
            amount,
            txn_date,
            merchant,
            raw_text,
            confidence,
        })
    }
}

/// Convert the first page of a PDF to PNG bytes using `pdftoppm`.
async fn pdf_to_png(pdf_bytes: &[u8]) -> Result<Vec<u8>, OcrError> {
    let dir = std::env::temp_dir().join(format!("oa_ocr_{}", uuid::Uuid::new_v4().simple()));
    tokio::fs::create_dir_all(&dir).await?;
    let pdf_path = dir.join("input.pdf");
    let png_prefix = dir.join("out");
    let expected_png = dir.join("out-1.png");

    tokio::fs::write(&pdf_path, pdf_bytes).await?;

    let status = Command::new("pdftoppm")
        .arg("-r")
        .arg("150")
        .arg("-png")
        .arg("-singlefile")
        .arg(&pdf_path)
        .arg(&png_prefix)
        .status()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                OcrError::PdfToolingMissing
            } else {
                OcrError::Io(e)
            }
        })?;

    if !status.success() {
        return Err(OcrError::ProcessFailed(
            "pdftoppm returned non-zero exit code".into(),
        ));
    }

    let png = tokio::fs::read(&expected_png).await?;
    Ok(png)
}

/// Parse Tesseract TSV output.
///
/// Returns `(raw_text, mean_confidence)`.
/// TSV columns: level, page_num, block_num, par_num, line_num,
/// word_num, left, top, width, height, conf, text
fn parse_tsv(tsv: &str) -> (String, f32) {
    let mut words: Vec<String> = Vec::new();
    let mut conf_sum: f64 = 0.0;
    let mut conf_count: usize = 0;

    for line in tsv.lines().skip(1) {
        // Skip header
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 12 {
            continue;
        }
        let conf_str = cols[10];
        let text = cols[11];
        if text.trim().is_empty() {
            continue;
        }
        if let Ok(conf) = conf_str.parse::<f64>() {
            if conf >= 0.0 {
                conf_sum += conf;
                conf_count += 1;
            }
        }
        words.push(text.to_string());
    }

    let raw_text = words.join(" ");
    let confidence = if conf_count > 0 {
        (conf_sum / conf_count as f64) as f32
    } else {
        0.0
    };
    (raw_text, confidence)
}
