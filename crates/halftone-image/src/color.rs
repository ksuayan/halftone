//! Linear-light luminance (Phase 0 report §A.2).
//!
//! - Matrix-shaper RGB ICC profiles: linearize in the image's own primaries (no
//!   gamut clipping), Y from that profile's colorants, normalized so white = 1.
//! - Other RGB ICC profiles: convert to linear sRGB (out-of-gamut colours clip).
//! - Untagged, gray, or unusable profiles: assume sRGB.

use fast_image_resize::pixels::F32;
use image::DynamicImage;
use moxcms::{ColorProfile, Layout, ProfileText, ToneReprCurve, Transform16BitExecutor, TransformOptions};
use rayon::prelude::*;
use std::sync::{Arc, OnceLock};

pub struct LumaModel {
    /// Converts 16-bit RGBA to linear 16-bit RGBA; `None` = decode with the sRGB curve.
    transform: Option<Arc<Transform16BitExecutor>>,
    weights: [f32; 3],
    pub description: String,
}

fn with_linear_trc(p: &ColorProfile) -> ColorProfile {
    let mut lin = p.clone();
    // Without this, moxcms applies the CICP transfer and ignores the TRCs below.
    lin.cicp = None;
    lin.red_trc = Some(ToneReprCurve::Parametric(vec![1.0]));
    lin.green_trc = Some(ToneReprCurve::Parametric(vec![1.0]));
    lin.blue_trc = Some(ToneReprCurve::Parametric(vec![1.0]));
    lin
}

/// Y row of the colorant matrix, normalized so (1, 1, 1) -> 1.
fn y_weights(p: &ColorProfile) -> [f32; 3] {
    let v = p.colorant_matrix().v[1];
    let sum = v[0] + v[1] + v[2];
    [(v[0] / sum) as f32, (v[1] / sum) as f32, (v[2] / sum) as f32]
}

/// Human-readable profile name, e.g. "sRGB IEC61966-2.1".
fn profile_name(p: &ColorProfile) -> String {
    let text = match &p.description {
        Some(ProfileText::PlainString(s)) => s.clone(),
        Some(ProfileText::Localizable(v)) => {
            let en = v.iter().find(|l| l.language == "en").or(v.first());
            en.map(|l| l.value.clone()).unwrap_or_default()
        }
        Some(ProfileText::Description(d)) => {
            if d.ascii_string.trim().is_empty() {
                d.unicode_string.clone()
            } else {
                d.ascii_string.clone()
            }
        }
        None => String::new(),
    };
    let text = text.trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string();
    if text.is_empty() {
        "unnamed profile".into()
    } else {
        format!("\"{text}\"")
    }
}

impl LumaModel {
    fn srgb(description: &str) -> Self {
        Self {
            transform: None,
            weights: y_weights(&ColorProfile::new_srgb()),
            description: description.into(),
        }
    }

    /// Returns the model and an optional warning (profile ignored).
    pub fn new(has_color: bool, icc: Option<&[u8]>) -> (Self, Option<String>) {
        let Some(bytes) = icc else {
            return (Self::srgb("untagged: sRGB assumed"), None);
        };
        if !has_color {
            return (
                Self::srgb("grayscale: sRGB tone curve assumed (gray ICC ignored)"),
                None,
            );
        }
        match Self::from_icc(bytes) {
            Ok(m) => (m, None),
            Err(e) => (
                Self::srgb("unusable ICC profile: sRGB assumed"),
                Some(format!("embedded ICC profile ignored ({e}); assuming sRGB")),
            ),
        }
    }

    fn from_icc(bytes: &[u8]) -> Result<Self, String> {
        let src = ColorProfile::new_from_slice(bytes).map_err(|e| format!("{e:?}"))?;
        let name = profile_name(&src);
        let (dst, weights, how) = if src.is_matrix_shaper() {
            (with_linear_trc(&src), y_weights(&src), "own primaries")
        } else {
            let srgb = ColorProfile::new_srgb();
            (
                with_linear_trc(&srgb),
                y_weights(&srgb),
                "via linear sRGB, out-of-gamut colours clip",
            )
        };
        let t = src
            .create_transform_16bit(Layout::Rgba, &dst, Layout::Rgba, TransformOptions::default())
            .map_err(|e| format!("{e:?}"))?;
        Ok(Self {
            transform: Some(t),
            weights,
            description: format!("ICC {name}: {how}"),
        })
    }
}

