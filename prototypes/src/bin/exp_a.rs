//! Experiment A: image and colour pipeline.
//!   exp_a correctness <dir>        fixtures + checks (ramps, patches, ICC, alpha, orientation)
//!   exp_a gen-large <dir>          60 MP 16-bit TIFF + 24 MP JPEG for memory runs
//!   exp_a pipeline <file> <w_mm> <h_mm> <lpi> <px_per_cell>   timed stages (run under /usr/bin/time -l)
//!   exp_a downscale-error          sampling error vs pixels-per-cell and filter
use fast_image_resize::FilterType;
use halftone_prototypes::lattice::{Lattice, Rect};
use halftone_prototypes::luma::*;
use image::codecs::png::PngEncoder;
use image::metadata::Orientation;
use image::{DynamicImage, ImageBuffer, ImageEncoder, Rgb, Rgba};
use moxcms::ColorProfile;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("correctness") => correctness(Path::new(&args[2])),
        Some("gen-large") => gen_large(Path::new(&args[2])),
        Some("pipeline") => pipeline(
            Path::new(&args[2]),
            args[3].parse().unwrap(),
            args[4].parse().unwrap(),
            args[5].parse().unwrap(),
            args[6].parse().unwrap(),
        ),
        Some("downscale-error") => downscale_error(),
        _ => eprintln!("see header comment for usage"),
    }
}

fn save_png(path: &Path, img: &DynamicImage, icc: Option<Vec<u8>>) {
    let f = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut enc = PngEncoder::new(f);
    if let Some(p) = icc {
        enc.set_icc_profile(p).unwrap();
    }
    enc.write_image(img.as_bytes(), img.width(), img.height(), img.color().into()).unwrap();
}

fn luma_of(path: &Path) -> (Plane, &'static str) {
    let d = decode(path).unwrap();
    let m = luma_model(d.image.color().has_color(), d.icc.as_deref()).unwrap();
    (orient(linear_luma(&d.image, &m, 1.0), d.orientation), m.path)
}

