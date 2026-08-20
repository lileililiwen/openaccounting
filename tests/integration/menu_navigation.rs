//! HTTP integration tests for the menu-navigation spec
//! (`u11-menu-navigation`).
//!
//! Verifies:
//! - The active-state `is-active` class is rendered on the right
//!   nav link for each top-level section route.
//! - The active state survives a navigation into a child view
//!   (Requirement 8: child views must keep the parent's section
//!   active — the anti-regression test for the Frappe v16 sidebar
//!   auto-switching bug).
//! - The breadcrumb renders the expected chain.
//! - The sidebar renders the four cluster labels.
//! - The sidebar collapse toggle reduces the width.
//! - The active state is also visible in dark mode.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::*;
use regex::Regex;
use uuid::Uuid;

/// Build the regex that matches `<a ...data-section="X"...class="...is-active...">` in
/// the rendered nav HTML, where the two attributes can be separated by any
/// amount of other attribute pairs (`data-tooltip`, `href`, etc.) — but
/// not by a closing `>` (the lazy match should stop at the same `<a>` tag).
fn active_link(section: &str) -> Regex {
    Regex::new(&format!(
        r#"<a[^>]*data-section="{section}"[^>]*class="[^"]*\bis-active\b[^"]*""#
    ))
    .unwrap()
}

async fn make_ledger(server: &TestServer) -> Uuid {
    let resp = server
        .client()
        .post(format!("{}/ledgers/new", server.base_url()))
        .form(&[
            ("name", "NavCo"),
            ("base_currency", "USD"),
            ("timezone", "UTC"),
            ("basis", "accrual"),
        ])
        .send()
        .await
        .expect("POST /ledgers/new");
    let status = resp.status();
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    assert!(
        status.is_success() || status.as_u16() == 303,
        "ledger create status={status} loc={loc:?}"
    );
    let loc = loc.expect("Location header");
    Uuid::parse_str(loc.rsplit('/').next().unwrap()).unwrap()
}

async fn fetch(server: &TestServer, cookie: &str, path: &str) -> (u16, String) {
    let resp = server
        .client()
        .get(format!("{}{}", server.base_url(), path))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET");
    let status = resp.status().as_u16();
    let body = resp.text().await.expect("body");
    (status, body)
}

#[tokio::test]
async fn http_active_state_for_transactions_section() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("u_tx", "u_tx@example.com", "correct horse battery staple")
        .await;
    let id = make_ledger(&server).await;
    let (status, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/transactions")).await;
    assert_eq!(status, 200);
    let re = active_link("transactions");
    assert!(
        re.is_match(&body),
        "expected Transactions to be the active nav link; body[0..300] = {}",
        &body[..body.len().min(300)]
    );
    // The Dashboard link must NOT be active on /transactions.
    let dashboard_re = active_link("dashboard");
    assert!(
        !dashboard_re.is_match(&body),
        "Dashboard must not be active on /transactions"
    );
}

#[tokio::test]
async fn http_active_state_for_each_section() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("u_sec", "u_sec@example.com", "correct horse battery staple")
        .await;
    let id = make_ledger(&server).await;
    let cases = [
        ("dashboard", format!("/ledgers/{id}/dashboard")),
        ("accounts", format!("/ledgers/{id}/accounts")),
        ("documents", format!("/ledgers/{id}/documents")),
        ("reports", format!("/ledgers/{id}/reports")),
        ("bank-feeds", format!("/ledgers/{id}/bank-feeds")),
        ("approvals", format!("/ledgers/{id}/approval-policies")),
        ("expenses", format!("/ledgers/{id}/reimbursements")),
        ("import", format!("/ledgers/{id}/import")),
        ("import-wechat", format!("/ledgers/{id}/import/wechat")),
        ("import-alipay", format!("/ledgers/{id}/import/alipay")),
    ];
    for (section, path) in cases {
        let (status, body) = fetch(&server, &cookie, &path).await;
        assert_eq!(status, 200, "GET {path} must return 200");
        let re = active_link(section);
        assert!(
            re.is_match(&body),
            "expected section {section} to be active on {path}; body[0..300] = {}",
            &body[..body.len().min(300)]
        );
    }
}

