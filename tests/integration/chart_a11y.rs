//! Chart accessibility (`u13-ux-a11y-mobile` finding A2).
//!
//! Verifies that the SVG chart helpers emit an accessible
//! container with `role="img"`, an `aria-label` summary, and a
//! visually-hidden `<table>` carrying the same data. The HTML
//! response from the dashboard route must also surface the
//! markup so screen-reader users see the data table.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openaccounting::charts::{render_donut, render_line, DonutSegment, LineSeries};

#[test]
fn render_line_is_accessible() {
    let svg = render_line(
        400,
        160,
        vec!["Jan".into(), "Feb".into(), "Mar".into()],
        vec![LineSeries {
            name: "Income".into(),
            color: "#0ea5e9".into(),
            values: vec![10.0, 20.0, 30.0],
        }],
    );
    assert!(
        svg.contains(r#"data-chart="line""#),
        "line chart must carry data-chart=line; got: {}",
        &svg[..svg.len().min(300)]
    );
    assert!(
        svg.contains(r#"role="img""#),
        "line chart must expose role=img"
    );
    assert!(
        svg.contains("aria-label="),
        "line chart must include an aria-label summary"
    );
    assert!(
        svg.contains("<table>"),
        "line chart must ship a hidden data table"
    );
    assert!(
        svg.contains("Income"),
        "line chart summary must name the series"
    );
}

#[test]
fn render_donut_is_accessible() {
    let svg = render_donut(
        200,
        vec![
            DonutSegment {
                label: "Marketing".into(),
                value: 60.0,
                color: "#0ea5e9".into(),
            },
            DonutSegment {
                label: "Rent".into(),
                value: 40.0,
                color: "#f43f5e".into(),
            },
        ],
        "100",
    );
    assert!(
        svg.contains(r#"data-chart="donut""#),
        "donut must carry data-chart=donut; got: {}",
        &svg[..svg.len().min(300)]
    );
    assert!(svg.contains(r#"role="img""#), "donut must expose role=img");
    assert!(
        svg.contains("aria-label="),
        "donut must include an aria-label summary"
    );
    assert!(
        svg.contains("<table>"),
        "donut must ship a hidden data table"
    );
    assert!(
        svg.contains("Marketing"),
        "donut summary must name the dominant slice"
    );
}

#[test]
fn empty_line_chart_is_still_accessible() {
    let svg = render_line(
        200,
        80,
        vec![],
        vec![LineSeries {
            name: "Empty".into(),
            color: "#0ea5e9".into(),
            values: vec![],
        }],
    );
    assert!(
        svg.contains(r#"role="img""#),
        "empty chart still needs role=img"
    );
    assert!(
        svg.contains("Empty line chart"),
        "empty chart summary must explain the state"
    );
}

#[test]
fn empty_donut_chart_is_still_accessible() {
    let svg = render_donut(120, vec![], "0");
    // An empty donut returns the wrapper so the role/aria-label
    // still apply — even zero data should not be a silent
    // visual element.
    assert!(
        svg.contains("Empty donut chart"),
        "empty donut must report its state via the summary"
    );
}
