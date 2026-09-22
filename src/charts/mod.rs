//! Server-rendered SVG chart helpers (line + donut).
//!
//! We pre-compute every coordinate in Rust and render a simple SVG
//! template that just does field substitution. This avoids Askama's
//! template-language arithmetic and keeps the templates tiny.
//!
//! Every chart wraps its output in an accessible
//! `<figure role="img">` with an `aria-label` summary plus a
//! visually-hidden `<table>` carrying the same data. Screen
//! readers therefore receive both a one-sentence summary and
//! the underlying numbers without any new endpoints
//! (`u13-ux-a11y-mobile`, finding A2).

use askama::Template;

#[derive(Template)]
#[template(path = "partials/charts/line.html", escape = "none")]
struct LineSvg<'a> {
    width: u32,
    height: u32,
    points_attr: String,
    circles: String,
    y_axis: String,
    x_axis: String,
    legend: String,
    _phantom: std::marker::PhantomData<&'a ()>,
}

#[derive(Clone, Debug)]
pub struct LineSeries {
    pub name: String,
    pub color: String,
    pub values: Vec<f64>,
}

#[derive(Template)]
#[template(path = "partials/charts/donut.html", escape = "none")]
struct DonutSvg {
    cx: f64,
    cy: f64,
    r_inner: f64,
    paths: String,
    center_label: String,
    legend: String,
}

#[derive(Clone, Debug)]
pub struct DonutSegment {
    pub label: String,
    pub value: f64,
    pub color: String,
}

fn nice_max(v: f64) -> f64 {
    if v <= 0.0 {
        return 100.0;
    }
    let exp = v.log10().floor();
    let base = 10f64.powf(exp);
    let m = v / base;
    let factor = if m <= 1.0 {
        1.0
    } else if m <= 2.0 {
        2.0
    } else if m <= 5.0 {
        5.0
    } else {
        10.0
    };
    factor * base
}