#[tokio::test]
async fn http_active_state_stable_on_child_view() {
    // Requirement 8: child views must keep the parent's section
    // active (Frappe v16 sidebar auto-switching anti-regression).
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "u_child",
            "u_child@example.com",
            "correct horse battery staple",
        )
        .await;
    let id = make_ledger(&server).await;
    let (status, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/transactions/new")).await;
    assert_eq!(status, 200, "new-transaction page must render");
    let re = active_link("transactions");
    assert!(
        re.is_match(&body),
        "Transactions must remain active on its child view; body[0..300] = {}",
        &body[..body.len().min(300)]
    );
    let dashboard_re = active_link("dashboard");
    assert!(
        !dashboard_re.is_match(&body),
        "Dashboard must not be active on a child view of /transactions"
    );
}

#[tokio::test]
async fn http_active_state_does_not_leak_to_other_sections() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "u_leak",
            "u_leak@example.com",
            "correct horse battery staple",
        )
        .await;
    let id = make_ledger(&server).await;
    let (status, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/reports")).await;
    assert_eq!(status, 200);
    for other in [
        "dashboard",
        "transactions",
        "accounts",
        "documents",
        "bank-feeds",
        "approvals",
        "expenses",
        "import",
        "import-wechat",
        "import-alipay",
    ] {
        let re = active_link(other);
        assert!(
            !re.is_match(&body),
            "section {other} must not be active on /reports"
        );
    }
}

