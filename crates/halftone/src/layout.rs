//! Page layout. The SVG page is the paper minus the printer margin on each side,
//! so printing with Fit to Page scales by about 1.0. Registration targets sit
//! in a band inside the page; the photo is contain-fitted inside the band.

use halftone_core::lattice::Rect;
use halftone_types::RegistrationTarget;
use halftone_types::config::{Output, RectMm};

/// Width of the band that holds registration targets (per side).
pub const MARK_BAND_MM: f64 = 12.0;
pub const TARGET_RADIUS_MM: f64 = 3.0;
pub const TARGET_ARM_MM: f64 = 5.0;
/// Target stroke: 0.5 pt, about 4 printer dots at 600 dpi.
pub const MARK_STROKE_MM: f64 = 0.5 * 25.4 / 72.0;

#[derive(Clone, Debug, PartialEq)]
pub struct PageLayout {
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub image: Rect,
    pub targets: Vec<RegistrationTarget>,
}

#[derive(Debug, PartialEq)]
pub struct LayoutError(pub String);

pub fn compute(output: &Output, source_px: (u32, u32)) -> Result<PageLayout, LayoutError> {
    let pw = output.paper_width_mm - 2.0 * output.printer_margin_mm;
    let ph = output.paper_height_mm - 2.0 * output.printer_margin_mm;
    let band = if output.registration_marks { MARK_BAND_MM } else { 0.0 };
    let (aw, ah) = (pw - 2.0 * band, ph - 2.0 * band);
    if aw <= 1.0 || ah <= 1.0 {
        return Err(LayoutError(format!(
            "no room for the image: page {pw:.1} x {ph:.1} mm minus {band} mm mark band per side; use larger paper, a smaller printer margin, or --no-registration-marks"
        )));
    }
    let (sw, sh) = (source_px.0 as f64, source_px.1 as f64);
    let scale = (aw / sw).min(ah / sh);
    let (iw, ih) = (sw * scale, sh * scale);
    let (x0, y0) = (band + (aw - iw) / 2.0, band + (ah - ih) / 2.0);
    let image = Rect {
        x0,
        y0,
        x1: x0 + iw,
        y1: y0 + ih,
    };
    let target = |cx, cy| RegistrationTarget {
        cx_mm: cx,
        cy_mm: cy,
        radius_mm: TARGET_RADIUS_MM,
        arm_mm: TARGET_ARM_MM,
    };
    let targets = if output.registration_marks {
        let m = band / 2.0;
        vec![
            target(pw / 2.0, m),
            target(pw / 2.0, ph - m),
            target(m, ph / 2.0),
            target(pw - m, ph / 2.0),
        ]
    } else {
        Vec::new()
    };
    Ok(PageLayout {
        page_width_mm: pw,
        page_height_mm: ph,
        image,
        targets,
    })
}

pub fn rect_mm(r: &Rect) -> RectMm {
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    RectMm {
        x_mm: round(r.x0),
        y_mm: round(r.y0),
        width_mm: round(r.width()),
        height_mm: round(r.height()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halftone_types::Units;
    use halftone_types::config::Fit;

    fn output(marks: bool) -> Output {
        Output {
            paper: Some("a3".into()),
            paper_width_mm: 297.0,
            paper_height_mm: 420.0,
            printer_margin_mm: 6.35,
            fit: Fit::Contain,
            ink: "#000000".into(),
            registration_marks: marks,
            display_units: Units::Mm,
        }
    }

    #[test]
    fn contain_fit_centres_and_preserves_aspect() {
        let l = compute(&output(true), (3000, 2000)).unwrap();
        assert!((l.page_width_mm - 284.3).abs() < 1e-9);
        let (w, h) = (l.image.width(), l.image.height());
        assert!((w / h - 1.5).abs() < 1e-9);
        assert!((w - (284.3 - 24.0)).abs() < 1e-9); // width-limited
        assert!((l.image.y0 - (l.page_height_mm - l.image.y1)).abs() < 1e-9); // centred
        assert_eq!(l.targets.len(), 4);
        for t in &l.targets {
            assert!(t.cx_mm - t.arm_mm >= 0.0 && t.cy_mm - t.arm_mm >= 0.0);
        }
    }

    #[test]
    fn no_marks_uses_the_band_for_image() {
        let l = compute(&output(false), (1000, 1000)).unwrap();
        assert!(l.targets.is_empty());
        assert!((l.image.width() - 284.3).abs() < 1e-9);
    }

    #[test]
    fn too_small_page_is_an_error() {
        let mut o = output(true);
        o.paper_width_mm = 30.0;
        assert!(compute(&o, (100, 100)).is_err());
    }
}
