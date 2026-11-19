//! Experiments B + D: lattice, marks, Boolean union strategies, hybrid arcs, SVG.
//!   exp_d <preset> <method> [out.svg]
//!   preset: 9x12-22 | 9x12-45 | a3-22 | a3-45
//!   method: poly-mt | poly-st | tiled | hybrid | check
use fast_image_resize::FilterType;
use fast_image_resize::pixels::F32;
use halftone_prototypes::coverage::{coverage_of_diameter, diameter_for_coverage};
use halftone_prototypes::geom::*;
use halftone_prototypes::lattice::{Lattice, Rect};
use halftone_prototypes::luma::{Plane, downscale};
use rayon::prelude::*;
use std::time::Instant;

fn tol_mm() -> f64 { std::env::var("HT_TOL").ok().and_then(|v| v.parse().ok()).unwrap_or(0.005) } // max radial error of polygon circles
const MIN_DOT_MM: f64 = 0.13; // ~3 device px at 600 dpi (hypothesis; Exp E)
const ANGLE: f64 = 22.5;

struct Job {
    rect: Rect,
    lat: Lattice,
    marks: Vec<Mark>,
    predicted_ink_mm2: f64,
    set: halftone_prototypes::lattice::CellSet,
    cell_of_mark: Vec<usize>,
}

fn synthetic(x: f64, y: f64) -> f64 {
    // Same family as exp_a: smooth tones + texture + hard edges, full tonal range.
    let h = |a: i64, b: i64| {
        let mut v = (a.wrapping_mul(374761393) ^ b.wrapping_mul(668265263)) as u64;
        v = (v ^ (v >> 13)).wrapping_mul(1274126177);
        ((v >> 40) & 0xffff) as f64 / 65535.0
    };
    let noise = |sx: f64, sy: f64| {
        let (ix, iy) = (sx.floor() as i64, sy.floor() as i64);
        let (fx, fy) = (sx - ix as f64, sy - iy as f64);
        let a = h(ix, iy) * (1.0 - fx) + h(ix + 1, iy) * fx;
        let b = h(ix, iy + 1) * (1.0 - fx) + h(ix + 1, iy + 1) * fx;
        a * (1.0 - fy) + b * fy
    };
    let smooth = 0.5 + 0.48 * (x * 3.1).sin() * (y * 2.3).cos();
    let texture = 0.2 * (noise(x * 40.0, y * 40.0) - 0.5);
    let edge = if ((x * 8.0).floor() as i64 + (y * 5.0).floor() as i64) % 7 == 0 { -0.3 } else { 0.0 };
    (smooth + texture + edge).clamp(0.0, 1.0)
}

fn job(preset: &str) -> Job {
    let dark = preset.ends_with("-dark");
    let preset = preset.trim_end_matches("-dark");
    let (w, h, lpi) = match preset {
        "9x12-22" => (228.6, 304.8, 22.0),
        "9x12-45" => (228.6, 304.8, 45.0),
        "a3-22" => (297.0, 420.0, 22.0),
        "a3-45" => (297.0, 420.0, 45.0),
        _ => panic!("unknown preset"),
    };
    let rect = Rect { x0: 0.0, y0: 0.0, x1: w, y1: h };
    let lat = Lattice::from_lpi(lpi, ANGLE, rect);
    let p = lat.pitch;
    // Tone plane: "photo" at 8 px/mm, downscaled to 2 px/cell (Exp A recommendation).
    let (sw, sh) = ((w * 8.0) as u32, (h * 8.0) as u32);
    let mut full = Plane { width: sw, height: sh, px: vec![F32::new(0.0); (sw * sh) as usize] };
    full.px.par_chunks_exact_mut(sw as usize).enumerate().for_each(|(y, row)| {
        for (x, v) in row.iter_mut().enumerate() {
            v.0 = synthetic((x as f64 + 0.5) / sw as f64, (y as f64 + 0.5) / sh as f64) as f32;
        }
    });
    let small = downscale(&full, (w / p * 2.0).round() as u32, (h / p * 2.0).round() as u32, FilterType::Box);
    let ppm = small.width as f64 / w;
    let set = lat.whole_cells(rect, p * std::f64::consts::FRAC_1_SQRT_2);
    let per_cell: Vec<Option<Mark>> = set
        .cells
        .par_iter()
        .map(|c| {
            let y = small.bilinear(c.x * ppm, c.y * ppm) * if dark { 0.25 } else { 1.0 };
            let d = diameter_for_coverage(1.0 - y) * p;
            (d >= MIN_DOT_MM).then_some(Mark { i: c.i, j: c.j, x: c.x, y: c.y, r: d / 2.0 })
        })
        .collect();
    let mut marks = Vec::new();
    let mut cell_of_mark = Vec::new();
    let mut predicted = 0.0;
    for (k, m) in per_cell.into_iter().enumerate() {
        if let Some(m) = m {
            predicted += coverage_of_diameter(2.0 * m.r / p) * p * p;
            marks.push(m);
            cell_of_mark.push(k);
        }
    }
    Job { rect, lat, marks, predicted_ink_mm2: predicted, set, cell_of_mark }
}

