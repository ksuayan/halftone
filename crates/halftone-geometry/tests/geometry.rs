use geo::{Area, MapCoords, Validation};
use halftone_geometry::{GeometryOptions, build, circle_contour, overlap_flags, segments_for};
use halftone_types::Mark;
use halftone_types::geometry::area_mm2;
use std::f64::consts::PI;

fn opts() -> GeometryOptions {
    GeometryOptions {
        tolerance_mm: 0.010,
        max_union_vertices: 10_000_000,
        multithread: true,
    }
}

/// Square lattice of marks; radius from `r(i, j)`.
fn grid(n: i32, pitch: f64, r: impl Fn(i32, i32) -> f64) -> Vec<Mark> {
    (0..n)
        .flat_map(|j| (0..n).map(move |i| (i, j)))
        .map(|(i, j)| Mark {
            i,
            j,
            x_mm: 5.0 + i as f64 * pitch,
            y_mm: 5.0 + j as f64 * pitch,
            radius_mm: r(i, j),
        })
        .collect()
}

#[test]
fn polygon_area_equals_circle_area() {
    for r in [0.065, 0.2, 0.8] {
        let m = Mark {
            i: 0,
            j: 0,
            x_mm: 10.0,
            y_mm: 10.0,
            radius_mm: r,
        };
        let c = circle_contour(&m, 0.010).unwrap();
        let mut s = 0.0;
        for k in 0..c.len() {
            let (p, q) = (c[k], c[(k + 1) % c.len()]);
            s += p.x as f64 * q.y as f64 - q.x as f64 * p.y as f64;
        }
        let area_mm2 = (s / 2.0).abs() / 1e6;
        // Integer rounding of vertices bounds the error.
        assert!((area_mm2 - PI * r * r).abs() / (PI * r * r) < 2e-3, "r={r}");
    }
    assert!(segments_for(0.8, 0.010) > segments_for(0.1, 0.010));
}

#[test]
fn isolated_marks_bypass_union_and_overlaps_merge() {
    let pitch = 1.0;
    let sparse = grid(10, pitch, |_, _| 0.3);
    assert!(overlap_flags(&sparse, pitch, 0.010).iter().all(|f| !f));
    let (mp, stats) = build(&sparse, pitch, &opts()).unwrap();
    assert_eq!((stats.isolated, stats.overlapping, mp.0.len()), (100, 0, 100));

    // d = 1.2 pitch: neighbours overlap; one connected region with holes at cell corners.
    let dense = grid(10, pitch, |_, _| 0.6);
    let (mp, stats) = build(&dense, pitch, &opts()).unwrap();
    assert_eq!(stats.overlapping, 100);
    assert_eq!(mp.0.len(), 1);
    assert_eq!(mp.0[0].interiors().len(), 81);
}

#[test]
fn output_is_valid_and_area_is_consistent() {
    let pitch = 0.8;
    // Mixed: isolated highlights, touching midtones, merged shadows.
    let marks = grid(40, pitch, |i, j| 0.05 + 0.5 * pitch * ((i + j) as f64 / 78.0));
    let (mp, _) = build(&marks, pitch, &opts()).unwrap();
    let as_f64 = mp.map_coords(|c| geo::coord! { x: c.x as f64, y: c.y as f64 });
    assert!(as_f64.is_valid(), "{:?}", as_f64.validation_errors().first());
    let geo_area = as_f64.unsigned_area() / 1e6;
    assert!((geo_area - area_mm2(&mp)).abs() < 1e-6);
    // Never more ink than the sum of the disks; overlaps only reduce it.
    let disks: f64 = marks.iter().map(|m| PI * m.radius_mm * m.radius_mm).sum();
    assert!(area_mm2(&mp) <= disks * 1.002);
}

#[test]
fn deterministic_across_thread_modes() {
    let marks = grid(30, 0.6, |i, j| 0.2 + 0.25 * ((i * 7 + j * 3) % 10) as f64 / 10.0);
    let (a, _) = build(&marks, 0.6, &opts()).unwrap();
    let (b, _) = build(&marks, 0.6, &opts()).unwrap();
    let (c, _) = build(
        &marks,
        0.6,
        &GeometryOptions {
            multithread: false,
            ..opts()
        },
    )
    .unwrap();
    assert_eq!(a, b);
    assert_eq!(a, c);
}

#[test]
fn complexity_limit_is_enforced() {
    let dense = grid(20, 1.0, |_, _| 0.7);
    let err = build(
        &dense,
        1.0,
        &GeometryOptions {
            max_union_vertices: 100,
            ..opts()
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("exceed the limit"));
}
