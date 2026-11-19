//! Mark geometry (Phase 0 report §D):
//! - circles become area-preserving polygons with a bounded radial error;
//! - only marks that overlap (or nearly touch) a neighbour go through the
//!   Boolean union (`i_overlay`, integer µm, NonZero); isolated marks are
//!   emitted directly, which is exact and much faster;
//! - output is a `geo-types` `MultiPolygon<i32>` (decision D33).

use halftone_types::units::UM_PER_MM;
use halftone_types::{Mark, MultiPolygon, Polygon};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::{IntOverlayOptions, Overlay, ShapeType};
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::core::solver::Solver;
use i_overlay::i_float::int::point::IntPoint;
use i_overlay::i_shape::int::shape::{IntContour, IntShapes};
use rayon::prelude::*;
use std::collections::HashMap;
use std::f64::consts::PI;

/// Marks closer than this (edge to edge) are unioned rather than emitted separately.
const TOUCH_MARGIN_MM: f64 = 0.002;

#[derive(Clone, Copy, Debug)]
pub struct GeometryOptions {
    /// Maximum radial error of polygonal circles.
    pub tolerance_mm: f64,
    pub max_union_vertices: usize,
    /// Use i_overlay's internal threads (output is identical either way).
    pub multithread: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GeometryStats {
    pub marks: usize,
    pub isolated: usize,
    pub overlapping: usize,
    pub coords: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum GeometryError {
    #[error(
        "{vertices} vertices to union exceed the limit of {limit}; lower the LPI, raise the tolerance, or reduce the output size"
    )]
    TooComplex { vertices: usize, limit: usize },
    #[error("mark coordinates exceed the supported geometry range")]
    OutOfRange,
}

/// Segment count for a circle of radius `r` with sagitta error <= `tol`.
pub fn segments_for(r: f64, tol: f64) -> usize {
    if r <= tol {
        return 8;
    }
    ((PI / (1.0 - tol / r).acos()).ceil() as usize).clamp(8, 512)
}

/// Vertex radius of the area-preserving polygon for a circle of radius `r`
/// (slightly larger than `r`: the polygon's vertices lie outside the circle).
pub fn polygon_radius(r: f64, tol: f64) -> f64 {
    let n = segments_for(r, tol) as f64;
    r * (2.0 * PI / (n * (2.0 * PI / n).sin())).sqrt()
}

/// Polygon with the same area as the circle; vertices in integer µm, ordered
/// counter-clockwise in y-up terms (clockwise on a y-down page).
pub fn circle_contour(m: &Mark, tol_mm: f64) -> Result<IntContour<i32>, GeometryError> {
    let n = segments_for(m.radius_mm, tol_mm);
    let rp = polygon_radius(m.radius_mm, tol_mm) * UM_PER_MM;
    let (cx, cy) = (m.x_mm * UM_PER_MM, m.y_mm * UM_PER_MM);
    let limit = i32::MAX as f64 / 2.0;
    if !(cx.abs() + rp < limit && cy.abs() + rp < limit) {
        return Err(GeometryError::OutOfRange);
    }
    Ok((0..n)
        .map(|s| {
            let t = 2.0 * PI * s as f64 / n as f64;
            IntPoint::new((cx + rp * t.cos()).round() as i32, (cy + rp * t.sin()).round() as i32)
        })
        .collect())
}

/// For each mark: could its polygon overlap (or nearly touch) any of its 8
/// lattice neighbours' polygons? Uses polygon vertex radii, not circle radii.
pub fn overlap_flags(marks: &[Mark], pitch_mm: f64, tol_mm: f64) -> Vec<bool> {
    let by_cell: HashMap<(i32, i32), usize> = marks.iter().enumerate().map(|(k, m)| ((m.i, m.j), k)).collect();
    let outer: Vec<f64> = marks.par_iter().map(|m| polygon_radius(m.radius_mm, tol_mm)).collect();
    marks
        .par_iter()
        .zip(&outer)
        .map(|(m, &rm)| {
            (-1..=1).any(|dj: i32| {
                (-1..=1).any(|di: i32| {
                    if di == 0 && dj == 0 {
                        return false;
                    }
                    by_cell.get(&(m.i + di, m.j + dj)).is_some_and(|&o| {
                        let dist = pitch_mm * ((di * di + dj * dj) as f64).sqrt();
                        rm + outer[o] > dist - TOUCH_MARGIN_MM
                    })
                })
            })
        })
        .collect()
}

fn union(contours: &[IntContour<i32>], multithread: bool) -> IntShapes<i32> {
    let mut solver = Solver::AUTO;
    if !multithread {
        solver.multithreading = None;
    }
    let edges = contours.iter().map(Vec::len).sum();
    let mut ov = Overlay::<i32>::new_custom(edges, IntOverlayOptions::default(), solver);
    ov.add_source(contours, ShapeType::Subject);
    ov.overlay(OverlayRule::Subject, FillRule::NonZero)
}

fn ring(c: &[IntPoint<i32>]) -> geo_types::LineString<i32> {
    let mut coords: Vec<geo_types::Coord<i32>> = c.iter().map(|p| geo_types::coord! { x: p.x, y: p.y }).collect();
    if let Some(&first) = coords.first() {
        coords.push(first);
    }
    geo_types::LineString(coords)
}

fn to_polygon(shape: &[IntContour<i32>]) -> Polygon {
    Polygon::new(ring(&shape[0]), shape[1..].iter().map(|h| ring(h)).collect())
}

/// Build the ink artwork for a set of marks on a lattice of pitch `pitch_mm`.
pub fn build(
    marks: &[Mark],
    pitch_mm: f64,
    opts: &GeometryOptions,
) -> Result<(MultiPolygon, GeometryStats), GeometryError> {
    let flags = overlap_flags(marks, pitch_mm, opts.tolerance_mm);
    let contours: Vec<IntContour<i32>> = marks
        .par_iter()
        .map(|m| circle_contour(m, opts.tolerance_mm))
        .collect::<Result<_, _>>()?;
    let (mut overlapping, mut isolated) = (Vec::new(), Vec::new());
    for (c, f) in contours.into_iter().zip(&flags) {
        if *f { overlapping.push(c) } else { isolated.push(c) }
    }
    let vertices: usize = overlapping.iter().map(Vec::len).sum();
    if vertices > opts.max_union_vertices {
        return Err(GeometryError::TooComplex {
            vertices,
            limit: opts.max_union_vertices,
        });
    }
    let stats_counts = (marks.len(), isolated.len(), overlapping.len());
    let unioned = if overlapping.is_empty() {
        Vec::new()
    } else {
        union(&overlapping, opts.multithread)
    };
    drop(overlapping);
    let mut polys: Vec<Polygon> = unioned.par_iter().map(|s| to_polygon(s)).collect();
    polys.par_extend(isolated.par_iter().map(|c| Polygon::new(ring(c), vec![])));
    let mp = MultiPolygon::new(polys);
    let stats = GeometryStats {
        marks: stats_counts.0,
        isolated: stats_counts.1,
        overlapping: stats_counts.2,
        coords: halftone_types::geometry::coord_count(&mp),
    };
    Ok((mp, stats))
}
