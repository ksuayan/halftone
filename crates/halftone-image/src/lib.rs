//! Image ingestion: decode -> linear-light luminance -> orientation -> downscale.
//!
//! Order (Phase 0 report §A): luminance is computed row by row straight from the
//! decoded buffer, which is then dropped; EXIF orientation is applied to the
//! 4-byte luminance plane; the plane is downscaled in linear light with a box
//! filter, which also acts as the cell-area prefilter.

mod color;
mod orient;

use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::pixels::F32;
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader};
use std::path::{Path, PathBuf};

pub use color::LumaModel;

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("cannot read {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("cannot decode {path}: {message}")]
    Decode { path: PathBuf, message: String },
    #[error("{path} is {pixels} pixels; the limit is {limit} (reduce the image or raise the limit)")]
    TooLarge { path: PathBuf, pixels: u64, limit: u64 },
    #[error("{path}: unsupported pixel format {format}")]
    Unsupported { path: PathBuf, format: String },
}

/// Header information, read without decoding pixels.
#[derive(Clone, Debug)]
pub struct Probe {
    /// Size after EXIF orientation.
    pub width_px: u32,
    pub height_px: u32,
    pub orientation: String,
    pub pixel_format: String,
    /// How luminance will be derived (ICC path or sRGB assumption).
    pub color: String,
    pub warnings: Vec<String>,
}

/// Full-resolution linear luminance (owned by this crate; downscale to use it).
pub struct Luminance {
    width: u32,
    height: u32,
    px: Vec<F32>,
}

/// Small luminance plane at working resolution.
#[derive(Clone, Debug)]
pub struct LumaPlane {
    pub width: u32,
    pub height: u32,
    data: Vec<f32>,
}

fn decode_err(path: &Path, e: impl std::fmt::Display) -> ImageError {
    ImageError::Decode {
        path: path.to_path_buf(),
        message: e.to_string(),
    }
}

fn open_decoder(path: &Path) -> Result<impl ImageDecoder, ImageError> {
    let reader = ImageReader::open(path).map_err(|source| ImageError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = reader.with_guessed_format().map_err(|source| ImageError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    reader.into_decoder().map_err(|e| decode_err(path, e))
}

fn swaps_axes(o: Orientation) -> bool {
    matches!(
        o,
        Orientation::Rotate90 | Orientation::Rotate270 | Orientation::Rotate90FlipH | Orientation::Rotate270FlipH
    )
}

struct Header {
    probe: Probe,
    orientation: Orientation,
    model: LumaModel,
}

fn read_header(dec: &mut impl ImageDecoder, path: &Path, max_pixels: u64) -> Result<Header, ImageError> {
    let (w, h) = dec.dimensions();
    let pixels = w as u64 * h as u64;
    if pixels > max_pixels {
        return Err(ImageError::TooLarge {
            path: path.to_path_buf(),
            pixels,
            limit: max_pixels,
        });
    }
    let orientation = dec.orientation().map_err(|e| decode_err(path, e))?;
    let icc = dec.icc_profile().map_err(|e| decode_err(path, e))?;
    let has_color = dec.color_type().has_color();
    let (model, warning) = LumaModel::new(has_color, icc.as_deref());
    let (ow, oh) = if swaps_axes(orientation) { (h, w) } else { (w, h) };
    let probe = Probe {
        width_px: ow,
        height_px: oh,
        orientation: format!("{orientation:?}"),
        pixel_format: format!("{:?}", dec.color_type()),
        color: model.description.clone(),
        warnings: warning.into_iter().collect(),
    };
    Ok(Header {
        probe,
        orientation,
        model,
    })
}

/// Read size, orientation, pixel format and colour handling without decoding pixels.
pub fn probe(path: &Path, max_pixels: u64) -> Result<Probe, ImageError> {
    let mut dec = open_decoder(path)?;
    Ok(read_header(&mut dec, path, max_pixels)?.probe)
}

/// Decode and convert to oriented linear luminance. Transparent pixels are
/// composited over `background` (an sRGB colour).
pub fn load_luminance(path: &Path, max_pixels: u64, background: [u8; 3]) -> Result<(Probe, Luminance), ImageError> {
    let mut dec = open_decoder(path)?;
    let header = read_header(&mut dec, path, max_pixels)?;
    let image = DynamicImage::from_decoder(dec).map_err(|e| decode_err(path, e))?;
    if !color::supported(&image) {
        return Err(ImageError::Unsupported {
            path: path.to_path_buf(),
            format: format!("{:?}", image.color()),
        });
    }
    let bg = color::background_luminance(background);
    let (w, h, px) = color::linear_luma(&image, &header.model, bg);
    drop(image);
    let (w, h, px) = orient::apply(w, h, px, header.orientation);
    Ok((
        header.probe,
        Luminance {
            width: w,
            height: h,
            px,
        },
    ))
}

impl Luminance {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Box-filter downscale in linear light. Never upscales.
    pub fn downscale(self, width: u32, height: u32) -> LumaPlane {
        let (width, height) = (width.clamp(1, self.width), height.clamp(1, self.height));
        if (width, height) == (self.width, self.height) {
            return LumaPlane {
                width,
                height,
                data: self.px.iter().map(|p| p.0).collect(),
            };
        }
        let src = TypedImageRef::<F32>::new(self.width, self.height, &self.px).expect("plane size matches dimensions");
        let mut dst = TypedImage::<F32>::new(width, height);
        let opts = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Box));
        Resizer::new()
            .resize_typed(&src, &mut dst, &opts)
            .expect("same pixel type and non-empty images");
        LumaPlane {
            width,
            height,
            data: dst.pixels().iter().map(|p| p.0.clamp(0.0, 1.0)).collect(),
        }
    }
}

impl LumaPlane {
    pub fn from_fn(width: u32, height: u32, f: impl Fn(u32, u32) -> f32) -> Self {
        let data = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y))
            .collect();
        Self { width, height, data }
    }

    pub fn get(&self, x: u32, y: u32) -> f32 {
        self.data[y as usize * self.width as usize + x as usize]
    }

    /// Bilinear sample at continuous pixel coordinates (pixel centres at +0.5), edge-clamped.
    pub fn bilinear(&self, x: f64, y: f64) -> f64 {
        let fx = (x - 0.5).clamp(0.0, (self.width - 1) as f64);
        let fy = (y - 0.5).clamp(0.0, (self.height - 1) as f64);
        let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
        let top = self.get(x0, y0) as f64 * (1.0 - tx) + self.get(x1, y0) as f64 * tx;
        let bottom = self.get(x0, y1) as f64 * (1.0 - tx) + self.get(x1, y1) as f64 * tx;
        top * (1.0 - ty) + bottom * ty
    }
}
