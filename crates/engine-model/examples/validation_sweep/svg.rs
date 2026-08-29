//! A minimum line-chart writer, so the validation document has no plotting
//! dependency.
//!
//! gnuplot and matplotlib both do this better. Neither is used, because the
//! validation document is the artifact that answers whether the model is real, and it
//! has to regenerate on any machine that has cargo and nothing else. A chart is a
//! polyline, two axes and some tick labels; that is not worth a toolchain.
//!
//! Every figure in the document is a line chart, including the ones normally drawn as
//! filled maps. A brake specific fuel consumption island plotted as one curve per
//! engine speed carries the same information as a contour plot and costs no extra
//! code to draw.

use std::fmt::Write as _;

/// One plotted curve.
pub struct Series {
    /// Legend text.
    pub label: String,
    /// Points in data coordinates.
    pub points: Vec<(f64, f64)>,
    /// Draw dashed rather than solid.
    pub dashed: bool,
}

impl Series {
    /// A solid series.
    pub fn solid(label: &str, points: Vec<(f64, f64)>) -> Self {
        Self {
            label: label.into(),
            points,
            dashed: false,
        }
    }
    /// A dashed series, for references and limits rather than model output.
    pub fn dashed(label: &str, points: Vec<(f64, f64)>) -> Self {
        Self {
            label: label.into(),
            points,
            dashed: true,
        }
    }
}

const W: f64 = 760.0;
const H: f64 = 420.0;
const L: f64 = 78.0;
const R: f64 = 18.0;
const T: f64 = 34.0;
const B: f64 = 50.0;

const INK: &str = "#F0F0F2";
const DIM: &str = "#8E8E96";
const GRID: &str = "#232326";
const BACK: &str = "#0A0A0B";
/// Stroke colours in drawing order. Luminance carries the ordering; hue only
/// separates curves that would otherwise overlap.
const STROKES: [&str; 6] = [
    "#F0F0F2", "#FF6B35", "#8E8E96", "#6E6E76", "#B8B8C0", "#55555C",
];

fn nice_ticks(lo: f64, hi: f64) -> Vec<f64> {
    if hi.is_nan() || lo.is_nan() || hi <= lo {
        return vec![lo];
    }
    let raw = (hi - lo) / 5.0;
    let magnitude = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .map(|m| m * magnitude)
        .find(|s| *s >= raw)
        .unwrap_or(magnitude * 10.0);
    let first = (lo / step).ceil() * step;
    let mut out = Vec::new();
    let mut v = first;
    while v <= hi + step * 1e-9 {
        out.push(v);
        v += step;
    }
    out
}

fn label(v: f64) -> String {
    let a = v.abs();
    if a >= 10.0 {
        format!("{v:.0}")
    } else if a >= 1.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

/// Render a line chart to standalone SVG.
///
/// Ranges are taken from the data unless overridden, and are padded so a curve never
/// runs along an axis. Passing an explicit y range is how a plot is made to contain a
/// reference line that lies outside the data.
#[must_use]
pub fn chart(
    title: &str,
    x_axis: &str,
    y_axis: &str,
    series: &[Series],
    y_range: Option<(f64, f64)>,
) -> String {
    let all = || series.iter().flat_map(|s| s.points.iter());
    let (mut x0, mut x1) = (f64::MAX, f64::MIN);
    let (mut y0, mut y1) = (f64::MAX, f64::MIN);
    for (x, y) in all() {
        x0 = x0.min(*x);
        x1 = x1.max(*x);
        y0 = y0.min(*y);
        y1 = y1.max(*y);
    }
    if let Some((a, b)) = y_range {
        y0 = a;
        y1 = b;
    } else {
        let pad = (y1 - y0) * 0.08;
        y0 -= pad;
        y1 += pad;
    }
    if x1 <= x0 {
        x1 = x0 + 1.0;
    }
    if y1 <= y0 {
        y1 = y0 + 1.0;
    }

    let sx = |x: f64| L + (x - x0) / (x1 - x0) * (W - L - R);
    let sy = |y: f64| H - B - (y - y0) / (y1 - y0) * (H - T - B);

    let mut s = String::new();
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W:.0} {H:.0}" width="{W:.0}" height="{H:.0}" font-family="ui-monospace,monospace">
<rect width="{W:.0}" height="{H:.0}" fill="{BACK}"/>
<text x="{L:.0}" y="20" fill="{INK}" font-size="14">{title}</text>"#
    );

    for t in nice_ticks(y0, y1) {
        let y = sy(t);
        let _ = write!(
            s,
            r#"<line x1="{L:.1}" y1="{y:.1}" x2="{:.1}" y2="{y:.1}" stroke="{GRID}"/>
<text x="{:.1}" y="{:.1}" fill="{DIM}" font-size="11" text-anchor="end">{}</text>"#,
            W - R,
            L - 8.0,
            y + 4.0,
            label(t)
        );
    }
    for t in nice_ticks(x0, x1) {
        let x = sx(t);
        let _ = write!(
            s,
            r#"<line x1="{x:.1}" y1="{:.1}" x2="{x:.1}" y2="{:.1}" stroke="{GRID}"/>
<text x="{x:.1}" y="{:.1}" fill="{DIM}" font-size="11" text-anchor="middle">{}</text>"#,
            T,
            H - B,
            H - B + 18.0,
            label(t)
        );
    }

    let _ = write!(
        s,
        r#"<text x="{:.0}" y="{:.0}" fill="{DIM}" font-size="11" text-anchor="middle">{x_axis}</text>
<text x="14" y="{:.0}" fill="{DIM}" font-size="11" text-anchor="middle" transform="rotate(-90 14 {:.0})">{y_axis}</text>"#,
        (L + W - R) / 2.0,
        H - 8.0,
        (T + H - B) / 2.0,
        (T + H - B) / 2.0
    );

    for (i, series) in series.iter().enumerate() {
        let stroke = STROKES[i % STROKES.len()];
        let dash = if series.dashed {
            r#" stroke-dasharray="6 4""#
        } else {
            ""
        };
        let points: String = series
            .points
            .iter()
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .map(|(x, y)| format!("{:.2},{:.2} ", sx(*x), sy(*y)))
            .collect();
        let _ = write!(
            s,
            r#"<polyline points="{points}" fill="none" stroke="{stroke}" stroke-width="2"{dash}/>"#
        );
        let ly = T + 16.0 + i as f64 * 16.0;
        let _ = write!(
            s,
            r#"<line x1="{:.0}" y1="{ly:.0}" x2="{:.0}" y2="{ly:.0}" stroke="{stroke}" stroke-width="2"{dash}/>
<text x="{:.0}" y="{:.0}" fill="{DIM}" font-size="11">{}</text>"#,
            W - R - 150.0,
            W - R - 126.0,
            W - R - 120.0,
            ly + 4.0,
            series.label
        );
    }

    s.push_str("</svg>\n");
    s
}
