//! The analysis configuration (spec S§12): a complete, versioned, editable
//! description of one generation job. Unknown fields are rejected.
//!
//! - Every length is millimetres and says so in its name.
//! - `recommendations` records what analysis proposed; the other sections hold
//!   the effective values; `user_set` lists fields the user supplied explicitly.
//! - `layout` is derived. Generation recomputes it; edits to it are ignored.

use crate::units::Units;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    /// Halftone version that wrote this file.
    pub app_version: String,
    pub source: Source,
    pub output: Output,
    pub screen: Screen,
    pub tone: Tone,
    pub geometry: GeometrySettings,
    pub layout: Layout,
    pub recommendations: Recommendations,
    /// Dotted paths of settings the user supplied (e.g. `screen.lpi`).
    pub user_set: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Path to the source image, relative to the config file's directory or absolute.
    pub path: String,
    /// BLAKE3 hash of the source file; generation warns when it no longer matches.
    pub blake3: Option<String>,
    /// Pixel size after EXIF orientation.
    pub width_px: u32,
    pub height_px: u32,
    pub orientation: String,
    pub pixel_format: String,
    /// How luminance is derived (ICC path or sRGB assumption).
    pub color: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Fit {
    /// Whole image inside the image area, aspect preserved, centred.
    Contain,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Output {
    /// Named paper size, if one was used.
    pub paper: Option<String>,
    /// Paper size. The SVG page is this size minus `printer_margin_mm` on every side,
    /// so printing with Fit to Page scales by about 1.0.
    pub paper_width_mm: f64,
    pub paper_height_mm: f64,
    /// Non-printable border of the target printer, per side.
    pub printer_margin_mm: f64,
    pub fit: Fit,
    /// Fill colour, `#RRGGBB`. Laser output should be pure black.
    pub ink: String,
    pub registration_marks: bool,
    /// Units for reports and messages.
    pub display_units: Units,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Grid {
    Cartesian,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Shape {
    Circle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    pub grid: Grid,
    pub shape: Shape,
    /// Lines per inch, measured along the lattice axes (pitch = 25.4 / lpi mm).
    pub lpi: f64,
    /// Lattice rotation in degrees, normalized to [0, 90).
    pub angle_deg: f64,
    /// Screen mesh count (threads per inch), if known.
    pub mesh_count: Option<f64>,
    /// Mesh-to-LPI ratio used for recommendations (5.0 conservative, 4.0 aggressive).
    pub mesh_ratio: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Tone {
    /// false: dark source -> large marks. true: light source -> large marks.
    pub invert: bool,
    /// Linear luminance at or below this is treated as black (0..1).
    pub black_point: f64,
    /// Linear luminance at or above this is treated as white (0..1).
    pub white_point: f64,
    /// Midtone gamma: normalized luminance is raised to 1/gamma (1.0 = none; >1 lightens).
    pub gamma: f64,
    /// Marks smaller than this diameter are not drawn.
    pub min_dot_mm: f64,
    /// Upper bound on target coverage (1.0 = solid shadows allowed).
    pub max_coverage: f64,
    /// Transparent pixels are composited over this colour before analysis.
    pub analysis_background: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GeometrySettings {
    /// Maximum radial error of polygonal circles, in micrometres.
    pub tolerance_um: f64,
    /// Working-image resolution in pixels per screen cell (per axis).
    pub px_per_cell: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RectMm {
    pub x_mm: f64,
    pub y_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    /// SVG page size (paper minus printer margins).
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    /// Where the photograph sits on the page.
    pub image: RectMm,
    pub pitch_mm: f64,
    /// Whole screen cells inside the image area.
    pub cells: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LpiSource {
    /// mesh_count / mesh_ratio.
    Mesh,
    /// Profile default (no mesh count given).
    Profile,
    /// Supplied by the user.
    User,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Recommendations {
    pub profile: String,
    pub lpi: f64,
    pub lpi_source: LpiSource,
    /// Advisory LPI range for the mesh (mesh/5 .. mesh/4), if a mesh is known.
    pub lpi_range: Option<[f64; 2]>,
    pub notes: Vec<String>,
}
