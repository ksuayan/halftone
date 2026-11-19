//! Generation: config -> luminance -> marks -> geometry -> validated SVG.

use crate::layout::{self, MARK_STROKE_MM, PageLayout};
use crate::{Error, analyze::derive_layout};
use halftone_core::tone::ToneCurve;
use halftone_core::{ScreenSpec, place_marks};
use halftone_geometry::{GeometryOptions, GeometryStats};
use halftone_svg::{Document, Expect};
use halftone_types::units::pitch_mm;
use halftone_types::validate::{has_errors, parse_color, validate};
use halftone_types::{Config, Issue, MultiPolygon, Severity, limits::Limits};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct GenerateOptions {
    /// Use i_overlay's internal threads (identical output either way).
    pub multithread: bool,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self { multithread: true }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Timings {
    pub decode: Duration,
    pub downscale: Duration,
    pub marks: Duration,
    pub geometry: Duration,
    pub svg: Duration,
}

pub struct Generated {
    pub svg: String,
    /// The effective configuration, with `layout` refreshed.
    pub config: Config,
    pub artwork: MultiPolygon,
    pub page: PageLayout,
    pub stats: GeometryStats,
    pub warnings: Vec<Issue>,
    pub timings: Timings,
}

impl Generated {
    pub fn document(&self) -> Document<'_> {
        Document {
            page_width_mm: self.page.page_width_mm,
            page_height_mm: self.page.page_height_mm,
            ink: &self.config.output.ink,
            artwork: &self.artwork,
            targets: &self.page.targets,
            mark_stroke_mm: MARK_STROKE_MM,
        }
    }

    pub fn preview_png(&self, dpi: f64) -> Result<Vec<u8>, String> {
        halftone_svg::preview_png(&self.document(), dpi).map_err(|e| e.to_string())
    }
}

pub fn resolve_source(config: &Config, config_dir: &Path) -> PathBuf {
    let p = Path::new(&config.source.path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        config_dir.join(p)
    }
}

fn warn(field: &str, message: String) -> Issue {
    Issue {
        severity: Severity::Warning,
        field: field.into(),
        message,
    }
}

pub fn generate(
    config: &Config,
    config_dir: &Path,
    limits: &Limits,
    opts: &GenerateOptions,
) -> Result<Generated, Error> {
    let mut warnings = validate(config);
    if has_errors(&warnings) {
        return Err(Error::Config(warnings));
    }
    let mut t = Timings::default();
    let source = resolve_source(config, config_dir);
    if let Some(expected) = &config.source.blake3
        && crate::hash_file(&source)? != *expected
    {
        warnings.push(warn(
            "source.blake3",
            format!(
                "{} changed since analysis; results may differ from the analysed image",
                source.display()
            ),
        ));
    }

    let clock = Instant::now();
    let background = parse_color(&config.tone.analysis_background).expect("validated");
    let (_, luminance) = halftone_image::load_luminance(&source, limits.max_input_pixels, background)?;
    let (sw, sh) = (luminance.width(), luminance.height());
    t.decode = clock.elapsed();
    let mut config = config.clone();
    if (sw, sh) != (config.source.width_px, config.source.height_px) {
        warnings.push(warn(
            "source.width_px",
            format!(
                "image is now {sw}x{sh} px; analysed at {}x{}",
                config.source.width_px, config.source.height_px
            ),
        ));
        (config.source.width_px, config.source.height_px) = (sw, sh);
    }
    let page = layout::compute(&config.output, (sw, sh)).map_err(|e| Error::Layout(e.0))?;

    // Downscale to px_per_cell pixels per screen cell (box filter = cell-area prefilter).
    let clock = Instant::now();
    let pitch = pitch_mm(config.screen.lpi);
    let k = config.geometry.px_per_cell;
    let (tw, th) = (
        ((page.image.width() / pitch) * k).round() as u32,
        ((page.image.height() / pitch) * k).round() as u32,
    );
    let plane = luminance.downscale(tw.max(1), th.max(1));
    t.downscale = clock.elapsed();

    let clock = Instant::now();
    let img = page.image;
    let (sx, sy) = (plane.width as f64 / img.width(), plane.height as f64 / img.height());
    let spec = ScreenSpec {
        pitch_mm: pitch,
        angle_deg: config.screen.angle_deg,
        min_dot_mm: config.tone.min_dot_mm,
        keep_radius_mm: crate::layout::keep_radius_mm(pitch, config.geometry.tolerance_um),
    };
    let placement = place_marks(&img, &spec, &ToneCurve::from(&config.tone), limits.max_cells, |x, y| {
        plane.bilinear((x - img.x0) * sx, (y - img.y0) * sy)
    })?;
    t.marks = clock.elapsed();

    let clock = Instant::now();
    let gopts = GeometryOptions {
        tolerance_mm: config.geometry.tolerance_um / 1000.0,
        max_union_vertices: limits.max_union_vertices,
        multithread: opts.multithread,
    };
    let (artwork, stats) = halftone_geometry::build(&placement.marks, pitch, &gopts)?;
    t.geometry = clock.elapsed();

    let clock = Instant::now();
    config.layout = derive_layout(&config, limits)?;
    let generated = Generated {
        svg: String::new(),
        config,
        artwork,
        page,
        stats,
        warnings,
        timings: t,
    };
    let svg = halftone_svg::write(&generated.document());
    let expect = Expect {
        page_width_mm: generated.page.page_width_mm,
        page_height_mm: generated.page.page_height_mm,
        ink: &generated.config.output.ink,
        registration_marks: generated.config.output.registration_marks,
    };
    halftone_svg::validate(&svg, &expect).map_err(Error::InvalidOutput)?;
    let mut generated = Generated { svg, ..generated };
    generated.timings.svg = clock.elapsed();
    Ok(generated)
}
