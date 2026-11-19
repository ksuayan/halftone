//! Geometry interchange (decision D33): `geo-types` with integer micrometre
//! coordinates.
//!
//! Conventions:
//! - 1 unit = 1 µm; x right, y down (SVG orientation).
//! - Rings are closed (`geo-types` repeats the first coordinate).
//! - Because y points down, exterior rings appear clockwise on screen and holes
//!   counter-clockwise. `geo` algorithms that rely on y-up orientation must
//!   account for this; area magnitudes are unaffected.

pub use geo_types::{Coord, LineString, MultiPolygon as GeoMultiPolygon, Polygon as GeoPolygon};

/// Ink artwork in integer micrometres.
pub type MultiPolygon = geo_types::MultiPolygon<i32>;
pub type Polygon = geo_types::Polygon<i32>;

/// One halftone mark before geometry construction (millimetres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    /// Lattice indices (stable identity; neighbours differ by ±1).
    pub i: i32,
    pub j: i32,
    pub x_mm: f64,
    pub y_mm: f64,
    pub radius_mm: f64,
}

/// Printer's registration target: circle plus crosshair, drawn as a stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegistrationTarget {
    pub cx_mm: f64,
    pub cy_mm: f64,
    pub radius_mm: f64,
    pub arm_mm: f64,
}

/// Ink area in mm² (exteriors minus holes).
pub fn area_mm2(mp: &MultiPolygon) -> f64 {
    fn ring(ls: &LineString<i32>) -> f64 {
        let pts = &ls.0;
        let mut s = 0.0;
        for w in pts.windows(2) {
            s += w[0].x as f64 * w[1].y as f64 - w[1].x as f64 * w[0].y as f64;
        }
        (s / 2.0).abs()
    }
    let um2: f64 =
        mp.0.iter()
            .map(|p| ring(p.exterior()) - p.interiors().iter().map(ring).sum::<f64>())
            .sum();
    um2 / 1e6
}

/// Total coordinates, counting the repeated closing coordinate of each ring.
pub fn coord_count(mp: &MultiPolygon) -> usize {
    mp.0.iter()
        .map(|p| p.exterior().0.len() + p.interiors().iter().map(|r| r.0.len()).sum::<usize>())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_with_hole_area() {
        let sq = |s: i32, o: i32| LineString::from(vec![(o, o), (o + s, o), (o + s, o + s), (o, o + s), (o, o)]);
        let p = Polygon::new(sq(10_000, 0), vec![sq(2_000, 1_000)]);
        let mp = MultiPolygon::new(vec![p]);
        assert!((area_mm2(&mp) - (100.0 - 4.0)).abs() < 1e-9);
        assert_eq!(coord_count(&mp), 10);
    }
}
