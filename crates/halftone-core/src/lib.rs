//! Halftone screening: tone response, periodic coverage, rotated lattice and
//! mark placement. Image-agnostic: luminance is read through a closure.

pub mod coverage;
pub mod lattice;
pub mod tone;

use halftone_types::Mark;
use lattice::{CellSet, Lattice, Rect};
use rayon::prelude::*;
use std::f64::consts::FRAC_1_SQRT_2;
use tone::ToneCurve;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("about {estimated} screen cells exceed the limit of {limit}; lower the LPI or the output size")]
    TooManyCells { estimated: u64, limit: usize },
}

#[derive(Clone, Copy, Debug)]
pub struct ScreenSpec {
    pub pitch_mm: f64,
    pub angle_deg: f64,
    pub min_dot_mm: f64,
    /// Whole-cell rule: a cell is kept only if a disk of this radius around its
    /// centre fits inside the image. Use the outer extent of the largest mark as
    /// drawn (at least pitch / sqrt 2; more for polygons whose vertices lie
    /// outside the circle).
    pub keep_radius_mm: f64,
}

pub struct Placement {
    pub lattice: Lattice,
    pub cells: CellSet,
    pub marks: Vec<Mark>,
}

/// Lay a whole-cell lattice over `image` and size one mark per cell.
/// `luminance_at(x_mm, y_mm)` returns linear luminance at a page position.
pub fn place_marks(
    image: &Rect,
    screen: &ScreenSpec,
    tone: &ToneCurve,
    max_cells: usize,
    luminance_at: impl Fn(f64, f64) -> f64 + Sync,
) -> Result<Placement, CoreError> {
    let lattice = Lattice::new(screen.pitch_mm, screen.angle_deg, image);
    let estimated = lattice.estimate_cells(image).ceil() as u64;
    if estimated > max_cells as u64 {
        return Err(CoreError::TooManyCells {
            estimated,
            limit: max_cells,
        });
    }
    // Largest possible mark (100 % coverage) has diameter sqrt(2) * pitch.
    let cells = lattice.whole_cells(image, screen.keep_radius_mm.max(screen.pitch_mm * FRAC_1_SQRT_2));
    let marks = cells
        .cells
        .par_iter()
        .filter_map(|c| {
            let coverage = tone.coverage(luminance_at(c.x, c.y));
            let d = coverage::diameter_for_coverage(coverage) * screen.pitch_mm;
            (d > 0.0 && d >= screen.min_dot_mm).then_some(Mark {
                i: c.i,
                j: c.j,
                x_mm: c.x,
                y_mm: c.y,
                radius_mm: d / 2.0,
            })
        })
        .collect();
    Ok(Placement { lattice, cells, marks })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone() -> ToneCurve {
        ToneCurve {
            invert: false,
            black_point: 0.0,
            white_point: 1.0,
            gamma: 1.0,
            max_coverage: 1.0,
        }
    }

    #[test]
    fn uniform_field_gives_requested_coverage() {
        let rect = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 60.0,
            y1: 60.0,
        };
        let spec = ScreenSpec {
            pitch_mm: 1.0,
            angle_deg: 22.5,
            min_dot_mm: 0.0,
            keep_radius_mm: FRAC_1_SQRT_2,
        };
        let p = place_marks(&rect, &spec, &tone(), 1_000_000, |_, _| 0.6).unwrap();
        assert_eq!(p.marks.len(), p.cells.cells.len());
        let c = coverage::coverage_of_diameter(2.0 * p.marks[0].radius_mm / spec.pitch_mm);
        assert!((c - 0.4).abs() < 1e-12);
    }

    #[test]
    fn small_marks_are_suppressed_and_white_is_empty() {
        let rect = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 30.0,
            y1: 30.0,
        };
        let spec = ScreenSpec {
            pitch_mm: 1.0,
            angle_deg: 0.0,
            min_dot_mm: 0.3,
            keep_radius_mm: FRAC_1_SQRT_2,
        };
        let p = place_marks(
            &rect,
            &spec,
            &tone(),
            1_000_000,
            |x, _| if x < 15.0 { 1.0 } else { 0.99 },
        )
        .unwrap();
        // 1 % coverage -> d ≈ 0.113 mm < 0.3 mm: suppressed; pure white: no mark.
        assert!(p.marks.is_empty());
        let err = place_marks(&rect, &spec, &tone(), 100, |_, _| 0.5).err().unwrap();
        assert!(matches!(err, CoreError::TooManyCells { .. }));
    }
}
