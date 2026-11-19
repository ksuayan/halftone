//! SVG output (spec S§10, S§19).
//!
//! The page is `width`×`height` in millimetres; the `viewBox` is in integer
//! micrometres, so path data are compact integers with no float formatting.
//! - `<path id="halftone">`: one compound path, single fill, `evenodd`.
//! - `<path id="registration-marks">` (optional): stroked targets, same ink.
//!
//! Nothing else: no background, no rasters, no metadata.

mod preview;
mod validate;

use halftone_types::geometry::Coord;
use halftone_types::units::UM_PER_MM;
use halftone_types::{MultiPolygon, RegistrationTarget};
use std::fmt::Write;

pub use preview::preview_png;
pub use validate::{Expect, SvgReport, validate};

pub struct Document<'a> {
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    /// `#RRGGBB`.
    pub ink: &'a str,
    pub artwork: &'a MultiPolygon,
    pub targets: &'a [RegistrationTarget],
    pub mark_stroke_mm: f64,
}

fn um(mm: f64) -> i64 {
    (mm * UM_PER_MM).round() as i64
}

/// Millimetre value for attributes: at most 3 decimals, no trailing zeros.
fn mm_attr(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn ring_d(d: &mut String, pts: &[Coord<i32>]) {
    // geo-types rings repeat the first coordinate; `z` closes instead.
    let n = if pts.len() > 1 && pts[0] == pts[pts.len() - 1] {
        pts.len() - 1
    } else {
        pts.len()
    };
    if n < 3 {
        return;
    }
    let mut prev = pts[0];
    let _ = write!(d, "M{} {}l", prev.x, prev.y);
    for (k, p) in pts[1..n].iter().enumerate() {
        if k > 0 {
            d.push(' ');
        }
        let _ = write!(d, "{} {}", p.x - prev.x, p.y - prev.y);
        prev = *p;
    }
    d.push('z');
}

pub fn artwork_path_data(artwork: &MultiPolygon) -> String {
    let mut d = String::with_capacity(halftone_types::geometry::coord_count(artwork) * 9);
    for p in &artwork.0 {
        ring_d(&mut d, &p.exterior().0);
        for h in p.interiors() {
            ring_d(&mut d, &h.0);
        }
    }
    d
}

pub fn targets_path_data(targets: &[RegistrationTarget]) -> String {
    let mut d = String::new();
    for t in targets {
        let (x, y, r, a) = (um(t.cx_mm), um(t.cy_mm), um(t.radius_mm), um(t.arm_mm));
        let _ = write!(
            d,
            "M{} {y}a{r} {r} 0 1 0 {} 0a{r} {r} 0 1 0 {} 0zM{} {y}h{}M{x} {}v{}",
            x - r,
            2 * r,
            -2 * r,
            x - a,
            2 * a,
            y - a,
            2 * a
        );
    }
    d
}

pub fn write(doc: &Document) -> String {
    let (w, h) = (um(doc.page_width_mm), um(doc.page_height_mm));
    let mut s = String::new();
    let _ = writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {w} {h}\">",
        mm_attr(doc.page_width_mm),
        mm_attr(doc.page_height_mm)
    );
    let _ = writeln!(
        s,
        "<path id=\"halftone\" fill=\"{}\" fill-rule=\"evenodd\" d=\"{}\"/>",
        doc.ink,
        artwork_path_data(doc.artwork)
    );
    if !doc.targets.is_empty() {
        let _ = writeln!(
            s,
            "<path id=\"registration-marks\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" d=\"{}\"/>",
            doc.ink,
            um(doc.mark_stroke_mm),
            targets_path_data(doc.targets)
        );
    }
    s.push_str("</svg>\n");
    s
}
