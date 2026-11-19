//! Analysis: inspect the source and requested output, recommend screen
//! settings, and produce a complete configuration (no SVG).

use crate::layout::{self, rect_mm};
use crate::profile::Profile;
use crate::{Error, VERSION};
use halftone_core::lattice::Lattice;
use halftone_types::config::*;
use halftone_types::limits::Limits;
use halftone_types::units::pitch_mm;
use halftone_types::validate::{has_errors, normalize_angle, validate};
use halftone_types::{Config, Issue, Units, paper};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Match the photo: landscape photos get landscape paper.
    Auto,
    Portrait,
    Landscape,
}

#[derive(Clone, Debug)]
pub enum PaperChoice {
    Named(String),
    /// Custom size in millimetres.
    Custom {
        width_mm: f64,
        height_mm: f64,
    },
}

/// Inputs to analysis. `None` means "use the profile default".
#[derive(Clone, Debug)]
pub struct AnalyzeRequest {
    pub input: PathBuf,
    /// Directory the config will be written to (source path is stored relative to it).
    pub config_dir: PathBuf,
    pub paper: PaperChoice,
    pub orientation: Orientation,
    pub display_units: Units,
    pub profile: Profile,
    pub mesh_count: Option<f64>,
    pub lpi: Option<f64>,
    pub angle_deg: Option<f64>,
    pub ink: Option<String>,
    pub printer_margin_mm: Option<f64>,
    pub registration_marks: Option<bool>,
    pub invert: Option<bool>,
}

fn paper_size(req: &AnalyzeRequest, src: (u32, u32)) -> Result<(Option<String>, f64, f64), Error> {
    let (name, w, h) = match &req.paper {
        PaperChoice::Named(n) => {
            let p = paper::find(n).ok_or_else(|| {
                Error::Config(vec![Issue {
                    severity: halftone_types::Severity::Error,
                    field: "output.paper".into(),
                    message: format!("unknown paper '{n}' (known: {})", paper::names()),
                }])
            })?;
            (Some(p.name.to_string()), p.width_mm, p.height_mm)
        }
        PaperChoice::Custom { width_mm, height_mm } => (None, *width_mm, *height_mm),
    };
    let landscape_photo = src.0 > src.1;
    let swap = match req.orientation {
        Orientation::Auto => landscape_photo != (w > h),
        Orientation::Portrait => w > h,
        Orientation::Landscape => w < h,
    };
    Ok(if swap { (name, h, w) } else { (name, w, h) })
}

/// Path relative to `dir` when the file lives under it, else absolute (D31).
fn stored_path(input: &Path, dir: &Path) -> String {
    let abs = input.canonicalize().unwrap_or_else(|_| input.to_path_buf());
    let base = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    abs.strip_prefix(&base).unwrap_or(&abs).to_string_lossy().into_owned()
}

fn user_set(req: &AnalyzeRequest) -> Vec<String> {
    let mut v: Vec<String> = vec!["output.paper".into()];
    let flags = [
        (req.mesh_count.is_some(), "screen.mesh_count"),
        (req.lpi.is_some(), "screen.lpi"),
        (req.angle_deg.is_some(), "screen.angle_deg"),
        (req.ink.is_some(), "output.ink"),
        (req.printer_margin_mm.is_some(), "output.printer_margin_mm"),
        (req.registration_marks.is_some(), "output.registration_marks"),
        (req.invert.is_some(), "tone.invert"),
    ];
    v.extend(flags.iter().filter(|(set, _)| *set).map(|(_, f)| f.to_string()));
    v
}