fn polygons(marks: &[Mark]) -> Vec<Vec<i_overlay::i_float::int::point::IntPoint<i32>>> {
    marks.par_iter().map(|m| circle_polygon(m, tol_mm())).collect()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a[1] == "arc-check" {
        arc_check();
        return;
    }
    let (preset, method) = (a[1].as_str(), a[2].as_str());
    let out = a.get(3);
    let t = Instant::now();
    let j = job(preset);
    let (w, h) = (j.rect.x1, j.rect.y1);
    println!(
        "{preset}: pitch {:.4} mm, {} whole cells, {} marks (>= {MIN_DOT_MM} mm), setup {:?}",
        j.lat.pitch,
        j.set.cells.len(),
        j.marks.len(),
        t.elapsed()
    );
    let t = Instant::now();
    let (svg_d, ink_mm2, verts) = match method {
        "poly-mt" | "poly-st" => {
            let polys = polygons(&j.marks);
            let t_gen = t.elapsed();
            let u = union(&polys, method == "poly-mt");
            println!("  polygons {:?} ({} input vertices), union {:?}", t_gen, polys.iter().map(Vec::len).sum::<usize>(), t.elapsed() - t_gen);
            let mut d = String::new();
            write_polygons(&mut d, &u);
            (d, area_mm2(&u), vertex_count(&u))
        }
        "tiled" => {
            let u = union_tiled(&j.marks, tol_mm(), 32);
            println!("  tiled union (32x32-cell tiles, fixed tree merge) {:?}", t.elapsed());
            let mut d = String::new();
            write_polygons(&mut d, &u);
            (d, area_mm2(&u), vertex_count(&u))
        }
        "hybrid-poly" => {
            // Union only overlapping marks; isolated marks emitted as polygons directly.
            let flags = overlapping_flags(&j.marks, &j.set, &j.cell_of_mark, j.lat.pitch, 0.002);
            let over: Vec<Mark> = j.marks.iter().zip(&flags).filter(|(_, f)| **f).map(|(m, _)| *m).collect();
            let iso: Vec<Mark> = j.marks.iter().zip(&flags).filter(|(_, f)| !**f).map(|(m, _)| *m).collect();
            let mut u = union(&polygons(&over), true);
            u.extend(polygons(&iso).into_iter().map(|c| vec![c]));
            println!("  hybrid-poly: {} isolated, {} overlapping {:?}", iso.len(), over.len(), t.elapsed());
            let mut d = String::new();
            write_polygons(&mut d, &u);
            (d, area_mm2(&u), vertex_count(&u))
        }
        "hybrid" => {
            let flags = overlapping_flags(&j.marks, &j.set, &j.cell_of_mark, j.lat.pitch, 0.002);
            let over_owned: Vec<Mark> = j.marks.iter().zip(&flags).filter(|(_, f)| **f).map(|(m, _)| *m).collect();
            let iso: Vec<&Mark> = j.marks.iter().zip(&flags).filter(|(_, f)| !**f).map(|(m, _)| m).collect();
            let polys = polygons(&over_owned);
            let u = union(&polys, true);
            let iso_area: f64 = iso.iter().map(|m| std::f64::consts::PI * m.r * m.r).sum();
            let mut d = String::new();
            write_polygons(&mut d, &u);
            write_arcs(&mut d, &iso);
            println!("  hybrid: {} isolated (arcs), {} overlapping (union) {:?}", iso.len(), over_owned.len(), t.elapsed());
            (d, area_mm2(&u) + iso_area, vertex_count(&u))
        }
        "check" => {
            check(&j);
            return;
        }
        _ => panic!("unknown method"),
    };
    let total = t.elapsed();
    let svg = svg_document(w, h, &svg_d, "");
    let ink_pct = 100.0 * ink_mm2 / (w * h);
    let pred_pct = 100.0 * j.predicted_ink_mm2 / (w * h);
    println!(
        "  geometry total {:?}; output vertices {}; SVG {:.1} MB; ink {:.3}% vs predicted {:.3}% (diff {:+.4} pts)",
        total,
        verts,
        svg.len() as f64 / 1e6,
        ink_pct,
        pred_pct,
        ink_pct - pred_pct
    );
    if let Some(path) = out {
        std::fs::write(path, &svg).unwrap();
        let t = Instant::now();
        let (frac, _) = raster_ink_fraction(&svg, 600.0 / 25.4, j.rect);
        println!("  resvg @600dpi: ink {:.3}% (render {:?})", frac * 100.0, t.elapsed());
    }
}

