//! Experiment C: coverage <-> diameter for a periodic square lattice of equal disks.
//!
//! The Voronoi cell of a square lattice point is the square cell centered on it,
//! so the union coverage of the whole lattice equals the area of one disk clipped
//! to its own cell. That gives a closed form for every diameter in [0, sqrt 2].

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
    // Four circular segments beyond the cell edges at distance h = 0.5.
    let h = 0.5;
    let segment = r * r * (h / r).acos() - h * (r * r - h * h).sqrt();
    disk - 4.0 * segment
}

/// Isolated-circle approximation d = sqrt(4C/pi) (valid only while d <= 1).
pub fn isolated_diameter(c: f64) -> f64 {
    (4.0 * c / PI).sqrt()
}

/// Inverse of [`coverage_of_diameter`] by bisection; result in cell pitches.
pub fn diameter_for_coverage(c: f64) -> f64 {
    if c <= 0.0 {
        return 0.0;
    }
    if c >= 1.0 {
        return SQRT_2;
    }
    let (mut lo, mut hi) = (0.0, SQRT_2);
    for _ in 0..64 {
        let mid = 0.5 * (lo + hi);
        if coverage_of_diameter(mid) < c {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Brute-force coverage by supersampling one cell against the disks of the cell
/// and all eight neighbours (validation only; does not assume the Voronoi argument).
pub fn coverage_numeric(d: f64, samples: usize) -> f64 {
    let r2 = (d / 2.0) * (d / 2.0);
    let mut inside = 0usize;
    for y in 0..samples {
        let py = (y as f64 + 0.5) / samples as f64 - 0.5;
        for x in 0..samples {
            let px = (x as f64 + 0.5) / samples as f64 - 0.5;
            let hit = (-1..=1).any(|j| {
                (-1..=1).any(|i| {
                    let (dx, dy) = (px - i as f64, py - j as f64);
                    dx * dx + dy * dy <= r2
                })
            });
            if hit {
                inside += 1;
            }
        }
    }
    inside as f64 / (samples * samples) as f64
}