/// Inspect the image header and requested output; return the configuration and
/// any warnings. Fails on settings that would make generation impossible.
pub fn analyze(req: &AnalyzeRequest, limits: &Limits) -> Result<(Config, Vec<Issue>), Error> {
    let probe = halftone_image::probe(&req.input, limits.max_input_pixels)?;
    let d = req.profile.defaults();
    let (paper, paper_w, paper_h) = paper_size(req, (probe.width_px, probe.height_px))?;
    let mesh_count = req.mesh_count.or(d.mesh_count);
    let (lpi, lpi_source) = match (req.lpi, mesh_count) {
        (Some(l), _) => (l, LpiSource::User),
        (None, Some(m)) => (m / d.mesh_ratio, LpiSource::Mesh),
        (None, None) => (d.lpi, LpiSource::Profile),
    };
    let lpi = (lpi * 100.0).round() / 100.0;
    let mut notes: Vec<String> = d.notes.iter().map(|s| s.to_string()).collect();
    notes.extend(probe.warnings.iter().cloned());
    let output = Output {
        paper,
        paper_width_mm: paper_w,
        paper_height_mm: paper_h,
        printer_margin_mm: req.printer_margin_mm.unwrap_or(d.printer_margin_mm),
        fit: Fit::Contain,
        ink: req.ink.clone().unwrap_or_else(|| "#000000".into()),
        registration_marks: req.registration_marks.unwrap_or(true),
        display_units: req.display_units,
    };
    let mut config = Config {
        schema_version: SCHEMA_VERSION,
        app_version: VERSION.into(),
        source: Source {
            path: stored_path(&req.input, &req.config_dir),
            blake3: Some(crate::hash_file(&req.input)?),
            width_px: probe.width_px,
            height_px: probe.height_px,
            orientation: probe.orientation,
            pixel_format: probe.pixel_format,
            color: probe.color,
        },
        output,
        screen: Screen {
            grid: Grid::Cartesian,
            shape: Shape::Circle,
            lpi,
            angle_deg: normalize_angle(req.angle_deg.unwrap_or(d.angle_deg)),
            mesh_count,
            mesh_ratio: d.mesh_ratio,
        },
        tone: Tone {
            invert: req.invert.unwrap_or(false),
            black_point: 0.0,
            white_point: 1.0,
            gamma: 1.0,
            min_dot_mm: d.min_dot_mm,
            max_coverage: d.max_coverage,
            analysis_background: "#FFFFFF".into(),
        },
        geometry: GeometrySettings {
            tolerance_um: d.tolerance_um,
            px_per_cell: d.px_per_cell,
        },
        layout: Layout {
            page_width_mm: 0.0,
            page_height_mm: 0.0,
            image: rect_mm(&Default::default()),
            pitch_mm: 0.0,
            cells: 0,
        },
        recommendations: Recommendations {
            profile: req.profile.name().into(),
            lpi,
            lpi_source,
            lpi_range: mesh_count.map(|m| [m / 5.0, m / 4.0]),
            notes,
        },
        user_set: user_set(req),
    };
    let issues = validate(&config);
    if has_errors(&issues) {
        return Err(Error::Config(issues));
    }
    config.layout = derive_layout(&config, limits)?;
    Ok((config, issues))
}

/// Derived layout section (also recomputed by `generate`).
pub(crate) fn derive_layout(c: &Config, limits: &Limits) -> Result<Layout, Error> {
    let page = layout::compute(&c.output, (c.source.width_px, c.source.height_px)).map_err(|e| Error::Layout(e.0))?;
    let pitch = pitch_mm(c.screen.lpi);
    let lattice = Lattice::new(pitch, c.screen.angle_deg, &page.image);
    let estimated = lattice.estimate_cells(&page.image).ceil() as u64;
    if estimated > limits.max_cells as u64 {
        return Err(halftone_core::CoreError::TooManyCells {
            estimated,
            limit: limits.max_cells,
        }
        .into());
    }
    let cells = lattice
        .whole_cells(&page.image, pitch * std::f64::consts::FRAC_1_SQRT_2)
        .cells
        .len() as u64;
    if cells == 0 {
        return Err(Error::Layout(format!(
            "the {:.1} x {:.1} mm image area holds no whole screen cell at {} lpi; use larger paper, a higher LPI, or --no-registration-marks",
            page.image.width(),
            page.image.height(),
            c.screen.lpi
        )));
    }
    Ok(Layout {
        page_width_mm: (page.page_width_mm * 1000.0).round() / 1000.0,
        page_height_mm: (page.page_height_mm * 1000.0).round() / 1000.0,
        image: rect_mm(&page.image),
        pitch_mm: (pitch * 1e6).round() / 1e6,
        cells,
    })
}
