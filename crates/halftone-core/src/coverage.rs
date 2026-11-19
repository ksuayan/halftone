//! Coverage <-> diameter for a periodic square lattice of equal disks
//! (Phase 0 report §C).
//!
//! The Voronoi cell of a square-lattice point is its own square cell, so the
//! union coverage of the whole lattice equals one disk clipped to its cell. That
//! gives a closed form for every diameter in [0, sqrt 2] cell pitches; beyond
//! d = 1 it accounts for overlap, which the isolated-circle formula does not.

use std::f64::consts::{FRAC_1_SQRT_2, PI, SQRT_2};

/// Coverage of the periodic disk lattice; `d` is the diameter in cell pitches.
pub fn coverage_of_diameter(d: f64) -> f64 {
    let r = d / 2.0;
    if r <= 0.0 {
        return 0.0;
    }
    if r >= FRAC_1_SQRT_2 {
        return 1.0;
    }
    let disk = PI * r * r;
    if r <= 0.5 {
        return disk;
    }
    // Remove the four circular segments beyond the cell edges (distance h = 1/2).
    let h = 0.5;
    let segment = r * r * (h / r).acos() - h * (r * r - h * h).sqrt();
    disk - 4.0 * segment
}

/// Diameter (in cell pitches) whose periodic coverage equals `c`. Exact to
/// floating-point precision; monotonic.
pub fn diameter_for_coverage(c: f64) -> f64 {
    if c.is_nan() || c <= 0.0 {
        return 0.0;
    }
    if c >= 1.0 {
        return SQRT_2;
    }
    let (mut lo, mut hi) = (0.0, SQRT_2);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if coverage_of_diameter(mid) < c {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Brute force against the cell's disk and all eight neighbours.
    fn coverage_numeric(d: f64, n: usize) -> f64 {
        let r2 = d * d / 4.0;
        let mut inside = 0usize;
        for y in 0..n {
            let py = (y as f64 + 0.5) / n as f64 - 0.5;
            for x in 0..n {
                let px = (x as f64 + 0.5) / n as f64 - 0.5;
                let hit = (-1..=1).any(|j| (-1..=1).any(|i| (px - i as f64).powi(2) + (py - j as f64).powi(2) <= r2));
                inside += hit as usize;
            }
        }
        inside as f64 / (n * n) as f64
    }

    #[test]
    fn closed_form_matches_brute_force() {
        for k in 0..=20 {
            let d = SQRT_2 * k as f64 / 20.0;
            assert!(
                (coverage_of_diameter(d) - coverage_numeric(d, 600)).abs() < 2e-4,
                "d={d}"
            );
        }
    }

    #[test]
    fn boundaries() {
        assert_eq!(coverage_of_diameter(0.0), 0.0);
        assert!((coverage_of_diameter(1.0) - PI / 4.0).abs() < 1e-15);
        assert_eq!(coverage_of_diameter(SQRT_2), 1.0);
        assert_eq!(diameter_for_coverage(0.0), 0.0);
        assert_eq!(diameter_for_coverage(1.0), SQRT_2);
        assert_eq!(diameter_for_coverage(f64::NAN), 0.0);
    }

    proptest! {
        #[test]
        fn inverse_round_trips(c in 0.0f64..1.0) {
            prop_assert!((coverage_of_diameter(diameter_for_coverage(c)) - c).abs() < 1e-12);
        }

        #[test]
        fn inverse_is_monotonic(a in 0.0f64..1.0, b in 0.0f64..1.0) {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            prop_assert!(diameter_for_coverage(lo) <= diameter_for_coverage(hi));
        }
    }
}
