//! Experiment D: circle geometry, Boolean union, hybrid arcs, SVG output.
//!
//! Geometry is integer micrometres (1 unit = 1 um) from polygon construction
//! onward: deterministic, and ample for a 600 dpi device (1 px = 42.3 um).

use crate::lattice::CellSet;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::{IntOverlayOptions, Overlay, ShapeType};
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::core::solver::Solver;
use i_overlay::i_float::int::point::IntPoint;
use i_overlay::i_shape::int::shape::{IntContour, IntShapes};
use rayon::prelude::*;
use std::f64::consts::PI;
use std::fmt::Write;

pub const UM_PER_MM: f64 = 1000.0;

#[derive(Clone, Copy, Debug)]
pub struct Mark {
    pub i: i32,
    pub j: i32,
    pub x: f64, // mm
    pub y: f64, // mm
    pub r: f64, // mm
}

/// Segment count from the allowed radial (sagitta) error, clamped.
pub fn segments_for(r: f64, tol: f64) -> usize {
    if r <= tol {
        return 8;
    }
    let n = (PI / (1.0 - tol / r).acos()).ceil() as usize;
    n.clamp(8, 512)
}

/// Area-preserving polygon: vertex radius scaled so polygon area = circle area.
pub fn circle_polygon(m: &Mark, tol: f64) -> IntContour<i32> {
    let n = segments_for(m.r, tol);
    let k = (2.0 * PI / (n as f64 * (2.0 * PI / n as f64).sin())).sqrt();
    let rp = m.r * k * UM_PER_MM;
    let (cx, cy) = (m.x * UM_PER_MM, m.y * UM_PER_MM);
    (0..n)
        .map(|s| {
            let t = 2.0 * PI * s as f64 / n as f64;
            IntPoint::new((cx + rp * t.cos()).round() as i32, (cy + rp * t.sin()).round() as i32)
        })
        .collect()
}

pub fn union(contours: &[IntContour<i32>], multithread: bool) -> IntShapes<i32> {
    let mut solver = Solver::AUTO;
    if !multithread {
        solver.multithreading = None;
    }
    let edges: usize = contours.iter().map(|c| c.len()).sum();
    let mut ov = Overlay::<i32>::new_custom(edges, IntOverlayOptions::default(), solver);
    ov.add_source(contours, ShapeType::Subject);
    ov.overlay(OverlayRule::Subject, FillRule::NonZero)
}

/// Spatial tiles unioned in parallel, merged pairwise in a fixed tree order.
pub fn union_tiled(marks: &[Mark], tol: f64, tile_cells: i32) -> IntShapes<i32> {
    let mut tiles: std::collections::BTreeMap<(i32, i32), Vec<IntContour<i32>>> = Default::default();
    for m in marks {
        let key = (m.j.div_euclid(tile_cells), m.i.div_euclid(tile_cells));
        tiles.entry(key).or_default().push(circle_polygon(m, tol));
    }
    let parts: Vec<IntShapes<i32>> = tiles.into_values().collect::<Vec<_>>().par_iter().map(|c| union(c, false)).collect();
    merge_tree(parts)
}

fn merge_tree(mut parts: Vec<IntShapes<i32>>) -> IntShapes<i32> {
    while parts.len() > 1 {
        parts = parts
            .par_chunks(2)
            .map(|pair| {
                let contours: Vec<IntContour<i32>> = pair.iter().flatten().flatten().cloned().collect();
                if pair.len() == 1 { pair[0].clone() } else { union(&contours, false) }
            })
            .collect();
    }
    parts.pop().unwrap_or_default()
}

/// Marks that overlap (or nearly touch) any neighbour; the rest are isolated.
pub fn overlapping_flags(marks: &[Mark], set: &CellSet, cell_of_mark: &[usize], pitch: f64, margin: f64) -> Vec<bool> {
    let mut mark_of_cell = vec![usize::MAX; set.cells.len()];
    for (m, &c) in cell_of_mark.iter().enumerate() {
        mark_of_cell[c] = m;
    }
    marks
        .par_iter()
        .map(|m| {
            for dj in -1..=1 {
                for di in -1..=1 {
                    if di == 0 && dj == 0 {
                        continue;
                    }
                    let Some(c) = set.get(m.i + di, m.j + dj) else { continue };
                    let other = mark_of_cell[c];
                    if other == usize::MAX {
                        continue;
                    }
                    let dist = pitch * ((di * di + dj * dj) as f64).sqrt();
                    if m.r + marks[other].r > dist - margin {
                        return true;
                    }
                }
            }
            false
        })
        .collect()
}