/// Exp B checks + D determinism checks.
fn check(j: &Job) {
    // Every mark lies fully inside the image rectangle (whole-cell policy).
    let outside = j.marks.iter().filter(|m| m.x - m.r < j.rect.x0 || m.y - m.r < j.rect.y0 || m.x + m.r > j.rect.x1 || m.y + m.r > j.rect.y1).count();
    println!("  marks crossing the image edge: {outside}");
    // Lattice round trip.
    let worst = j
        .set
        .cells
        .iter()
        .map(|c| {
            let (a, b) = j.lat.to_lattice(c.x, c.y);
            (a - c.i as f64).abs().max((b - c.j as f64).abs())
        })
        .fold(0.0, f64::max);
    println!("  lattice inverse round-trip worst error: {worst:.2e} cells");
    // Determinism: multithreaded union twice, and vs single-threaded.
    let polys = polygons(&j.marks);
    let a = union(&polys, true);
    let b = union(&polys, true);
    let c = union(&polys, false);
    println!("  union mt == mt: {}, mt == st: {}", a == b, a == c);
    let tiled = union_tiled(&j.marks, tol_mm(), 32);
    println!("  tiled area {:.6} vs single {:.6} mm2 (diff {:.2e})", area_mm2(&tiled), area_mm2(&a), area_mm2(&tiled) - area_mm2(&a));
}

/// Does a renderer reproduce exact-arc dots as accurately as polygon dots?
/// Isolated dots of one radius on a 40x40 grid, rendered at 600 and 4800 dpi.
fn arc_check() {
    println!("radius   exact%   arcs@600  poly@600  arcs@4800 poly@4800");
    for r in [0.065, 0.1, 0.15, 0.25, 0.4] {
        let pitch = 1.2;
        let marks: Vec<Mark> = (0..40)
            .flat_map(|j| (0..40).map(move |i| Mark { i, j, x: pitch * (i as f64 + 0.5), y: pitch * (j as f64 + 0.5), r }))
            .collect();
        let side = pitch * 40.0;
        let rect = Rect { x0: 0.0, y0: 0.0, x1: side, y1: side };
        let exact = 100.0 * marks.len() as f64 * std::f64::consts::PI * r * r / (side * side);
        let mut arcs = String::new();
        write_arcs(&mut arcs, &marks.iter().collect::<Vec<_>>());
        let polys: Vec<_> = marks.iter().map(|m| circle_polygon(m, 0.010)).collect();
        let mut poly = String::new();
        write_polygons(&mut poly, &vec![polys]);
        let ink = |d: &str, dpi: f64| 100.0 * raster_ink_fraction(&svg_document(side, side, d, ""), dpi / 25.4, rect).0;
        println!(
            "{r:.3} mm  {exact:.3}   {:.3}    {:.3}    {:.3}     {:.3}",
            ink(&arcs, 600.0),
            ink(&poly, 600.0),
            ink(&arcs, 4800.0),
            ink(&poly, 4800.0)
        );
    }
}
