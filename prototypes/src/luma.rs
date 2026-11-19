//! Experiment A: decode -> linear-light luminance -> orientation -> downscale.
//!
//! Order under test (TODO "Downscaling" decision):
//! decode -> (ICC -> sRGB) -> linear light -> alpha over white -> orientation -> downscale.
//! Orientation is applied to the 4-byte luminance plane rather than the decoded
//! RGB buffer so rotation never duplicates the large decode buffer.

use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::pixels::F32;
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader};
use moxcms::{ColorProfile, Layout, ToneReprCurve, Transform16BitExecutor, TransformOptions};
use rayon::prelude::*;
use std::path::Path;
use std::sync::{Arc, OnceLock};

pub struct Decoded {
    pub image: DynamicImage,
    pub icc: Option<Vec<u8>>,
    pub orientation: Orientation,
}

pub struct Plane {
    pub width: u32,
    pub height: u32,
    pub px: Vec<F32>,
}

impl Plane {
    pub fn at(&self, x: u32, y: u32) -> f32 {
        self.px[y as usize * self.width as usize + x as usize].0
    }

    /// Bilinear sample at continuous pixel coordinates (pixel centres at +0.5).
    pub fn bilinear(&self, x: f64, y: f64) -> f64 {
        let (fx, fy) = ((x - 0.5).clamp(0.0, self.width as f64 - 1.0), (y - 0.5).clamp(0.0, self.height as f64 - 1.0));
        let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
        let top = self.at(x0, y0) as f64 * (1.0 - tx) + self.at(x1, y0) as f64 * tx;
        let bot = self.at(x0, y1) as f64 * (1.0 - tx) + self.at(x1, y1) as f64 * tx;
        top * (1.0 - ty) + bot * ty
    }
}

pub fn decode(path: &Path) -> image::ImageResult<Decoded> {
    let mut dec = ImageReader::open(path)?.with_guessed_format()?.into_decoder()?;
    let icc = dec.icc_profile()?;
    let orientation = dec.orientation()?;
    let image = DynamicImage::from_decoder(dec)?;
    Ok(Decoded { image, icc, orientation })
}

/// sRGB transfer function decoded to linear light, for every 16-bit code value.
fn srgb_to_linear_16() -> &'static [f32] {
    static LUT: OnceLock<Vec<f32>> = OnceLock::new();
    LUT.get_or_init(|| {
        (0..=u16::MAX)
            .map(|v| {
                let c = v as f64 / 65535.0;
                (if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }) as f32
            })
            .collect()
    })
}