fn srgb_decode(v: f64) -> f64 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn correctness(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let w_srgb = ColorProfile::new_srgb().colorant_matrix().v[1];
    println!("sRGB D50 Y weights: {:.4} {:.4} {:.4}", w_srgb[0], w_srgb[1], w_srgb[2]);

    // 1. Untagged 8-bit gray ramp: Y must equal the sRGB EOTF.
    let ramp = ImageBuffer::from_fn(256, 4, |x, _| Rgb([x as u8; 3]));
    let p = dir.join("ramp8.png");
    save_png(&p, &DynamicImage::ImageRgb8(ramp), None);
    let (pl, path) = luma_of(&p);
    let worst = (0..256).map(|x| (pl.at(x, 0) as f64 - srgb_decode(x as f64 / 255.0)).abs()).fold(0.0, f64::max);
    println!("[ramp8 untagged] path={path}; worst |Y - sRGB EOTF| = {worst:.2e}");

    // 2. Patches: untagged vs sRGB-tagged vs Display P3-tagged.
    let patches: [[u8; 3]; 6] = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255], [0, 0, 0], [128, 128, 128]];
    let img = DynamicImage::ImageRgb8(ImageBuffer::from_fn(6, 1, |x, _| Rgb(patches[x as usize])));
    let p3 = ColorProfile::new_display_p3();
    for (name, icc) in [
        ("untagged", None),
        ("sRGB-tagged", Some(ColorProfile::new_srgb().encode().unwrap())),
        ("P3-tagged", Some(p3.encode().unwrap())),
    ] {
        let f = dir.join(format!("patches-{name}.png"));
        save_png(&f, &img, icc);
        let (pl, path) = luma_of(&f);
        let ys: Vec<String> = (0..6).map(|x| format!("{:.4}", pl.at(x, 0))).collect();
        println!("[patches {name}] path={path}; Y(R,G,B,W,K,128) = {}", ys.join(" "));
    }
    let p3y = p3.colorant_matrix().v[1];
    println!("  expected P3 primaries Y (D50): {:.4} {:.4} {:.4}", p3y[0], p3y[1], p3y[2]);
    // What a convert-to-sRGB-first pipeline would have produced for P3 red:
    let t = p3
        .create_transform_16bit(moxcms::Layout::Rgb, &ColorProfile::new_srgb(), moxcms::Layout::Rgb, Default::default())
        .unwrap();
    let mut out = [0u16; 3];
    t.transform(&[65535, 0, 0], &mut out).unwrap();
    let clipped: f64 = (0..3).map(|c| w_srgb[c] * srgb_decode(out[c] as f64 / 65535.0)).sum();
    println!("  P3 red via sRGB conversion (gamut-clipped) would give Y = {clipped:.4}");

    // 3. Alpha: black with alpha ramp over the white analysis background.
    let a = ImageBuffer::from_fn(256, 1, |x, _| Rgba([0, 0, 0, x as u8]));
    let f = dir.join("alpha.png");
    save_png(&f, &DynamicImage::ImageRgba8(a), None);
    let (pl, _) = luma_of(&f);
    let worst = (0..256).map(|x| (pl.at(x, 0) as f64 - (1.0 - x as f64 / 255.0)).abs()).fold(0.0, f64::max);
    println!("[alpha over white] worst |Y - (1 - a)| = {worst:.2e}");

    // 4. Orientation on the luminance plane vs image's apply_orientation.
    let src = DynamicImage::ImageRgb8(ImageBuffer::from_fn(7, 5, |x, y| Rgb([(x * 30) as u8, (y * 50) as u8, ((x * y) % 255) as u8])));
    let m = luma_model(true, None).unwrap();
    let mut all_ok = true;
    for o in [
        Orientation::NoTransforms, Orientation::Rotate90, Orientation::Rotate180, Orientation::Rotate270,
        Orientation::FlipHorizontal, Orientation::FlipVertical, Orientation::Rotate90FlipH, Orientation::Rotate270FlipH,
    ] {
        let mine = orient(linear_luma(&src, &m, 1.0), o);
        let mut reference = src.clone();
        reference.apply_orientation(o);
        let theirs = linear_luma(&reference, &m, 1.0);
        let ok = mine.width == theirs.width && mine.px.iter().zip(&theirs.px).all(|(a, b)| a.0 == b.0);
        all_ok &= ok;
        if !ok {
            println!("  orientation {o:?}: MISMATCH");
        }
    }
    println!("[orientation] all 8 EXIF orientations match image::apply_orientation: {all_ok}");

    // 4b. Real-world ICC files shipped with macOS: parse + tagged-PNG round trip.
    for sys in ["/System/Library/ColorSync/Profiles", "/Library/ColorSync/Profiles"] {
        let Ok(rd) = std::fs::read_dir(sys) else { continue };
        for e in rd.flatten() {
            let bytes = std::fs::read(e.path()).unwrap_or_default();
            let name = e.file_name().to_string_lossy().to_string();
            match ColorProfile::new_from_slice(&bytes) {
                Ok(p) if p.color_space == moxcms::DataColorSpace::Rgb => {
                    let f = dir.join(format!("icc-{name}.png"));
                    save_png(&f, &img, Some(bytes.clone()));
                    let d = decode(&f).unwrap();
                    match luma_model(true, d.icc.as_deref()) {
                        Ok(m) => {
                            let pl = linear_luma(&d.image, &m, 1.0);
                            println!("  [{name}] {}; Y(R, 128 gray, W) = {:.4} {:.4} {:.4}", m.path, pl.at(0, 0), pl.at(5, 0), pl.at(3, 0));
                        }
                        Err(err) => println!("  [{name}] UNUSABLE ({err}); matrix_shaper={}", p.is_matrix_shaper()),
                    }
                }
                Ok(p) => println!("  [{name}] {:?} profile (not RGB; skipped)", p.color_space),
                Err(err) => println!("  [{name}] parse error: {err:?}"),
            }
        }
    }

    // 5. 16-bit TIFF ramp.
    let r16 = ImageBuffer::from_fn(1024, 2, |x, _| Rgb([(x * 64) as u16; 3]));
    let f = dir.join("ramp16.tif");
    DynamicImage::ImageRgb16(r16).save(&f).unwrap();
    let (pl, _) = luma_of(&f);
    let worst = (0..1024).map(|x| (pl.at(x, 0) as f64 - srgb_decode((x * 64) as f64 / 65535.0)).abs()).fold(0.0, f64::max);
    println!("[ramp16 tiff] worst |Y - sRGB EOTF| = {worst:.2e}");
}