pub fn render_line(
    width: u32,
    height: u32,
    x_labels: Vec<String>,
    series: Vec<LineSeries>,
) -> String {
    let pad_l: f64 = 50.0;
    let pad_r: f64 = 16.0;
    let pad_t: f64 = 16.0;
    let pad_b: f64 = 30.0;
    let plot_w = width as f64 - pad_l - pad_r;
    let plot_h = height as f64 - pad_t - pad_b;

    let all_values: Vec<f64> = series
        .iter()
        .flat_map(|s| s.values.iter())
        .copied()
        .collect();
    let raw_max = all_values.iter().cloned().fold(0.0_f64, f64::max);
    let y_max = nice_max(raw_max);

    // Y gridlines + labels (4 ticks).
    let mut y_axis = String::new();
    for i in 0..=4 {
        let ratio = i as f64 / 4.0;
        let y = pad_t + plot_h * (1.0 - ratio);
        y_axis.push_str(&format!(
            r##"<line x1="{}" y1="{:.2}" x2="{}" y2="{:.2}" stroke="#e2e8f0" stroke-width="1" />"##,
            pad_l,
            y,
            width as f64 - pad_r,
            y,
        ));
        y_axis.push_str(&format!(
            r##"<text x="{}" y="{:.2}" text-anchor="end" font-size="10" fill="#64748b">{:.0}</text>"##,
            pad_l - 6.0, y + 4.0, y_max * ratio,
        ));
    }

    // X labels.
    let mut x_axis = String::new();
    let n = x_labels.len();
    if n > 0 {
        for (i, label) in x_labels.iter().enumerate() {
            let ratio = if n > 1 {
                i as f64 / (n as f64 - 1.0)
            } else {
                0.0
            };
            let x = pad_l + plot_w * ratio;
            x_axis.push_str(&format!(
                r##"<text x="{:.2}" y="{:.2}" text-anchor="middle" font-size="10" fill="#64748b">{}</text>"##,
                x, height as f64 - pad_b + 14.0, html_escape(label),
            ));
        }
    }

    // Series lines and points.
    let mut points_attr = String::new();
    let mut circles = String::new();
    for s in &series {
        if n == 0 {
            continue;
        }
        let mut pts = String::new();
        for (i, &v) in s.values.iter().enumerate() {
            let ratio = if n > 1 {
                i as f64 / (n as f64 - 1.0)
            } else {
                0.0
            };
            let x = pad_l + plot_w * ratio;
            let y_norm = if y_max > 0.0 {
                (v / y_max).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let y = pad_t + plot_h * (1.0 - y_norm);
            if !pts.is_empty() {
                pts.push(' ');
            }
            pts.push_str(&format!("{:.2},{:.2}", x, y));
            circles.push_str(&format!(
                r##"<circle cx="{:.2}" cy="{:.2}" r="3" fill="{}" />"##,
                x, y, s.color,
            ));
        }
        points_attr.push_str(&format!(
            r##"<polyline fill="none" stroke="{}" stroke-width="2" points="{}" />"##,
            s.color, pts,
        ));
    }

    // Legend.
    let mut legend = String::new();
    for (i, s) in series.iter().enumerate() {
        let lx = pad_l + 6.0 + i as f64 * 90.0;
        legend.push_str(&format!(
            r##"<rect x="{:.2}" y="2" width="10" height="10" fill="{}" />"##,
            lx, s.color,
        ));
        legend.push_str(&format!(
            r##"<text x="{:.2}" y="10" font-size="10" fill="#334155">{}</text>"##,
            lx + 14.0,
            html_escape(&s.name),
        ));
    }

    let tmpl = LineSvg {
        width,
        height,
        points_attr,
        circles,
        y_axis,
        x_axis,
        legend,
        _phantom: std::marker::PhantomData,
    };
    let svg = tmpl.render().unwrap_or_default();

    let summary = build_line_summary(&x_labels, &series, y_max);
    let data_table = build_line_data_table(&x_labels, &series);
    wrap_figure(&svg, &summary, &data_table)
}

fn build_line_summary(x_labels: &[String], series: &[LineSeries], y_max: f64) -> String {
    if series.is_empty() || x_labels.is_empty() {
        return "Empty line chart.".to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    for s in series {
        let total: f64 = s.values.iter().sum();
        let peak = s
            .values
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, v)| {
                format!(
                    "peak {:.0} at {}",
                    v,
                    x_labels.get(i).cloned().unwrap_or_default()
                )
            })
            .unwrap_or_else(|| "no peak".to_string());
        parts.push(format!("{}: total {:.0}, {}", s.name, total, peak));
    }
    format!(
        "Line chart of {} over {} buckets; y-axis up to {:.0}. {}",
        series
            .iter()
            .map(|s| s.name.clone())
            .collect::<Vec<_>>()
            .join(" and "),
        x_labels.len(),
        y_max,
        parts.join("; ")
    )
}

