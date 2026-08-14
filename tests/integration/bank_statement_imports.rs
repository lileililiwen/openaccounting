//! Integration tests for the bank-statement importers
//! (OFX / QIF / MT940). Drives the real axum router via
//! `TestServer` with a multipart upload.

use crate::common::*;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

/// Minimal URL-encoder for form bodies. We only need the
/// handful of characters that appear in our test data:
/// letters, digits, `-`, `.`, `_`, `~`, and `,`.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') {
            out.push(c);
        } else {
            // Encode each UTF-8 byte as %HH.
            for b in c.to_string().as_bytes() {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

const QFX_SAMPLE: &str = "OFXHEADER:100\n\
DATA:OFXSGML\n\
VERSION:102\n\
SECURITY:NONE\n\
ENCODING:USASCII\n\
CHARSET:1252\n\
COMPRESSION:NONE\n\
OLDFILEUID:NONE\n\
NEWFILEUID:NONE\n\
\n\
<OFX>\n\
<BANKMSGSRSV1>\n\
<STMTTRNRS>\n\
<TRNUID>1</TRNUID>\n\
<STATUS><CODE>0</CODE><SEVERITY>INFO</SEVERITY></STATUS>\n\
<STMTRS>\n\
<CURDEF>USD</CURDEF>\n\
<BANKACCTFROM><BANKID>123</BANKID><ACCTID>456</ACCTID><ACCTTYPE>CHECKING</ACCTTYPE></BANKACCTFROM>\n\
<BANKTRANLIST>\n\
<DTSTART>20260814</DTSTART>\n\
<DTEND>20260814</DTEND>\n\
<STMTTRN>\n\
<TRNTYPE>DEBIT</TRNTYPE>\n\
<DTPOSTED>20260814120000</DTPOSTED>\n\
<TRNAMT>-42.50</TRNAMT>\n\
<FITID>20260814001</FITID>\n\
<NAME>STARBUCKS</NAME>\n\
<MEMO>Latte</MEMO>\n\
</STMTTRN>\n\
</BANKTRANLIST>\n\
</STMTRS>\n\
</STMTTRNRS>\n\
</BANKMSGSRSV1>\n\
</OFX>\n";

const QIF_SAMPLE: &str = "!Type:Bank\n\
D08/14/2026\n\
T-12.34\n\
PSupermarket\n\
MWeekly shop\n\
N123\n\
^\n\
D08/15/2026\n\
T-5.00\n\
PCoffee shop\n\
^\n";

const MT940_SAMPLE: &str = ":20:STATEMENT\n\
:25:1234567890\n\
:60F:C260101EUR100,00\n\
:61:2608140814D42,50N024NONREF//Settlement\n\
:86: ?20PMT ?21Latte ?32STARBUCKS\n\
:62F:C260814EUR57,50\n\
:61:2608150815C100,00N001SALARY//August\n\
:86: ?32Acme Inc\n\
:62F:C260815EUR157,50\n";

async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "BankStmt Co"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .expect("POST /ledgers/new");
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("Location")
        .to_string();
    Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

async fn account_id(pool: &PgPool, ledger_id: Uuid, name: &str) -> Uuid {
    let (id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM accounts WHERE ledger_id = $1 AND name = $2",
    )
    .bind(ledger_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("account exists");
    id
}

async fn upload_file(
    server: &TestServer,
    cookie: &str,
    ledger_id: Uuid,
    filename: &str,
    content: &str,
) -> reqwest::Response {
    let part = reqwest::multipart::Part::text(content.to_string())
        .file_name(filename.to_string());
    let form = reqwest::multipart::Form::new()
        .text("filename", filename.to_string())
        .part("file", part);
    server
        .client()
        .post(format!(
            "{}/ledgers/{}/import",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .multipart(form)
        .send()
        .await
        .expect("upload")
}

#[tokio::test]
async fn http_qfx_upload_previews_one_row() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("alice", "alice@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let resp = upload_file(
        &server,
        &cookie,
        ledger_id,
        "statement.qfx",
        QFX_SAMPLE,
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200);
    assert!(body.contains("STARBUCKS"), "body should contain payee");
    assert!(body.contains("Latte"), "body should contain memo");
    assert!(body.contains("2026-08-14"), "body should contain date");
    assert!(body.contains("42.50"), "body should contain amount");
    assert!(body.contains("ofx"), "body should label the format");
}

#[tokio::test]
async fn http_qif_upload_previews_two_rows() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("bob", "bob@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let resp = upload_file(
        &server,
        &cookie,
        ledger_id,
        "statement.qif",
        QIF_SAMPLE,
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200);
    assert!(body.contains("Supermarket"));
    assert!(body.contains("Coffee shop"));
    assert!(body.contains("qif"));
}

#[tokio::test]
async fn http_mt940_upload_previews_two_rows() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("carol", "carol@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let resp = upload_file(
        &server,
        &cookie,
        ledger_id,
        "statement.sta",
        MT940_SAMPLE,
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200);
    assert!(body.contains("STARBUCKS"));
    assert!(body.contains("Acme Inc"));
    assert!(body.contains("mt940"));
}

#[tokio::test]
async fn http_qfx_commit_creates_transactions() {
    let server = TestServer::new().await;
    let pool = server.db().pool();
    let cookie = server
        .bootstrap_user("dave", "dave@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let cash = account_id(&pool, ledger_id, "Cash on Hand").await;
    let resp = upload_file(
        &server,
        &cookie,
        ledger_id,
        "statement.qfx",
        QFX_SAMPLE,
    )
    .await;
    assert_eq!(resp.status(), 200);

    // Now POST /import/confirm with the rows from the parser.
    // We re-parse on the test side to avoid scraping the HTML.
    let rows = openaccounting::import::ofx::parse(QFX_SAMPLE);
    let rows_json = serde_json::to_string(&rows).unwrap();
    // The handler uses `Form<ImportConfirmForm>` (urlencoded),
    // not multipart — `rows` is deserialized via serde_json
    // inside the struct from the urlencoded JSON text.
    let mut form_body: Vec<(String, String)> = vec![
        ("filename".to_string(), "statement.qfx".to_string()),
        ("account_id".to_string(), cash.to_string()),
        ("default_account_id".to_string(), cash.to_string()),
        ("rows".to_string(), rows_json),
    ];
    // Form values must be > 0 for the usize fields; pass 0.
    form_body.push(("date_column".to_string(), "0".to_string()));
    form_body.push(("description_column".to_string(), "1".to_string()));
    let body = form_body
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let resp = server
        .client()
        .post(format!(
            "{}/ledgers/{}/import/confirm",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .expect("confirm");
    let status = resp.status();
    let body = resp.text().await.expect("confirm body");
    assert!(
        status == 303 || status == 302,
        "confirm should redirect; got {status} body={body}"
    );

    // Verify the transaction was created.
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE ledger_id = $1",
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
    let (sum_d, sum_c): (Decimal, Decimal) = sqlx::query_as(
        r#"SELECT
             COALESCE(SUM(CASE WHEN direction='DEBIT'  THEN amount ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN direction='CREDIT' THEN amount ELSE 0 END), 0)
           FROM postings p
           JOIN transactions t ON t.id = p.transaction_id
           WHERE t.ledger_id = $1"#,
    )
    .bind(ledger_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sum_d, sum_c, "trial balance must be in balance");
    assert_eq!(sum_d, Decimal::new(4250, 2));
}
