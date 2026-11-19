//! EXIF orientation applied to the luminance plane (4 bytes/pixel) instead of
//! the decoded RGB buffer, so rotation never duplicates the large buffer.

use fast_image_resize::pixels::F32;
use image::metadata::Orientation;
use rayon::prelude::*;

pub fn apply(w: u32, h: u32, px: Vec<F32>, o: Orientation) -> (u32, u32, Vec<F32>) {
    if o == Orientation::NoTransforms {
        return (w, h, px);
    }
    let (w, h) = (w as usize, h as usize);
    let (ow, oh) = if super::swaps_axes(o) { (h, w) } else { (w, h) };
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
            *v = px[sy * w + sx];
        }
    });
    (ow as u32, oh as u32, out)
}

#[cfg(test)]
mod tests {
    use crate::color::{LumaModel, linear_luma};
    use image::metadata::Orientation::*;
    use image::{DynamicImage, ImageBuffer, Rgb};

    #[test]
    fn matches_image_crate_for_all_eight_orientations() {
        let src = DynamicImage::ImageRgb8(ImageBuffer::from_fn(7, 5, |x, y| {
            Rgb([(x * 30) as u8, (y * 50) as u8, ((x * y * 7) % 255) as u8])
        }));
        let (m, _) = LumaModel::new(true, None);
        for o in [
            NoTransforms,
            Rotate90,
            Rotate180,
            Rotate270,
            FlipHorizontal,
            FlipVertical,
            Rotate90FlipH,
            Rotate270FlipH,
        ] {
            let (w, h, px) = linear_luma(&src, &m, 1.0);
            let (mw, _, mine) = super::apply(w, h, px, o);
            let mut reference = src.clone();
            reference.apply_orientation(o);
            let (rw, _, theirs) = linear_luma(&reference, &m, 1.0);
            assert_eq!(mw, rw, "{o:?}");
            assert!(mine.iter().zip(&theirs).all(|(a, b)| a.0 == b.0), "{o:?}");
        }
    }
}