fn build_line_data_table(x_labels: &[String], series: &[LineSeries]) -> String {
    if x_labels.is_empty() {
        return String::new();
    }
    let mut head = String::from("<thead><tr><th scope=\"col\">Bucket</th>");
    for s in series {
        head.push_str(&format!("<th scope=\"col\">{}</th>", html_escape(&s.name)));
    }
    head.push_str("</tr></thead>");
    let mut body = String::from("<tbody>");
    for (i, label) in x_labels.iter().enumerate() {
        body.push_str(&format!(
            "<tr><th scope=\"row\">{}</th>",
            html_escape(label)
        ));
        for s in series {
            let v = s.values.get(i).copied().unwrap_or(0.0);
            body.push_str(&format!("<td>{:.0}</td>", v));
        }
        body.push_str("</tr>");
    }
    body.push_str("</tbody>");
    format!("<table>{}</table>", head + &body)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn donut_slice(cx: f64, cy: f64, r_outer: f64, r_inner: f64, a1: f64, a2: f64) -> String {
    let large = if (a2 - a1) > std::f64::consts::PI {
        1
    } else {
        0
    };
    let p1 = (cx + r_outer * a1.cos(), cy + r_outer * a1.sin());
    let p2 = (cx + r_outer * a2.cos(), cy + r_outer * a2.sin());
    let p3 = (cx + r_inner * a2.cos(), cy + r_inner * a2.sin());
    let p4 = (cx + r_inner * a1.cos(), cy + r_inner * a1.sin());
    format!(
        "M {x1:.2} {y1:.2} A {ro:.2} {ro:.2} 0 {large} 1 {x2:.2} {y2:.2} L {x3:.2} {y3:.2} A {ri:.2} {ri:.2} 0 {large} 0 {x4:.2} {y4:.2} Z",
        x1 = p1.0, y1 = p1.1, ro = r_outer,
        x2 = p2.0, y2 = p2.1,
        x3 = p3.0, y3 = p3.1, ri = r_inner,
        x4 = p4.0, y4 = p4.1,
        large = large,
    )
}

pub fn render_donut(size: u32, segments: Vec<DonutSegment>, center_label: &str) -> String {
    let cx = size as f64 / 2.0;
    let cy = size as f64 / 2.0;
    let r_outer = (size as f64 / 2.0) - 8.0;
    let r_inner = r_outer * 0.6;

    let total: f64 = segments.iter().map(|s| s.value).sum();
    if total <= 0.0 {
        // Still emit an accessible container so a screen-reader
        // user learns why the chart is empty (`u13-ux-a11y-mobile`
        // finding A2 — every chart needs a summary).
        let summary = build_donut_summary(&segments, total, center_label);
        return wrap_figure("", &summary, "");
    }

    let mut paths = String::new();
    let mut legend = String::new();
    let mut angle = -std::f64::consts::FRAC_PI_2;
    for s in &segments {
        let frac = s.value / total;
        let a1 = angle;
        let a2 = angle + frac * std::f64::consts::TAU;
        paths.push_str(&format!(
            r##"<path d="{}" fill="{}" />"##,
            donut_slice(cx, cy, r_outer, r_inner, a1, a2),
            s.color,
        ));
        legend.push_str(&format!(
            r##"<div class="flex items-center gap-2"><span class="inline-block w-3 h-3 rounded-sm" style="background:{}"></span><span class="truncate flex-1">{}</span><span class="tabular text-slate-500">{:.0}%</span></div>"##,
            s.color, html_escape(&s.label), frac * 100.0,
        ));
        angle = a2;
    }

    let tmpl = DonutSvg {
        cx,
        cy,
        r_inner,
        paths,
        center_label: center_label.to_string(),
        legend,
    };
    let svg = tmpl.render().unwrap_or_default();

    let summary = build_donut_summary(&segments, total, center_label);
    let data_table = build_donut_data_table(&segments, total);
    wrap_figure(&svg, &summary, &data_table)
}

fn build_donut_summary(segments: &[DonutSegment], total: f64, center_label: &str) -> String {
    if segments.is_empty() || total <= 0.0 {
        return "Empty donut chart.".to_string();
    }
    let dominant = segments
        .iter()
        .max_by(|a, b| {
            a.value
                .partial_cmp(&b.value)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|s| format!("{} {:.0}%", s.label, (s.value / total) * 100.0))
        .unwrap_or_else(|| "no dominant slice".to_string());
    format!(
        "Donut chart, total {}; dominant {}.",
        center_label, dominant
    )
}

fn build_donut_data_table(segments: &[DonutSegment], total: f64) -> String {
    if segments.is_empty() {
        return String::new();
    }
    let head =
        String::from("<thead><tr><th scope=\"col\">Category</th><th scope=\"col\">Value</th><th scope=\"col\">Percent</th></tr></thead>");
    let mut body = String::from("<tbody>");
    for s in segments {
        let pct = if total > 0.0 {
            (s.value / total) * 100.0
        } else {
            0.0
        };
        body.push_str(&format!(
            "<tr><th scope=\"row\">{}</th><td>{:.0}</td><td>{:.2}%</td></tr>",
            html_escape(&s.label),
            s.value,
            pct
        ));
    }
    body.push_str("</tbody>");
    format!("<table>{}</table>", head + &body)
}

/// Wrap the raw SVG in an accessible `<figure>` so that screen
/// readers receive both a one-line summary and the full data
/// table. `data-chart="line|donut"` lets E2E tests target each
/// kind unambiguously.
fn wrap_figure(svg: &str, summary: &str, data_table: &str) -> String {
    let kind = if data_table.contains("Bucket") {
        "line"
    } else {
        "donut"
    };
    format!(
        "<figure class=\"oa-chart\" data-chart=\"{kind}\" role=\"img\" aria-label=\"{label}\">\
{svg}\
<figcaption class=\"sr-only\">{caption}</figcaption>\
<div class=\"sr-only\">{table}</div>\
</figure>",
        kind = kind,
        label = html_escape(summary),
        svg = svg,
        caption = html_escape(summary),
        table = data_table,
    )
}