/// Signed-area sum of union output in mm^2 (outer and hole orientations cancel correctly).
pub fn area_mm2(shapes: &IntShapes<i32>) -> f64 {
    let mut a = 0.0;
    for shape in shapes {
        for c in shape {
            let mut s = 0.0;
            for k in 0..c.len() {
                let (p, q) = (c[k], c[(k + 1) % c.len()]);
                s += p.x as f64 * q.y as f64 - q.x as f64 * p.y as f64;
            }
            a += s / 2.0;
        }
    }
    a.abs() / (UM_PER_MM * UM_PER_MM)
}

pub fn vertex_count(shapes: &IntShapes<i32>) -> usize {
    shapes.iter().flatten().map(|c| c.len()).sum()
}

/// Append polygon contours as relative path data.
pub fn write_polygons(d: &mut String, shapes: &IntShapes<i32>) {
    for c in shapes.iter().flatten() {
        let mut prev = c[0];
        let _ = write!(d, "M{} {}l", prev.x, prev.y);
        for p in &c[1..] {
            let _ = write!(d, "{} {} ", p.x - prev.x, p.y - prev.y);
            prev = *p;
        }
        d.pop();
        d.push('z');
    }
}

/// Append exact circles as two relative arcs each.
pub fn write_arcs(d: &mut String, marks: &[&Mark]) {
    for m in marks {
        let r = (m.r * UM_PER_MM).round() as i64;
        let (cx, cy) = ((m.x * UM_PER_MM).round() as i64, (m.y * UM_PER_MM).round() as i64);
        let _ = write!(d, "M{} {}a{r} {r} 0 1 0 {} 0a{r} {r} 0 1 0 {} 0z", cx - r, cy, 2 * r, -2 * r);
    }
}

/// Minimal positive-artwork SVG: physical size in mm, viewBox in um.
pub fn svg_document(width_mm: f64, height_mm: f64, path_d: &str, extra: &str) -> String {
    let (w, h) = ((width_mm * UM_PER_MM).round() as i64, (height_mm * UM_PER_MM).round() as i64);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width_mm}mm\" height=\"{height_mm}mm\" viewBox=\"0 0 {w} {h}\">\n\
         <path id=\"halftone\" fill=\"#000000\" fill-rule=\"evenodd\" d=\"{path_d}\"/>\n{extra}</svg>\n"
    )
}

/// Rasterize an SVG with resvg and return the ink fraction inside `rect_px`.
pub fn raster_ink_fraction(svg: &str, px_per_mm: f64, rect_mm: crate::lattice::Rect) -> (f64, resvg::tiny_skia::Pixmap) {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).expect("parse svg");
    let size = tree.size();
    // tree.size() is in CSS px (96/in); scale to the requested resolution.
    let s = px_per_mm * 25.4 / 96.0;
    let (w, h) = ((size.width() as f64 * s).ceil() as u32, (size.height() as f64 * s).ceil() as u32);
    let mut pm = resvg::tiny_skia::Pixmap::new(w, h).expect("pixmap");
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s as f32, s as f32), &mut pm.as_mut());
    let (x0, y0) = ((rect_mm.x0 * px_per_mm) as u32, (rect_mm.y0 * px_per_mm) as u32);
    let (x1, y1) = (((rect_mm.x1 * px_per_mm) as u32).min(w), ((rect_mm.y1 * px_per_mm) as u32).min(h));
    let mut ink = 0.0;
    for y in y0..y1 {
        for x in x0..x1 {
            ink += pm.pixel(x, y).unwrap().alpha() as f64 / 255.0;
        }
    }
    (ink / ((x1 - x0) as f64 * (y1 - y0) as f64), pm)
}
