//! Experiment E: printable test chart (600 dpi laser -> transparency film).
//!   exp_e <letter|tabloid|a3> <out.svg>
//! Sections: tonal wedges at 22 / 24 / 27.5 LPI, shadow ramps, minimum-dot ramp in
//! device pixels, angle variants, a scale bar, and registration targets.
//! Labels use SVG <text> (chart only; product output stays text-free).
use halftone_prototypes::coverage::diameter_for_coverage;
use halftone_prototypes::geom::*;
use halftone_prototypes::lattice::{Lattice, Rect};
use std::fmt::Write;

const TOL: f64 = 0.010;
const DEVICE_PX_MM: f64 = 25.4 / 600.0;
const PRINTER_MARGIN: f64 = 6.35; // assumed non-printable border (to be measured)
const MARK_BAND: f64 = 12.0; // band outside the chart area holding the targets
const HAIRLINE: f64 = 0.5 * 25.4 / 72.0; // 0.5 pt

struct Chart {
    polys: Vec<Vec<i_overlay::i_float::int::point::IntPoint<i32>>>,
    text: String,
}

impl Chart {
    fn label(&mut self, x: f64, y: f64, size: f64, s: &str) {
        let _ = writeln!(
            self.text,
            "<text x=\"{:.0}\" y=\"{:.0}\" font-family=\"Helvetica, Arial, sans-serif\" font-size=\"{:.0}\" fill=\"#000000\">{s}</text>",
            x * UM_PER_MM,
            y * UM_PER_MM,
            size * UM_PER_MM
        );
    }

    /// Uniform-coverage patch; whole cells only, lattice phase at the patch centre.
    fn patch(&mut self, rect: Rect, lpi: f64, angle: f64, coverage: f64) {
        let lat = Lattice { angle_deg: angle, ..Lattice::from_lpi(lpi, angle, rect) };
        let d = diameter_for_coverage(coverage) * lat.pitch;
        if d <= 0.0 {
            return;
        }
        let set = lat.whole_cells(rect, lat.pitch * std::f64::consts::FRAC_1_SQRT_2);
        for c in &set.cells {
            self.polys.push(circle_polygon(&Mark { i: c.i, j: c.j, x: c.x, y: c.y, r: d / 2.0 }, TOL));
        }
    }

    /// Isolated dots of a fixed diameter (minimum-dot survival test).
    fn dot_patch(&mut self, rect: Rect, pitch: f64, diameter: f64) {
        let lat = Lattice { pitch, angle_deg: 22.5, origin: ((rect.x0 + rect.x1) / 2.0, (rect.y0 + rect.y1) / 2.0) };
        for c in &lat.whole_cells(rect, pitch).cells {
            self.polys.push(circle_polygon(&Mark { i: c.i, j: c.j, x: c.x, y: c.y, r: diameter / 2.0 }, TOL / 4.0));
        }
    }
}