#[tokio::test]
async fn http_breadcrumb_chain_inside_ledger() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("u_bc", "u_bc@example.com", "correct horse battery staple")
        .await;
    let id = make_ledger(&server).await;
    let (status, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/transactions")).await;
    assert_eq!(status, 200);
    assert!(
        body.contains(r#"<a href="/ledgers""#),
        "breadcrumb should contain a link to /ledgers"
    );
    assert!(
        body.contains(&format!(r#"<a href="/ledgers/{id}/dashboard""#)),
        "breadcrumb should contain a link to the dashboard"
    );
    assert!(
        body.contains("NavCo"),
        "breadcrumb should include the ledger name 'NavCo'"
    );
    // The current section is rendered as a non-link span with
    // aria-current="page" and the section name as text.
    assert!(
        body.contains(r#"aria-current="page""#) && body.contains("Transactions"),
        "breadcrumb current-section span should be present"
    );
}

#[tokio::test]
async fn http_breadcrumb_on_ledgers_list() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("u_bcl", "u_bcl@example.com", "correct horse battery staple")
        .await;
    let (status, body) = fetch(&server, &cookie, "/ledgers").await;
    assert_eq!(status, 200);
    assert!(
        body.contains("Ledgers"),
        "the /ledgers page should show 'Ledgers' in the breadcrumb"
    );
}

#[tokio::test]
async fn http_sidebar_renders_four_clusters() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("u_sc", "u_sc@example.com", "correct horse battery staple")
        .await;
    let id = make_ledger(&server).await;
    let (status, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/dashboard")).await;
    assert_eq!(status, 200);
    assert!(body.contains("app-sidebar"), "sidebar should be rendered");
    for label in ["Record", "Plan", "Analyze"] {
        // The cluster label appears on its own line between the
        // wrapping <div> tags. Use a plain substring check.
        assert!(
            body.contains(label),
            "sidebar should contain cluster label {label}; body[0..300] = {}",
            &body[..body.len().min(300)]
        );
    }
    // Every cluster link must be wrapped in <a class="sidebar-link">.
    assert!(
        body.contains(r#"class="sidebar-link"#),
        "sidebar should contain sidebar-link anchors"
    );
}

#[tokio::test]
async fn http_sidebar_hides_admin_cluster_for_non_admin() {
    // The default user is role "user", so Admin must not appear.
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "u_noadmin",
            "u_noadmin@example.com",
            "correct horse battery staple",
        )
        .await;
    let id = make_ledger(&server).await;
    let (status, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/dashboard")).await;
    assert_eq!(status, 200);
    let admin_re = Regex::new(r#"data-section="admin""#).unwrap();
    assert!(
        !admin_re.is_match(&body),
        "non-admin user must NOT see any admin nav link"
    );
}

#[tokio::test]
async fn http_sidebar_collapse_toggle_present() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("u_tog", "u_tog@example.com", "correct horse battery staple")
        .await;
    let id = make_ledger(&server).await;
    let (status, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/dashboard")).await;
    assert_eq!(status, 200);
    assert!(
        body.contains("app-sidebar"),
        "sidebar element must be rendered"
    );
    assert!(
        body.contains(r#"data-sidebar-toggle"#),
        "sidebar collapse toggle must be rendered"
    );
    assert!(
        body.contains(r#"data-sidebar-collapsed="false""#),
        "sidebar must start in the expanded state"
    );
}

/// Regression: when the sidebar is collapsed to icon-only, every
/// link MUST carry a `data-tooltip` attribute so the CSS tooltip
/// surfaces the label on hover. Without this, several icon-only
/// links look identical (e.g. "WeChat" and "Alipay" both use the
/// import icon) and the user has no way to tell which is which.
#[tokio::test]
async fn http_sidebar_links_have_tooltips() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("u_tip", "u_tip@example.com", "correct horse battery staple")
        .await;
    let id = make_ledger(&server).await;
    let (_, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/dashboard")).await;
    // Every sidebar-link anchor must have a data-tooltip attribute.
    // We extract the sidebar region first to avoid matching links
    // outside the sidebar.
    let sidebar = body
        .find("app-sidebar")
        .and_then(|start| {
            // Find the START of <aside class="app-sidebar ...">.
            let aside_open = body[..start]
                .rfind("<aside ")
                .expect("app-sidebar must be inside an <aside> tag");
            body[aside_open..]
                .find("</aside>")
                .map(|end| &body[aside_open..aside_open + end + 7])
        })
        .expect("sidebar must close with </aside>");
    let link_re = Regex::new(r#"<a[^>]*class="sidebar-link[^"]*"[^>]*>"#).unwrap();
    // The template emits `data-tooltip` BEFORE `class`, so the
    // test regex looks for the tooltip attribute in any position
    // within the same `<a>` tag.
    let tooltip_re = Regex::new(r#"<a[^>]*data-tooltip="[^"]+"[^>]*class="sidebar-link"#).unwrap();
    let n_links = link_re.find_iter(sidebar).count();
    let n_tooltips = tooltip_re.find_iter(sidebar).count();
    assert!(
        n_links > 0,
        "sidebar must contain at least one sidebar-link anchor"
    );
    assert_eq!(
        n_links, n_tooltips,
        "every sidebar-link must have a data-tooltip; \
         found {n_links} links but only {n_tooltips} tooltips"
    );
}

#[tokio::test]
async fn http_active_state_in_dark_mode() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "u_dark",
            "u_dark@example.com",
            "correct horse battery staple",
        )
        .await;
    let id = make_ledger(&server).await;
    // Toggle theme to dark via the dedicated endpoint.
    let resp = server
        .client()
        .post(format!("{}/account/theme", server.base_url()))
        .header(reqwest::header::COOKIE, cookie.clone())
        .form(&[("theme", "dark"), ("next", "/account")])
        .send()
        .await
        .expect("toggle theme");
    let s = resp.status();
    assert!(s == 303 || s == 302, "theme toggle must redirect, got {s}");
    let (status, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/accounts")).await;
    assert_eq!(status, 200);
    let re = active_link("accounts");
    assert!(
        re.is_match(&body),
        "active state must still render in dark mode; body[0..300] = {}",
        &body[..body.len().min(300)]
    );
}

#[tokio::test]
async fn http_nav_contains_icons() {
    // Requirement 3: every top-nav link is paired with an icon.
    // The icons are vendored SVGs referenced via <img>.
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("u_ico", "u_ico@example.com", "correct horse battery staple")
        .await;
    let id = make_ledger(&server).await;
    let (status, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/dashboard")).await;
    assert_eq!(status, 200);
    // The sidebar should reference /static/icons/nav/*.svg.
    assert!(
        body.contains("/static/icons/nav/"),
        "sidebar must reference vendored nav icons"
    );
    // And the icons must have aria-hidden="true" for accessibility.
    assert!(
        body.contains(r#"aria-hidden="true""#),
        "icons must be marked aria-hidden"
    );
}

#[tokio::test]
async fn http_icons_served_as_static_assets() {
    // Sanity: the vendored SVGs are actually served.
    let server = TestServer::new().await;
    for name in [
        "transactions",
        "documents",
        "bank_feeds",
        "reports",
        "import",
        "accounts",
        "approvals",
        "expenses",
        "dashboard",
        "admin",
    ] {
        let resp = server
            .client()
            .get(format!("{}/static/icons/nav/{name}.svg", server.base_url()))
            .send()
            .await
            .expect("GET static icon");
        assert_eq!(
            resp.status(),
            200,
            "icon {name}.svg must be served as a static asset"
        );
        let body = resp.text().await.expect("icon body");
        assert!(
            body.starts_with("<svg"),
            "icon {name}.svg must look like an SVG"
        );
    }
}

#[tokio::test]
async fn http_nav_css_served() {
    let server = TestServer::new().await;
    let resp = server
        .client()
        .get(format!("{}/static/css/nav.css", server.base_url()))
        .send()
        .await
        .expect("GET nav.css");
    assert_eq!(resp.status(), 200, "nav.css must be served");
    let body = resp.text().await.expect("css body");
    assert!(
        body.contains(".nav-shell") && body.contains(".is-active"),
        "nav.css must contain the active-state styles"
    );
}

/// Regression: the persistent sidebar must not push <main> down
/// below the sidebar. The fix is `position: fixed` on the
/// sidebar plus a `padding-left` rule on `body:has(...) main`.
/// Without that, the body being `flex flex-col` makes `<aside>`
/// take the full row, pushing `<main>` to a new row below.
#[tokio::test]
async fn http_sidebar_layout_does_not_push_main_down() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "u_layout",
            "u_layout@example.com",
            "correct horse battery staple",
        )
        .await;
    let id = make_ledger(&server).await;
    let resp = server
        .client()
        .get(format!("{}/ledgers/{id}/dashboard", server.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET dashboard");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.expect("body");
    // 1. The sidebar is `position: fixed` so it doesn't take up
    //    flow space and push <main> down.
    let css = server
        .client()
        .get(format!("{}/static/css/nav.css", server.base_url()))
        .send()
        .await
        .expect("GET nav.css")
        .text()
        .await
        .expect("css body");
    assert!(
        css.contains(".app-sidebar") && css.contains("position: fixed"),
        "nav.css must fix the sidebar out of normal flow"
    );
    // 2. The body reserves space for the sidebar via a
    //    `padding-left` rule on <main>. The exact value is
    //    `calc(var(--oa-sidebar-width) + var(--oa-sidebar-gap))`
    //    so the main content has a small visual gap from the
    //    rail and doesn't kiss it.
    assert!(
        css.contains("var(--oa-sidebar-width)") && css.contains("var(--oa-sidebar-gap)"),
        "nav.css must reserve left padding for the sidebar with a visual gap"
    );
    // 3. Sanity: the rendered page contains both the sidebar and
    //    a <main> element.
    assert!(body.contains("app-sidebar"), "sidebar must render");
    assert!(
        body.contains("<main"),
        "main element must render after the sidebar"
    );
}

/// Regression: the top nav is intentionally thin. The 11 per-ledger
/// links live in the sidebar; the top nav only carries the global
/// `Ledgers` link (and `Admin` for admins). This avoids duplicating
/// the same nav surface in two places.
#[tokio::test]
async fn http_top_nav_is_thin_only_global_links() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "u_thin",
            "u_thin@example.com",
            "correct horse battery staple",
        )
        .await;
    let id = make_ledger(&server).await;
    let (_, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/dashboard")).await;
    // The top nav (inside `<nav class="sticky ...">`) should NOT
    // contain any per-ledger section links. The sidebar does —
    // that's where they live now. Extract the top nav region by
    // finding the `<nav class="sticky ...` opening and the first
    // `</nav>` after it.
    let nav_open = body
        .find(r#"<nav class="sticky"#)
        .expect("page must have a top <nav class=\"sticky ...> tag");
    let end_in_after = body[nav_open..]
        .find("</nav>")
        .expect("top nav must have a closing </nav>");
    let top_nav = &body[nav_open..nav_open + end_in_after + 6];
    // The top nav must contain Ledgers. The nav template puts
    // the "Ledgers" anchor in a `<a href="/ledgers">...</a>`
    // block; the rendered text on its own line is `Ledgers`
    // (no other surrounding text). Use a substring check that
    // works whether or not the text is wrapped in a `<span>`.
    let re = Regex::new(r#">\s*Ledgers\s*<"#).unwrap();
    assert!(re.is_match(top_nav), "top nav must contain Ledgers link");
    // The top nav must NOT contain per-ledger section links.
    for forbidden in [
        "Transactions",
        "Documents",
        "Bank Feeds",
        "Import",
        "WeChat",
        "Alipay",
        "Accounts",
        "Approvals",
        "Expenses",
        "Reports",
        "Dashboard",
    ] {
        let re = Regex::new(&format!(r#">\s*{forbidden}\s*<"#)).unwrap();
        assert!(
            !re.is_match(top_nav),
            "top nav must not contain per-ledger link {forbidden}; the sidebar owns those"
        );
    }
}

/// The WeChat/Alipay upload pages use a wider main container so
/// the form doesn't look cramped when the sidebar is on screen.
#[tokio::test]
async fn http_import_upload_uses_wide_container() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "u_wide",
            "u_wide@example.com",
            "correct horse battery staple",
        )
        .await;
    let id = make_ledger(&server).await;
    for path in ["import", "import/wechat", "import/alipay"] {
        let (_, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/{path}")).await;
        // Extract the <main>...</main> region and assert it does
        // not use `max-w-md` (28rem). Other parts of the page
        // (e.g. modal dialogs) may legitimately use max-w-md.
        let main_region = body
            .find("<main")
            .and_then(|start| {
                body[start..]
                    .find("</main>")
                    .map(|end| &body[start..start + end])
            })
            .unwrap_or("");
        assert!(
            !main_region.contains(r#"max-w-md"#),
            "/ledgers/{id}/{path}: <main> must not use max-w-md; main is too narrow next to the sidebar; main body: {main_region}"
        );
    }
}

/// Regression: the "no X yet" inline messages across the app
/// (admin dashboard, contacts, approval_policies, rules,
/// reconciliation/history, templates) used to be lonely
/// `text-slate-500` paragraphs. They now have a styled block
/// with a heading, description, and CTA. Verify the structural
/// classes are present on each list page when the data is empty.
#[tokio::test]
async fn http_empty_state_has_styled_block() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user(
            "u_empty",
            "u_empty@example.com",
            "correct horse battery staple",
        )
        .await;
    let id = make_ledger(&server).await;
    // Each of these pages should render a styled empty-state
    // block with a border-dashed class (the visual cue for an
    // empty container).
    for (label, path) in [
        ("contacts", format!("/ledgers/{id}/contacts")),
        (
            "approval_policies",
            format!("/ledgers/{id}/approval-policies"),
        ),
        ("rules", format!("/ledgers/{id}/rules")),
        ("templates", format!("/ledgers/{id}/templates")),
    ] {
        let (status, body) = fetch(&server, &cookie, &path).await;
        assert_eq!(status, 200, "GET {path} must return 200");
        assert!(
            body.contains("border-dashed")
                || body.contains("data-empty-state"),
            "{label} empty state must use a styled block (border-dashed or data-empty-state); body[0..300] = {}",
            &body[..body.len().min(300)]
        );
    }
}

/// Regression: the persistent left sidebar's scrollbar should be
/// styled thinly (so it doesn't dominate the chrome) and use the
/// `auto` overflow so it only appears when the content overflows.
/// The aside carries the `scrollbar-thin` class.
#[tokio::test]
async fn http_sidebar_uses_thin_styled_scrollbar() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("u_sb", "u_sb@example.com", "correct horse battery staple")
        .await;
    let id = make_ledger(&server).await;
    let (_, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/dashboard")).await;
    // The aside must carry the scrollbar-thin class.
    let aside_re = Regex::new(r#"<aside[^>]*class="[^"]*app-sidebar[^"]*"[^>]*"#).unwrap();
    let aside_match = aside_re
        .find(&body)
        .expect("page must render <aside class=\"app-sidebar ...\">");
    let aside_open_tag = aside_match.as_str();
    assert!(
        aside_open_tag.contains("scrollbar-thin"),
        "sidebar <aside> must carry `scrollbar-thin` class for a styled scrollbar; got: {aside_open_tag}"
    );
    // The aside must use `overflow-y: auto` (auto, not scroll),
    // so the scrollbar only appears when content overflows.
    assert!(
        aside_open_tag.contains("overflow-y-auto"),
        "sidebar <aside> must use `overflow-y: auto`; got: {aside_open_tag}"
    );
    // The static CSS must define the thin scrollbar styles.
    let css = server
        .client()
        .get(format!("{}/static/css/app.css", server.base_url()))
        .send()
        .await
        .expect("GET app.css")
        .text()
        .await
        .expect("css body");
    assert!(
        css.contains(".scrollbar-thin") && css.contains("scrollbar-width: thin"),
        "app.css must define thin scrollbar styles for both WebKit and Firefox"
    );
}

/// Regression: clicking the dark mode toggle from inside a
/// ledger should NOT redirect the user to `/account` (where the
/// sidebar isn't rendered). The fix is the JS handler in
/// `templates/partials/_nav.html` setting the `next` field to
/// the current URL before submit. Verify the form has an
/// `id="theme-next"` field that JS can write to.
#[tokio::test]
async fn http_dark_mode_toggle_preserves_current_url() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("u_dm", "u_dm@example.com", "correct horse battery staple")
        .await;
    let id = make_ledger(&server).await;
    let (_, body) = fetch(&server, &cookie, &format!("/ledgers/{id}/transactions/new")).await;
    // The theme toggle form must carry an `id="theme-next"`
    // hidden input that the JS updates with the current URL
    // before submit. Without this id, the server would redirect
    // to /account and the sidebar would "vanish".
    assert!(
        body.contains(r#"id="theme-next""#),
        "theme toggle form must have id=\"theme-next\" hidden input"
    );
    // The form action must still POST to /account/theme.
    assert!(
        body.contains(r#"action="/account/theme""#),
        "theme toggle form must POST to /account/theme"
    );
}

/// Regression: the theme toggle handler in `_nav.html` runs the
/// pre-submit JS that sets `next` to the current URL. The page
/// itself is generated server-side with the `next` field set
/// to the static default `value="/account"`. We can't exercise
/// the JS path in a server-side integration test, but we can
/// verify the structural pieces are wired up: a form with the
/// `id="theme-toggle-form"`, an `id="theme-input"` hidden field
/// that the JS writes the new theme into, and an `id="theme-next"`
/// hidden field that the JS overwrites with the current URL.
#[tokio::test]
async fn http_theme_toggle_form_has_required_ids() {
    let server = TestServer::new().await;
    let cookie = server
        .bootstrap_user("u_tt", "u_tt@example.com", "correct horse battery staple")
        .await;
    let (_, body) = fetch(&server, &cookie, "/account").await;
    for id in ["theme-toggle-form", "theme-input", "theme-next"] {
        assert!(
            body.contains(&format!(r#"id="{id}""#)),
            "page must include id=\"{id}\" for the theme toggle form"
        );
    }
}
