//! PNG preview rasterized straight from the geometry (not by re-parsing the SVG),
//! flattened onto white: transparent previews show as solid black in common viewers.

use crate::Document;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("preview of {0}x{1} px is too large; lower the preview dpi")]
    TooLarge(u32, u32),
    #[error("cannot encode PNG: {0}")]
    Encode(String),
}

pub fn preview_png(doc: &Document, dpi: f64) -> Result<Vec<u8>, PreviewError> {
    let px_per_mm = dpi / 25.4;
    let (w, h) = (
        (doc.page_width_mm * px_per_mm).ceil() as u32,
        (doc.page_height_mm * px_per_mm).ceil() as u32,
    );
    if w as u64 * h as u64 > 400_000_000 {
        return Err(PreviewError::TooLarge(w, h));
    }
    let mut pm = Pixmap::new(w.max(1), h.max(1)).ok_or(PreviewError::TooLarge(w, h))?;
    pm.fill(Color::WHITE);
    let rgb = halftone_types::validate::parse_color(doc.ink).unwrap_or([0, 0, 0]);
    let mut paint = Paint::default();
    paint.set_color_rgba8(rgb[0], rgb[1], rgb[2], 255);
    paint.anti_alias = true;
    let s = (px_per_mm / 1000.0) as f32; // px per µm
    let mut pb = PathBuilder::new();
    for poly in &doc.artwork.0 {
        for ring in std::iter::once(poly.exterior()).chain(poly.interiors()) {
            let mut it = ring.0.iter();
            if let Some(p) = it.next() {
                pb.move_to(p.x as f32 * s, p.y as f32 * s);
                for p in it {
                    pb.line_to(p.x as f32 * s, p.y as f32 * s);
                }
                pb.close();
            }
        }
    }
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint, FillRule::EvenOdd, Transform::identity(), None);
    }
    let mm = px_per_mm as f32;
    let mut pb = PathBuilder::new();
    for t in doc.targets {
        let (x, y, r, a) = (
            t.cx_mm as f32 * mm,
            t.cy_mm as f32 * mm,
            t.radius_mm as f32 * mm,
            t.arm_mm as f32 * mm,
        );
        pb.push_circle(x, y, r);
        pb.move_to(x - a, y);
        pb.line_to(x + a, y);
        pb.move_to(x, y - a);
        pb.line_to(x, y + a);
    }
    if let Some(path) = pb.finish() {
        let stroke = Stroke {
            width: (doc.mark_stroke_mm as f32 * mm).max(1.0),
            ..Stroke::default()
        };
        pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
    pm.encode_png().map_err(|e| PreviewError::Encode(e.to_string()))
}
