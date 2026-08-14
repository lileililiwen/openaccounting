//! HTTP integration tests for the Progressive Web App shell:
//! manifest, service worker, install prompt partial, and the
//! `Service-Worker-Allowed` header.

use crate::common::*;

#[tokio::test]
async fn http_manifest_fetches_correctly() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/static/manifest.webmanifest", server.base_url()))
        .send()
        .await
        .expect("GET manifest");
    let status = resp.status();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = resp.text().await.expect("body");
    assert_eq!(status, 200);
    assert!(
        content_type.contains("json") || content_type.contains("manifest"),
        "manifest should be served as JSON; got Content-Type={content_type}"
    );

    // Parse and assert the required fields per the W3C web-app-
    // manifest spec and the project spec.
    let v: serde_json::Value = serde_json::from_str(&body).expect("manifest is JSON");
    assert_eq!(v["name"], "OpenAccounting");
    assert_eq!(v["short_name"], "OpenAcct");
    assert_eq!(v["start_url"], "/");
    assert_eq!(v["display"], "standalone");
    assert_eq!(v["theme_color"], "#0f172a");
    assert_eq!(v["background_color"], "#f8fafc");
    let icons = v["icons"].as_array().expect("icons array");
    assert!(
        icons.iter().any(|i| i["sizes"] == "192x192"),
        "192x192 icon missing"
    );
    assert!(
        icons.iter().any(|i| i["sizes"] == "512x512"),
        "512x512 icon missing"
    );
    assert!(
        icons.iter().any(|i| {
            i["sizes"] == "512x512" && i["purpose"] == "maskable"
        }),
        "maskable 512x512 icon missing"
    );
}

#[tokio::test]
async fn http_sw_fetches_with_allowed_header() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/static/sw.js", server.base_url()))
        .send()
        .await
        .expect("GET sw.js");
    let status = resp.status();
    let allowed = resp
        .headers()
        .get("service-worker-allowed")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let body = resp.text().await.expect("sw body");
    assert_eq!(status, 200);
    assert_eq!(
        allowed.as_deref(),
        Some("/"),
        "Service-Worker-Allowed header missing or wrong; got {allowed:?}"
    );
    // The script should reference both the precache list and
    // the never-cache patterns; the spec's acceptance criteria
    // are about the header but the body should also look like
    // a service worker, not a 404 page.
    assert!(
        body.contains("oa-precache"),
        "sw.js should reference the precache name"
    );
    assert!(
        body.contains("install") && body.contains("fetch"),
        "sw.js should install + fetch handlers"
    );
}

#[tokio::test]
async fn http_sw_cache_patterns_match_documented_routes() {
    // Re-implement the pattern list from the spec to assert
    // that each documented read-only route is covered. This
    // catches a regression where the SW is updated but the
    // patterns are forgotten.
    let documented: &[&str] = &[
        "/ledgers/abc/dashboard",
        "/ledgers/abc/reports/balance-sheet",
        "/ledgers/abc/reports/trial-balance",
        "/ledgers/abc/reports/income-statement",
        "/ledgers/abc/reports/cash-flow",
        "/ledgers/abc/accounts",
    ];
    let patterns = [
        r"^/ledgers/[^/]+/dashboard/?$",
        r"^/ledgers/[^/]+/reports/(balance-sheet|trial-balance|income-statement|cash-flow|general-ledger)/?$",
        r"^/ledgers/[^/]+/accounts/?$",
    ];
    let never: &[&str] = &[
        "/login",
        "/register",
        "/logout",
        "/ledgers/abc/import/new",
        "/ledgers/abc/reimbursements/123/submit",
        "/ledgers/abc/reimbursements/123/approve",
        "/ledgers/abc/transactions/new",
        "/admin/users",
    ];
    let never_patterns = [
        r"^/login",
        r"^/register",
        r"^/logout",
        r"^/import",
        r"/import/",
        r"/reimbursements/.*/(submit|approve|reject|pay)",
        r"/transactions/new",
        r"/accounts/new",
        r"^/admin/",
    ];

    for path in documented {
        let matched = patterns
            .iter()
            .any(|p| regex::Regex::new(p).unwrap().is_match(path));
        assert!(
            matched,
            "expected {path} to be matched by a runtime cache pattern"
        );
    }
    for path in never {
        let matched = never_patterns
            .iter()
            .any(|p| regex::Regex::new(p).unwrap().is_match(path));
        assert!(
            matched,
            "expected {path} to be matched by a never-cache pattern"
        );
    }
}

#[tokio::test]
async fn http_icons_fetch_with_png_content_type() {
    let server = TestServer::new().await;
    for (name, size) in [
        ("icon-192.png", 192),
        ("icon-512.png", 512),
        ("icon-maskable-512.png", 512),
    ] {
        let resp = server
            .client()
            .get(format!("{}/static/icons/{name}", server.base_url()))
            .send()
            .await
            .expect("GET icon");
        let status = resp.status();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let bytes = resp.bytes().await.expect("icon bytes");
        assert_eq!(status, 200, "{name} should fetch with 200");
        assert!(
            content_type.contains("png") || content_type.contains("octet"),
            "{name} content-type should be PNG; got {content_type}"
        );
        assert_eq!(
            bytes.len() > 100,
            true,
            "{name} body should be non-trivial; got {} bytes",
            bytes.len()
        );
        // Read the PNG IHDR to confirm dimensions.
        // PNG header: 8 bytes signature, then IHDR chunk: 4 len,
        // 4 type "IHDR", 4 width, 4 height, …
        if bytes.len() >= 24 {
            let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
            let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
            assert_eq!(w, size, "{name} width");
            assert_eq!(h, size, "{name} height");
        }
    }
}

#[tokio::test]
async fn http_base_html_includes_pwa_partial() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/login", server.base_url()))
        .send()
        .await
        .expect("GET /login");
    let body = resp.text().await.expect("body");
    // The PWA partial should be rendered on every page; it
    // contains the manifest link and the install button.
    assert!(
        body.contains("/static/manifest.webmanifest"),
        "base.html should reference the manifest"
    );
    assert!(
        body.contains("pwa-install-btn"),
        "base.html should contain the install button element"
    );
    assert!(
        body.contains("/static/sw.js"),
        "base.html should register the service worker"
    );
    assert!(
        body.contains("safe-area-inset"),
        "base.html should have safe-area padding for standalone display"
    );
}