fn srgb_decode(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_lut() -> &'static [f32] {
    static LUT: OnceLock<Vec<f32>> = OnceLock::new();
    LUT.get_or_init(|| (0..=u16::MAX).map(|v| srgb_decode(v as f64 / 65535.0) as f32).collect())
}

fn identity_lut() -> &'static [f32] {
    static LUT: OnceLock<Vec<f32>> = OnceLock::new();
    LUT.get_or_init(|| (0..=u16::MAX).map(|v| v as f32 / 65535.0).collect())
}

/// Linear luminance of an sRGB colour (for the analysis background).
pub fn background_luminance(rgb: [u8; 3]) -> f32 {
    let w = y_weights(&ColorProfile::new_srgb());
    (0..3).map(|c| w[c] * srgb_decode(rgb[c] as f64 / 255.0) as f32).sum()
}

pub fn supported(img: &DynamicImage) -> bool {
    matches!(
        img,
        DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
            | DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb16(_)
            | DynamicImage::ImageRgba16(_)
            | DynamicImage::ImageLuma16(_)
            | DynamicImage::ImageLumaA16(_)
    )
}

/// Copy one row into 16-bit RGBA, whatever the decoded format.
fn row_rgba16(img: &DynamicImage, y: usize, out: &mut [u16]) {
    let w = img.width() as usize;
    let up = |v: u8| v as u16 * 257;
    macro_rules! fill {
        ($buf:expr, $n:expr, $f:expr) => {{
            let row = &$buf.as_raw()[y * w * $n..(y + 1) * w * $n];
            for (px, o) in row.chunks_exact($n).zip(out.chunks_exact_mut(4)) {
                o.copy_from_slice(&$f(px));
            }
        }};
    }
    match img {
        DynamicImage::ImageRgb8(b) => fill!(b, 3, |p: &[u8]| [up(p[0]), up(p[1]), up(p[2]), 65535]),
        DynamicImage::ImageRgba8(b) => fill!(b, 4, |p: &[u8]| [up(p[0]), up(p[1]), up(p[2]), up(p[3])]),
        DynamicImage::ImageLuma8(b) => fill!(b, 1, |p: &[u8]| [up(p[0]), up(p[0]), up(p[0]), 65535]),
        DynamicImage::ImageLumaA8(b) => fill!(b, 2, |p: &[u8]| [up(p[0]), up(p[0]), up(p[0]), up(p[1])]),
        DynamicImage::ImageRgb16(b) => fill!(b, 3, |p: &[u16]| [p[0], p[1], p[2], 65535]),
        DynamicImage::ImageRgba16(b) => fill!(b, 4, |p: &[u16]| [p[0], p[1], p[2], p[3]]),
        DynamicImage::ImageLuma16(b) => fill!(b, 1, |p: &[u16]| [p[0], p[0], p[0], 65535]),
        DynamicImage::ImageLumaA16(b) => fill!(b, 2, |p: &[u16]| [p[0], p[0], p[0], p[1]]),
        _ => unreachable!("checked by supported()"),
    }
}

/// Linear luminance plane (row-parallel), alpha composited over `background`.
pub fn linear_luma(img: &DynamicImage, model: &LumaModel, background: f32) -> (u32, u32, Vec<F32>) {
    let (w, h) = (img.width(), img.height());
    let lut = if model.transform.is_some() {
        identity_lut()
    } else {
        srgb_lut()
    };
    let [wr, wg, wb] = model.weights;
    let mut px = vec![F32::new(0.0); w as usize * h as usize];
    px.par_chunks_exact_mut(w as usize).enumerate().for_each_init(
        || (vec![0u16; w as usize * 4], vec![0u16; w as usize * 4]),
        |(raw, conv), (y, out)| {
            row_rgba16(img, y, raw);
            let src: &[u16] = match &model.transform {
                Some(t) => {
                    t.transform(raw, conv).expect("row buffers have matching lengths");
                    conv
                }
                None => raw,
            };
            for (o, p) in out.iter_mut().zip(src.chunks_exact(4)) {
                let yl = wr * lut[p[0] as usize] + wg * lut[p[1] as usize] + wb * lut[p[2] as usize];
                let a = p[3] as f32 / 65535.0;
                o.0 = a * yl + (1.0 - a) * background;
            }
        },
    );
    (w, h, px)
}
