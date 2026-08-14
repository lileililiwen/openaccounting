//! Integration tests for the Alipay dedicated importer (mobile +
//! web). Drives the real axum router via `TestServer`.

use crate::common::*;
use uuid::Uuid;

const MOBILE_REFUND: &str = "\
----支付宝（中国）网络技术有限公司  电子客户回单----,,,,,,,,,,,,,,,,
支付宝（中国）网络技术有限公司,,,,,,,,,,,,,,,,
交易号,商家订单号,交易创建时间,付款时间,最近修改时间,交易来源地,类型,交易对方,商品名称,金额（元）,收/支,交易状态,服务费（元）,成功退款（元）,备注,资金状态
2026080000000001,2026080000000001,2026-08-01 09:15:00,2026-08-01 09:15:03,2026-08-01 09:15:03,杭州,购物,星巴克咖啡,拿铁,28.00,支出,交易成功,0.00,0.00,买咖啡,已支出
2026080000000003,2026080000000003,2026-08-05 08:00:00,2026-08-05 08:00:02,2026-08-05 08:00:02,杭州,其他,淘宝商家,退款,100.00,其他,退款成功,0.00,100.00,买家退款,已收入
";

const WEB_SHORT: &str = "\
------------------------------------------------------
支付宝交易记录明细查询
账号：test@example.com
起始日期：[2026-08-01] 终止日期：[2026-08-31]
交易时间,交易分类,交易对方,对方账号,商品说明,收/支,金额,收付款方式,交易状态,交易订单号,商家订单号,备注
2026-08-01 09:15:00,购物,星巴克咖啡,sf@example.com,拿铁,支出,28.00,余额,交易成功,20260801001,,买咖啡
2026-08-03 18:40:00,转账,王老板,wb@example.com,转账,收入,100.00,余额,交易成功,20260801002,,餐费
";

async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "Alipay Co"),
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

fn upload(
    server: &TestServer,
    cookie: &str,
    ledger_id: Uuid,
    name: &str,
    content: &str,
    include_other: bool,
) -> reqwest::RequestBuilder {
    let part = reqwest::multipart::Part::text(content.to_string()).file_name(name.to_string());
    let mut form = reqwest::multipart::Form::new()
        .text("filename", name.to_string())
        .part("file", part);
    if include_other {
        form = form.text("include_other", "on");
    }
    server
        .client()
        .post(format!(
            "{}/ledgers/{}/import/alipay",
            server.base_url(),
            ledger_id
        ))
        .header(reqwest::header::COOKIE, cookie.to_string())
        .multipart(form)
}

#[tokio::test]
async fn http_alipay_mobile_infers_credit_on_refund() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("ali", "ali@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;

    // Default: the 其他/退款 row is dropped → 1 row.
    let resp = upload(
        &server,
        &cookie,
        ledger_id,
        "alipay.csv",
        MOBILE_REFUND,
        false,
    )
    .send()
    .await
    .expect("upload");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200);
    assert!(body.contains("星巴克咖啡"), "should contain payee");
    assert!(body.contains("alipay_mobile"), "format label");
    assert!(!body.contains("淘宝商家"), "refund row dropped by default");

    // Toggle on: the 其他/退款 row becomes a credit.
    let resp = upload(
        &server,
        &cookie,
        ledger_id,
        "alipay.csv",
        MOBILE_REFUND,
        true,
    )
    .send()
    .await
    .expect("upload");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200);
    assert!(body.contains("淘宝商家"), "refund row kept when toggled");
    assert!(body.contains("100.00"), "refund amount shown");
}

#[tokio::test]
async fn http_alipay_web_handles_short_columns() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("ali2", "ali2@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let resp = upload(&server, &cookie, ledger_id, "alipay.txt", WEB_SHORT, false)
        .send()
        .await
        .expect("upload");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200);
    assert!(body.contains("星巴克咖啡"), "web row parsed");
    assert!(body.contains("王老板"), "web income row parsed");
    assert!(body.contains("alipay_web"), "format label");
    assert!(
        !body.contains("付款账户"),
        "web variant must not surface 付款账户"
    );
}

#[tokio::test]
async fn http_auto_detect_dispatches_wechat_via_generic_import() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("det", "det@example.com", "correct horse battery staple")
        .await;
    let ledger_id = make_ledger(&server).await;
    let part = reqwest::multipart::Part::text(
        "交易时间,交易类型,交易对方,商品,收/支,金额(元),支付方式,当前状态,交易单号,商户单号,备注\n\
         2026-08-01 09:00:00,商户消费,商家A,商品A,支出,10.00,零钱,支付成功,1,,\n"
            .to_string(),
    )
    .file_name("bill.csv");
    let form = reqwest::multipart::Form::new()
        .text("filename", "bill.csv")
        .part("file", part);
    let resp = server
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
        .expect("upload");
    let status = resp.status();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200);
    assert!(
        body.contains("wechat"),
        "generic import should dispatch to wechat preview; got {body}"
    );
    assert!(body.contains("商家A"));
}