/// Deterministic photo-like content: smooth tones, fine texture and hard edges.
fn synthetic(x: f64, y: f64) -> f64 {
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
    let smooth = 0.5 + 0.35 * (x * 3.1).sin() * (y * 2.3).cos();
    let texture = 0.25 * (noise(x * 300.0, y * 300.0) - 0.5) + 0.15 * (noise(x * 2000.0, y * 2000.0) - 0.5);
    let edge = if ((x * 8.0).floor() as i64 + (y * 5.0).floor() as i64) % 7 == 0 { -0.3 } else { 0.0 };
    (smooth + texture + edge).clamp(0.0, 1.0)
}

fn gen_large(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let to16 = |v: f64| (v.powf(1.0 / 2.2) * 65535.0) as u16;
    let (w, h) = (9487u32, 6325u32);
    let t = Instant::now();
    let img = ImageBuffer::from_fn(w, h, |x, y| {
        let v = synthetic(x as f64 / w as f64, y as f64 / h as f64);
        Rgb([to16(v), to16((v * 0.9).min(1.0)), to16((v * 1.1).min(1.0))])
    });
    DynamicImage::ImageRgb16(img).save(dir.join("large-60mp-16bit.tif")).unwrap();
    let (w, h) = (6000u32, 4000u32);
    let img = ImageBuffer::from_fn(w, h, |x, y| {
        let v = (synthetic(x as f64 / w as f64, y as f64 / h as f64).powf(1.0 / 2.2) * 255.0) as u8;
        Rgb([v, v, v])
    });
    DynamicImage::ImageRgb8(img).save(dir.join("large-24mp.jpg")).unwrap();
    println!("generated in {:?}", t.elapsed());
}

/// Output-size target: px_per_cell pixels per screen cell along each axis.
fn pipeline(path: &Path, w_mm: f64, h_mm: f64, lpi: f64, px_per_cell: f64) {
    let t0 = Instant::now();
    let d = decode(path).unwrap();
    let (sw, sh) = (d.image.width(), d.image.height());
    println!("decode            {:>8.0?}  {}x{} {:?}  ({:.0} MB)", t0.elapsed(), sw, sh, d.image.color(), d.image.as_bytes().len() as f64 / 1e6);
    let t = Instant::now();
    let m = luma_model(d.image.color().has_color(), d.icc.as_deref()).unwrap();
    let plane = linear_luma(&d.image, &m, 1.0);
    let orientation = d.orientation;
    drop(d);
    println!("linear luma       {:>8.0?}  ({:.0} MB f32 plane; decode buffer dropped)", t.elapsed(), plane.px.len() as f64 * 4.0 / 1e6);
    let t = Instant::now();
    let plane = orient(plane, orientation);
    println!("orientation       {:>8.0?}  {:?}", t.elapsed(), orientation);
    // Contain-fit, then target pixels from the output size.
    let pitch = 25.4 / lpi;
    let scale = (w_mm / plane.width as f64).min(h_mm / plane.height as f64); // mm per source px
    let (fit_w, fit_h) = (plane.width as f64 * scale, plane.height as f64 * scale);
    let (tw, th) = (((fit_w / pitch) * px_per_cell).round() as u32, ((fit_h / pitch) * px_per_cell).round() as u32);
    let t = Instant::now();
    let small = downscale(&plane, tw, th, FilterType::Box);
    drop(plane);
    println!("downscale         {:>8.0?}  -> {}x{} ({:.1} MB)", t.elapsed(), small.width, small.height, small.px.len() as f64 * 4.0 / 1e6);
    println!("total             {:>8.0?}", t0.elapsed());
}