/// Standard registration target: circle plus crosshair, stroked.
fn target(d: &mut String, cx: f64, cy: f64) {
    let (r, arm) = (3.0 * UM_PER_MM, 6.0 * UM_PER_MM);
    let (x, y) = (cx * UM_PER_MM, cy * UM_PER_MM);
    let _ = write!(
        d,
        "M{:.0} {:.0}a{r:.0} {r:.0} 0 1 0 {:.0} 0a{r:.0} {r:.0} 0 1 0 {:.0} 0zM{:.0} {:.0}h{:.0}M{:.0} {:.0}v{:.0}",
        x - r, y, 2.0 * r, -2.0 * r, x - arm, y, 2.0 * arm, x, y - arm, 2.0 * arm
    );
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (pw, ph) = match a[1].as_str() {
        "letter" => (215.9, 279.4),
        "tabloid" => (279.4, 431.8),
        "a3" => (297.0, 420.0),
        _ => panic!("letter | tabloid | a3"),
    };
    let inset = PRINTER_MARGIN + MARK_BAND;
    let area = Rect { x0: inset, y0: inset, x1: pw - inset, y1: ph - inset };
    let aw = area.x1 - area.x0;
    let mut ch = Chart { polys: Vec::new(), text: String::new() };
    let mut y = area.y0 + 4.0;
    ch.label(area.x0, y, 3.0, "Halftone test chart - print with Fit to Page; measure the scale bar and target spacing");
    y += 3.0;

    // Patch size: 11 per row, at most 14 mm.
    let ps = (aw / 11.0 - 1.0).min(if ph < 300.0 { 11.0 } else { 17.0 }).floor();
    let step = ps + 1.0;
    let row = |ch: &mut Chart, y: f64, lpi: f64, covs: &[f64]| {
        for (k, &c) in covs.iter().enumerate() {
            let x0 = area.x0 + k as f64 * step;
            ch.patch(Rect { x0, y0: y, x1: x0 + ps, y1: y + ps }, lpi, 22.5, c);
            ch.label(x0, y + ps + 2.6, 1.8, &format!("{:.0}%", c * 100.0));
        }
    };

    // 1. Tonal wedges: 0..100 % target coverage in 5 % steps, two rows per LPI.
    let lo: Vec<f64> = (0..=10).map(|k| k as f64 * 0.05).collect();
    let hi: Vec<f64> = (11..=20).map(|k| k as f64 * 0.05).collect();
    for lpi in [22.0, 24.0, 27.5] {
        y += 4.5;
        ch.label(area.x0, y, 2.5, &format!("{lpi} LPI @ 22.5 deg - target coverage 0..100 % (5 % steps)"));
        y += 1.2;
        row(&mut ch, y, lpi, &lo);
        y += ps + 4.0;
        row(&mut ch, y, lpi, &hi);
        y += ps + 3.0;
    }

    // 2. Shadow ramps: 80..100 % in 2 % steps (where do holes fill in?).
    let sh: Vec<f64> = (0..=10).map(|k| 0.80 + 0.02 * k as f64).collect();
    for lpi in [22.0, 24.0, 27.5] {
        y += 4.5;
        ch.label(area.x0, y, 2.5, &format!("Shadows {lpi} LPI: 80..100 % (2 % steps)"));
        y += 1.2;
        row(&mut ch, y, lpi, &sh);
        y += ps + 3.0;
    }

    // 3. Minimum dot: isolated dots 1..8 device px (600 dpi) on a 1.155 mm pitch.
    y += 5.0;
    ch.label(area.x0, y, 2.5, "Minimum dot: diameters 1..8 device px at 600 dpi (42..339 um)");
    y += 1.5;
    let mw = (aw / 8.0).floor();
    for k in 1..=8 {
        let x0 = area.x0 + (k - 1) as f64 * mw;
        ch.dot_patch(Rect { x0, y0: y, x1: x0 + mw - 1.0, y1: y + ps }, 25.4 / 22.0, k as f64 * DEVICE_PX_MM);
        ch.label(x0, y + ps + 2.6, 1.8, &format!("{k} px / {:.0} um", k as f64 * DEVICE_PX_MM * 1000.0));
    }
    y += ps + 4.0;

    // 4. Angle variants at 24 LPI: 50 % and 90 %.
    y += 5.0;
    ch.label(area.x0, y, 2.5, "Angles at 24 LPI (50 % and 90 %): 0 / 15 / 22.5 / 30 / 45 deg");
    y += 1.5;
    let gw = (aw / 10.0).floor().min(ps + 4.0);
    for (n, angle) in [0.0, 15.0, 22.5, 30.0, 45.0].into_iter().enumerate() {
        for (m, cov) in [0.5, 0.9].into_iter().enumerate() {
            let x0 = area.x0 + (n * 2 + m) as f64 * gw;
            ch.patch(Rect { x0, y0: y, x1: x0 + gw - 0.8, y1: y + gw }, 24.0, angle, cov);
        }
        ch.label(area.x0 + (n * 2) as f64 * gw, y + gw + 2.5, 1.8, &format!("{angle} deg"));
    }
    y += gw + 4.0;

    // 5. Scale bar: 100 mm with 10 mm ticks, and 4 in with 1 in ticks.
    let mut marks = String::new();
    y += 4.0;
    let bar = |m: &mut String, y: f64, len: f64, step: f64| {
        let _ = write!(m, "M{:.0} {:.0}h{:.0}", area.x0 * UM_PER_MM, y * UM_PER_MM, len * UM_PER_MM);
        let mut t = 0.0;
        while t <= len + 1e-9 {
            let _ = write!(m, "M{:.0} {:.0}v{:.0}", (area.x0 + t) * UM_PER_MM, (y - 1.5) * UM_PER_MM, 3.0 * UM_PER_MM);
            t += step;
        }
    };
    bar(&mut marks, y, 100.0, 10.0);
    ch.label(area.x0 + 102.0, y + 1.0, 2.0, "100 mm");
    bar(&mut marks, y + 6.0, 101.6, 25.4);
    ch.label(area.x0 + 104.0, y + 7.0, 2.0, "4 in");
    assert!(y + 10.0 < area.y1, "chart content overflows the page");

    // Registration targets centred on each edge, in the mark band.
    let mid = MARK_BAND / 2.0 + PRINTER_MARGIN;
    for (cx, cy) in [(pw / 2.0, mid), (pw / 2.0, ph - mid), (mid, ph / 2.0), (pw - mid, ph / 2.0)] {
        target(&mut marks, cx, cy);
    }
    let spacing_x = pw - 2.0 * mid;
    let spacing_y = ph - 2.0 * mid;
    ch.label(area.x0, area.y1 - 1.0, 2.0, &format!("Target spacing at 100 %: {spacing_x:.1} mm (left-right), {spacing_y:.1} mm (top-bottom)"));

    let u = union(&ch.polys, true);
    let mut d = String::new();
    write_polygons(&mut d, &u);
    let extra = format!(
        "<path id=\"registration-marks\" fill=\"none\" stroke=\"#000000\" stroke-width=\"{:.0}\" d=\"{marks}\"/>\n{}",
        HAIRLINE * UM_PER_MM,
        ch.text
    );
    let svg = svg_document(pw, ph, &d, &extra);
    std::fs::write(&a[2], &svg).unwrap();
    if let Some(png) = a.get(3) {
        let (_, pm) = raster_ink_fraction(&svg, 150.0 / 25.4, Rect { x0: 0.0, y0: 0.0, x1: pw, y1: ph });
        // Flatten onto white: transparent previews show as black in many viewers.
        use resvg::tiny_skia::{Color, Pixmap, PixmapPaint, Transform};
        let mut flat = Pixmap::new(pm.width(), pm.height()).unwrap();
        flat.fill(Color::WHITE);
        flat.draw_pixmap(0, 0, pm.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        flat.save_png(png).unwrap();
    }
    println!("{} ({:.1} x {:.1} mm): {} dots, {:.1} MB", a[2], pw, ph, ch.polys.len(), svg.len() as f64 / 1e6);
}
