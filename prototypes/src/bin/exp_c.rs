//! Experiment C: validate the closed-form periodic coverage and its inverse.
use halftone_prototypes::coverage::*;
use std::time::Instant;

fn main() {
    println!("d/p    analytic   numeric(9-disk)  |err|");
    let mut max_err: f64 = 0.0;
    for k in 0..=30 {
        let d = std::f64::consts::SQRT_2 * k as f64 / 30.0;
        let a = coverage_of_diameter(d);
        let n = coverage_numeric(d, 2000);
        max_err = max_err.max((a - n).abs());
        if k % 3 == 0 {
            println!("{d:.4}  {a:.6}   {n:.6}         {:.2e}", (a - n).abs());
        }
    }
    println!("max |analytic - numeric| over 31 diameters: {max_err:.2e}");

    // Monotonicity and inverse round-trip.
    let mut prev = -1.0;
    let mut worst_rt: f64 = 0.0;
    for k in 0..=100_000 {
        let c = k as f64 / 100_000.0;
        let d = diameter_for_coverage(c);
        assert!(d >= prev, "inverse not monotonic at c={c}");
        prev = d;
        worst_rt = worst_rt.max((coverage_of_diameter(d) - c).abs());
    }
    println!("inverse monotonic over 100001 samples; worst round-trip error {worst_rt:.2e}");

    // How wrong is the isolated-circle formula once circles overlap?
    println!("\ncoverage  periodic d/p  isolated d/p  actual coverage with isolated d");
    for c in [0.05, 0.25, 0.5, 0.7854, 0.85, 0.9, 0.95, 0.99] {
        let d = diameter_for_coverage(c);
        let di = isolated_diameter(c);
        println!("{c:.4}    {d:.4}        {di:.4}        {:.4}", coverage_of_diameter(di));
    }

    let t = Instant::now();
    let mut acc = 0.0;
    for k in 0..400_000 {
        acc += diameter_for_coverage(k as f64 / 400_000.0);
    }
    println!("\n400k inversions (A3 at 45 LPI is ~390k cells): {:?} (checksum {acc:.3})", t.elapsed());
}