/// Sampling error vs ground truth (cell-area average on a high-res plane).
fn downscale_error() {
    // A3 portrait at 45 LPI and 22 LPI; a 40 MP-class source (5460 x 7720).
    let (w_mm, h_mm) = (297.0, 420.0);
    let (sw, sh) = (5460u32, 7720u32);
    let mut full = Plane { width: sw, height: sh, px: vec![fast_image_resize::pixels::F32::new(0.0); (sw * sh) as usize] };
    use rayon::prelude::*;
    full.px.par_chunks_exact_mut(sw as usize).enumerate().for_each(|(y, row)| {
        for (x, v) in row.iter_mut().enumerate() {
            v.0 = synthetic((x as f64 + 0.5) / sw as f64, (y as f64 + 0.5) / sh as f64) as f32;
        }
    });
    let px_per_mm = sw as f64 / w_mm;
    for lpi in [22.0, 45.0] {
        let rect = Rect { x0: 0.0, y0: 0.0, x1: w_mm, y1: h_mm };
        let lat = Lattice::from_lpi(lpi, 22.5, rect);
        let cells = lat.whole_cells(rect, lat.pitch * std::f64::consts::FRAC_1_SQRT_2).cells;
        let ((ux, uy), (vx, vy)) = {
            let (s, c) = 22.5f64.to_radians().sin_cos();
            ((c, s), (-s, c))
        };
        // Ground truth: mean of 16x16 bilinear samples over the rotated cell.
        let truth: Vec<f64> = cells
            .par_iter()
            .map(|c| {
                let mut acc = 0.0;
                for a in 0..16 {
                    for b in 0..16 {
                        let (s, t) = ((a as f64 + 0.5) / 16.0 - 0.5, (b as f64 + 0.5) / 16.0 - 0.5);
                        let (x, y) = (c.x + lat.pitch * (s * ux + t * vx), c.y + lat.pitch * (s * uy + t * vy));
                        acc += full.bilinear(x * px_per_mm, y * px_per_mm);
                    }
                }
                acc / 256.0
            })
            .collect();
        let report = |label: &str, plane: &Plane| {
            let ppm = plane.width as f64 / w_mm;
            let mut errs: Vec<f64> = cells.iter().zip(&truth).map(|(c, t)| (plane.bilinear(c.x * ppm, c.y * ppm) - t).abs()).collect();
            errs.sort_by(f64::total_cmp);
            let mean = errs.iter().sum::<f64>() / errs.len() as f64;
            println!("  {label:<28} mean {:.4}  p99 {:.4}  max {:.4}", mean, errs[errs.len() * 99 / 100], errs[errs.len() - 1]);
        };
        println!("A3 @ {lpi} LPI: {} cells, source {:.1} px/cell", cells.len(), lat.pitch * px_per_mm);
        report("full-res centre sample", &full);
        for k in [1.0, 2.0, 3.0, 4.0, 6.0] {
            let (tw, th) = ((w_mm / lat.pitch * k).round() as u32, (h_mm / lat.pitch * k).round() as u32);
            for (fname, f) in [("box", FilterType::Box), ("lanczos3", FilterType::Lanczos3)] {
                let t = Instant::now();
                let small = downscale(&full, tw, th, f);
                let el = t.elapsed();
                report(&format!("{k} px/cell {fname} ({el:.0?})"), &small);
            }
        }
    }
}

#[allow(dead_code)]
fn _unused(_: PathBuf) {}
