use clap::{Args, Parser, Subcommand, ValueEnum};
use halftone::{AnalyzeRequest, Config, Orientation, PaperChoice, Profile};
use halftone_types::Units;
use halftone_types::validate::normalize_angle;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "halftone",
    version,
    about = "Convert photographs into single-ink halftone SVG positives for screen printing and linocut."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
    /// Worker threads (default: all cores).
    #[arg(long, global = true)]
    pub threads: Option<usize>,
    /// Report effective settings, timings and geometry statistics.
    #[arg(long, short, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Inspect an image and write a reusable JSON configuration (no SVG).
    Analyze {
        #[command(flatten)]
        analyze: AnalyzeArgs,
        /// Configuration file to write.
        #[arg(long, short)]
        output: PathBuf,
    },
    /// Generate the SVG from a configuration, with optional overrides.
    Generate {
        /// Configuration written by `analyze`.
        config: PathBuf,
        #[command(flatten)]
        generate: GenerateArgs,
    },
    /// Analyze and generate in one step (same code path as the two-step workflow).
    Convert {
        #[command(flatten)]
        analyze: AnalyzeArgs,
        #[command(flatten)]
        generate: GenerateArgs,
    },
    /// Validate a configuration and, optionally, an SVG generated from it.
    Validate {
        /// Configuration file to check.
        config: PathBuf,
        /// SVG generated from this configuration, to check as well.
        #[arg(long)]
        svg: Option<PathBuf>,
    },
    /// Print the configuration JSON Schema.
    Schema,
    /// Print the version.
    Version,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OrientationArg {
    Auto,
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ProfileArg {
    ScreenPrint,
    Linocut,
}

#[derive(Args)]
pub struct AnalyzeArgs {
    /// Source photograph (JPEG, PNG or TIFF).
    pub input: PathBuf,
    /// Named paper: letter, legal, tabloid, 9x12, a5, a4, a3.
    #[arg(long, conflicts_with_all = ["width", "height"])]
    pub paper: Option<String>,
    /// Custom paper width (in --units).
    #[arg(long, requires = "height")]
    pub width: Option<f64>,
    /// Custom paper height (in --units).
    #[arg(long, requires = "width")]
    pub height: Option<f64>,
    /// Units for --width, --height and reports.
    #[arg(long, default_value = "in", value_parser = parse_units)]
    pub units: Units,
    /// Paper orientation; auto matches the photo.
    #[arg(long, value_enum, default_value_t = OrientationArg::Auto)]
    pub orientation: OrientationArg,
    /// Print profile supplying defaults (mesh, LPI, angle, minimum dot).
    #[arg(long, value_enum, default_value_t = ProfileArg::ScreenPrint)]
    pub profile: ProfileArg,
    /// Screen mesh count (threads per inch); LPI is recommended from it.
    #[arg(long)]
    pub mesh: Option<f64>,
    /// Screen frequency; overrides the mesh-based recommendation.
    #[arg(long)]
    pub lpi: Option<f64>,
    /// Screen angle in degrees.
    #[arg(long, allow_hyphen_values = true)]
    pub angle: Option<f64>,
    /// Ink colour #RRGGBB (laser output should stay #000000).
    #[arg(long)]
    pub ink: Option<String>,
    /// Printer's non-printable border per side, in mm.
    #[arg(long)]
    pub printer_margin_mm: Option<f64>,
    /// Add registration targets (off by default).
    #[arg(long)]
    pub registration_marks: bool,
    /// Light areas get large marks.
    #[arg(long)]
    pub invert: bool,
}

fn parse_units(s: &str) -> Result<Units, String> {
    s.parse()
}

impl AnalyzeArgs {
    pub fn request(&self, config_dir: PathBuf) -> Result<AnalyzeRequest, String> {
        let paper = match (&self.paper, self.width, self.height) {
            (Some(p), _, _) => PaperChoice::Named(p.clone()),
            (None, Some(w), Some(h)) => PaperChoice::Custom {
                width_mm: self.units.to_mm(w),
                height_mm: self.units.to_mm(h),
            },
            _ => return Err("give --paper NAME or both --width and --height".into()),
        };
        Ok(AnalyzeRequest {
            input: self.input.clone(),
            config_dir,
            paper,
            orientation: match self.orientation {
                OrientationArg::Auto => Orientation::Auto,
                OrientationArg::Portrait => Orientation::Portrait,
                OrientationArg::Landscape => Orientation::Landscape,
            },
            display_units: self.units,
            profile: match self.profile {
                ProfileArg::ScreenPrint => Profile::ScreenPrint,
                ProfileArg::Linocut => Profile::Linocut,
            },
            mesh_count: self.mesh,
            lpi: self.lpi,
            angle_deg: self.angle,
            ink: self.ink.clone(),
            printer_margin_mm: self.printer_margin_mm,
            registration_marks: self.registration_marks.then_some(true),
            invert: self.invert.then_some(true),
        })
    }
}

#[derive(Args)]
pub struct GenerateArgs {
    /// SVG file to write.
    #[arg(long, short)]
    pub output: PathBuf,
    /// Also write a PNG preview (flattened on white).
    #[arg(long)]
    pub preview: Option<PathBuf>,
    /// Resolution of the PNG preview.
    #[arg(long, default_value_t = 150.0)]
    pub preview_dpi: f64,
    /// Write the effective configuration (with overrides) to this file.
    #[arg(long)]
    pub save_config: Option<PathBuf>,
    /// Override the screen frequency.
    #[arg(long, value_name = "LPI")]
    pub set_lpi: Option<f64>,
    /// Override the screen angle.
    #[arg(long, value_name = "DEGREES", allow_hyphen_values = true)]
    pub set_angle: Option<f64>,
    /// Override the ink colour.
    #[arg(long, value_name = "COLOR")]
    pub set_ink: Option<String>,
    /// Smallest dot drawn, in mm.
    #[arg(long)]
    pub min_dot_mm: Option<f64>,
    /// Cap on ink coverage (0..1), e.g. 0.9 to keep shadows open.
    #[arg(long)]
    pub max_coverage: Option<f64>,
    /// Midtone gamma (>1 lightens).
    #[arg(long)]
    pub gamma: Option<f64>,
    /// Linear luminance treated as solid black (0..1).
    #[arg(long)]
    pub black_point: Option<f64>,
    /// Linear luminance treated as white, i.e. no ink (0..1).
    #[arg(long)]
    pub white_point: Option<f64>,
    /// Polygon tolerance in µm.
    #[arg(long)]
    pub tolerance_um: Option<f64>,
    /// Override registration targets on or off.
    #[arg(long, value_name = "BOOL")]
    pub set_registration_marks: Option<bool>,
}

impl GenerateArgs {
    /// Apply overrides; validation happens in `halftone::generate` (same rules as JSON).
    pub fn apply(&self, mut c: Config) -> Result<Config, String> {
        let set = |field: &str, c: &mut Config| {
            if !c.user_set.iter().any(|f| f == field) {
                c.user_set.push(field.into());
            }
        };
        if let Some(v) = self.set_lpi {
            c.screen.lpi = v;
            c.recommendations.lpi_source = halftone_types::config::LpiSource::User;
            set("screen.lpi", &mut c);
        }
        if let Some(v) = self.set_angle {
            c.screen.angle_deg = normalize_angle(v);
            set("screen.angle_deg", &mut c);
        }
        if let Some(v) = &self.set_ink {
            c.output.ink = v.clone();
            set("output.ink", &mut c);
        }
        if let Some(v) = self.min_dot_mm {
            c.tone.min_dot_mm = v;
            set("tone.min_dot_mm", &mut c);
        }
        if let Some(v) = self.max_coverage {
            c.tone.max_coverage = v;
            set("tone.max_coverage", &mut c);
        }
        if let Some(v) = self.gamma {
            c.tone.gamma = v;
            set("tone.gamma", &mut c);
        }
        if let Some(v) = self.black_point {
            c.tone.black_point = v;
            set("tone.black_point", &mut c);
        }
        if let Some(v) = self.white_point {
            c.tone.white_point = v;
            set("tone.white_point", &mut c);
        }
        if let Some(v) = self.tolerance_um {
            c.geometry.tolerance_um = v;
            set("geometry.tolerance_um", &mut c);
        }
        if let Some(v) = self.set_registration_marks {
            c.output.registration_marks = v;
            set("output.registration_marks", &mut c);
        }
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn clap_definition_is_consistent() {
        use clap::CommandFactory;
        super::Cli::command().debug_assert();
    }
}