/// Copy one row into 16-bit RGBA, whatever the decoded format.
fn row_rgba16(img: &DynamicImage, y: u32, out: &mut [u16]) {
    let w = img.width() as usize;
    let up = |v: u8| v as u16 * 257;
    macro_rules! fill {
        ($buf:expr, $n:expr, $f:expr) => {{
            let row = &$buf.as_raw()[y as usize * w * $n..(y as usize + 1) * w * $n];
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
        other => panic!("unsupported pixel format {:?}", other.color()),
    }
}

/// How luminance is derived for one image.
pub struct LumaModel {
    /// Converts to linear light (no transform = untagged, use the sRGB LUT).
    transform: Option<Arc<Transform16BitExecutor>>,
    /// Y row of the profile's (D50-adapted) colorant matrix.
    pub weights: [f32; 3],
    pub path: &'static str,
}

fn with_linear_trc(p: &ColorProfile) -> ColorProfile {
    let mut lin = p.clone();
    lin.cicp = None; // else moxcms applies the CICP transfer and ignores the TRCs below
    lin.red_trc = Some(ToneReprCurve::Parametric(vec![1.0]));
    lin.green_trc = Some(ToneReprCurve::Parametric(vec![1.0]));
    lin.blue_trc = Some(ToneReprCurve::Parametric(vec![1.0]));
    lin
}

/// Y row of the colorant matrix, normalized so device white (1,1,1) maps to Y = 1.
/// Scanner/device profiles often have colorants summing to less (or more) than 1.
fn y_row(p: &ColorProfile) -> [f32; 3] {
    let v = p.colorant_matrix().v[1];
    let sum = v[0] + v[1] + v[2];
    [(v[0] / sum) as f32, (v[1] / sum) as f32, (v[2] / sum) as f32]
}

/// Matrix-shaper RGB profiles: linearize in the image's own primaries (no gamut
/// clipping), then Y from that profile's colorants. Other RGB profiles: convert to
/// linear sRGB (out-of-gamut colours clip). Untagged or gray: assume sRGB.
pub fn luma_model(is_rgb: bool, icc: Option<&[u8]>) -> Result<LumaModel, String> {
    let srgb = ColorProfile::new_srgb();
    let opts = TransformOptions::default();
    let Some(bytes) = icc.filter(|_| is_rgb) else {
        return Ok(LumaModel { transform: None, weights: y_row(&srgb), path: "untagged/gray: sRGB assumed" });
    };
    let src = ColorProfile::new_from_slice(bytes).map_err(|e| format!("icc: {e:?}"))?;
    let (dst, weights, path) = if src.is_matrix_shaper() {
        (with_linear_trc(&src), y_row(&src), "matrix-shaper: own primaries")
    } else {
        (with_linear_trc(&srgb), y_row(&srgb), "LUT profile: via linear sRGB (clips)")
    };
    let t = src.create_transform_16bit(Layout::Rgba, &dst, Layout::Rgba, opts).map_err(|e| format!("transform: {e:?}"))?;
    Ok(LumaModel { transform: Some(t), weights, path })
}

/// Linear-light luminance (0 = black, 1 = white), alpha composited over `background`.
pub fn linear_luma(img: &DynamicImage, model: &LumaModel, background: f32) -> Plane {
    let (w, h) = (img.width(), img.height());
    let transform = &model.transform;
    let [wr, wg, wb] = model.weights;
    let lut: &[f32] = match transform {
        Some(_) => identity_16(),
        None => srgb_to_linear_16(),
    };
    let mut px = vec![F32::new(0.0); w as usize * h as usize];
    px.par_chunks_exact_mut(w as usize).enumerate().for_each_init(
        || (vec![0u16; w as usize * 4], vec![0u16; w as usize * 4]),
        |(raw, conv), (y, out)| {
            row_rgba16(img, y as u32, raw);
            let src: &[u16] = match &transform {
                Some(t) => {
                    t.transform(raw, conv).expect("cms row");
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
    Plane { width: w, height: h, px }
}

fn identity_16() -> &'static [f32] {
    static LUT: OnceLock<Vec<f32>> = OnceLock::new();
    LUT.get_or_init(|| (0..=u16::MAX).map(|v| v as f32 / 65535.0).collect())
}

/// Apply EXIF orientation to the luminance plane.
pub fn orient(p: Plane, o: Orientation) -> Plane {
    let (w, h) = (p.width as usize, p.height as usize);
    let swap = matches!(o, Orientation::Rotate90 | Orientation::Rotate270 | Orientation::Rotate90FlipH | Orientation::Rotate270FlipH);
    if o == Orientation::NoTransforms {
        return p;
    }
    let (ow, oh) = if swap { (h, w) } else { (w, h) };
    let mut out = vec![F32::new(0.0); ow * oh];
    out.par_chunks_exact_mut(ow).enumerate().for_each(|(y, row)| {
        for (x, v) in row.iter_mut().enumerate() {
            let (sx, sy) = match o {
                Orientation::NoTransforms => (x, y),
                Orientation::Rotate90 => (y, h - 1 - x),
                Orientation::Rotate180 => (w - 1 - x, h - 1 - y),
                Orientation::Rotate270 => (w - 1 - y, x),
                Orientation::FlipHorizontal => (w - 1 - x, y),
                Orientation::FlipVertical => (x, h - 1 - y),
                Orientation::Rotate90FlipH => (y, x),
                Orientation::Rotate270FlipH => (w - 1 - y, h - 1 - x),
            };
            *v = p.px[sy * w + sx];
        }
    });
    Plane { width: ow as u32, height: oh as u32, px: out }
}

/// Downscale in linear light with an area (box) filter.
pub fn downscale(p: &Plane, width: u32, height: u32, filter: FilterType) -> Plane {
    if width >= p.width && height >= p.height {
        return Plane { width: p.width, height: p.height, px: p.px.clone() };
    }
    let src = TypedImageRef::<F32>::new(p.width, p.height, &p.px).expect("src view");
    let mut dst = TypedImage::<F32>::new(width, height);
    let opts = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(filter));
    Resizer::new().resize_typed(&src, &mut dst, &opts).expect("resize");
    Plane { width, height, px: dst.pixels().to_vec() }
}
