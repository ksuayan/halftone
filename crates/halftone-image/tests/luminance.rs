use halftone_image::{ImageError, load_luminance, probe};
use image::codecs::png::PngEncoder;
use image::{DynamicImage, ImageBuffer, ImageEncoder, Rgb, Rgba};
use moxcms::ColorProfile;
use std::path::{Path, PathBuf};

const WHITE: [u8; 3] = [255, 255, 255];
const LIMIT: u64 = 150_000_000;

fn srgb_decode(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn save_png(dir: &Path, name: &str, img: &DynamicImage, icc: Option<Vec<u8>>) -> PathBuf {
    let path = dir.join(name);
    let mut enc = PngEncoder::new(std::io::BufWriter::new(std::fs::File::create(&path).unwrap()));
    if let Some(p) = icc {
        enc.set_icc_profile(p).unwrap();
    }
    enc.write_image(img.as_bytes(), img.width(), img.height(), img.color().into())
        .unwrap();
    path
}

/// Full-resolution plane (downscale to the same size is a copy).
fn plane(path: &Path) -> halftone_image::LumaPlane {
    let (_, l) = load_luminance(path, LIMIT, WHITE).unwrap();
    let (w, h) = (l.width(), l.height());
    l.downscale(w, h)
}

#[test]
fn untagged_ramp_follows_srgb_curve() {
    let dir = tempfile::tempdir().unwrap();
    let ramp = DynamicImage::ImageRgb8(ImageBuffer::from_fn(256, 2, |x, _| Rgb([x as u8; 3])));
    let p = plane(&save_png(dir.path(), "ramp.png", &ramp, None));
    for x in 0..256 {
        assert!(
            (p.get(x, 0) as f64 - srgb_decode(x as f64 / 255.0)).abs() < 1e-6,
            "x={x}"
        );
    }
}

#[test]
fn sixteen_bit_tiff_ramp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ramp16.tif");
    DynamicImage::ImageRgb16(ImageBuffer::from_fn(1024, 2, |x, _| Rgb([(x * 64) as u16; 3])))
        .save(&path)
        .unwrap();
    let p = plane(&path);
    for x in 0..1024 {
        assert!((p.get(x, 0) as f64 - srgb_decode((x * 64) as f64 / 65535.0)).abs() < 1e-6);
    }
}

#[test]
fn icc_tagged_images_use_their_own_primaries() {
    let dir = tempfile::tempdir().unwrap();
    let patches = [[255u8, 0, 0], [128, 128, 128], [255, 255, 255]];
    let img = DynamicImage::ImageRgb8(ImageBuffer::from_fn(3, 1, |x, _| Rgb(patches[x as usize])));
    let untagged = plane(&save_png(dir.path(), "u.png", &img, None));
    let srgb = plane(&save_png(
        dir.path(),
        "s.png",
        &img,
        Some(ColorProfile::new_srgb().encode().unwrap()),
    ));
    let p3_profile = ColorProfile::new_display_p3();
    let p3 = plane(&save_png(dir.path(), "p.png", &img, Some(p3_profile.encode().unwrap())));
    // Mid-gray and white agree regardless of tagging.
    for x in 1..3 {
        assert!((untagged.get(x, 0) - srgb.get(x, 0)).abs() < 2e-4);
        assert!((untagged.get(x, 0) - p3.get(x, 0)).abs() < 2e-4);
    }
    assert!((p3.get(2, 0) - 1.0).abs() < 1e-4);
    // P3 red keeps its own (larger) luminance instead of the gamut-clipped sRGB value.
    let v = p3_profile.colorant_matrix().v[1];
    let expected = v[0] / (v[0] + v[1] + v[2]);
    assert!((p3.get(0, 0) as f64 - expected).abs() < 2e-4);
    assert!(p3.get(0, 0) > srgb.get(0, 0) + 0.01);
    let probed = probe(&dir.path().join("p.png"), LIMIT).unwrap();
    assert!(probed.color.contains("own primaries"), "{}", probed.color);
    assert!(
        !probed.color.contains("DescriptionString") && !probed.color.contains("Localizable"),
        "{}",
        probed.color
    );
}

#[test]
fn alpha_composites_over_background() {
    let dir = tempfile::tempdir().unwrap();
    let img = DynamicImage::ImageRgba8(ImageBuffer::from_fn(256, 1, |x, _| Rgba([0, 0, 0, x as u8])));
    let p = plane(&save_png(dir.path(), "a.png", &img, None));
    for x in 0..256 {
        assert!((p.get(x, 0) as f64 - (1.0 - x as f64 / 255.0)).abs() < 1e-6);
    }
}

#[test]
fn rejects_images_over_the_pixel_limit_before_decoding() {
    let dir = tempfile::tempdir().unwrap();
    let img = DynamicImage::ImageRgb8(ImageBuffer::new(100, 100));
    let path = save_png(dir.path(), "big.png", &img, None);
    assert!(matches!(
        probe(&path, 9_999),
        Err(ImageError::TooLarge { pixels: 10_000, .. })
    ));
    assert!(matches!(
        load_luminance(&path, 9_999, WHITE),
        Err(ImageError::TooLarge { .. })
    ));
}

#[test]
fn missing_and_corrupt_files_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        probe(&dir.path().join("nope.png"), LIMIT),
        Err(ImageError::Io { .. })
    ));
    let bad = dir.path().join("bad.png");
    std::fs::write(&bad, b"\x89PNG\r\n\x1a\nnot really").unwrap();
    assert!(load_luminance(&bad, LIMIT, WHITE).is_err());
}

#[test]
fn downscale_preserves_mean_tone() {
    let dir = tempfile::tempdir().unwrap();
    let img = DynamicImage::ImageRgb8(ImageBuffer::from_fn(400, 300, |x, y| {
        Rgb([((x * 7 + y * 3) % 256) as u8; 3])
    }));
    let (_, l) = load_luminance(&save_png(dir.path(), "t.png", &img, None), LIMIT, WHITE).unwrap();
    let full = {
        let (_, l2) = load_luminance(&dir.path().join("t.png"), LIMIT, WHITE).unwrap();
        l2.downscale(400, 300)
    };
    let small = l.downscale(40, 30);
    let mean = |p: &halftone_image::LumaPlane| {
        let mut s = 0.0;
        for y in 0..p.height {
            for x in 0..p.width {
                s += p.get(x, y) as f64;
            }
        }
        s / (p.width * p.height) as f64
    };
    assert_eq!((small.width, small.height), (40, 30));
    assert!((mean(&small) - mean(&full)).abs() < 1e-3);
}
