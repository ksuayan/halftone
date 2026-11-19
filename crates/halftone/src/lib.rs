//! Photo-to-halftone pipeline. `analyze` writes a reusable [`Config`];
//! `generate` turns a config into a validated SVG. The CLI's `convert` is
//! `analyze` followed by `generate`; there is no separate processing path.

mod analyze;
pub mod config_io;
mod generate;
pub mod layout;
pub mod profile;

pub use analyze::{AnalyzeRequest, Orientation, PaperChoice, analyze};
pub use generate::{GenerateOptions, Generated, Timings, generate};
pub use halftone_types::{Config, Issue, Severity, limits::Limits};
pub use profile::Profile;

use std::path::PathBuf;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid configuration:\n{}", format_issues(.0))]
    Config(Vec<Issue>),
    #[error("{0}")]
    Image(#[from] halftone_image::ImageError),
    #[error("{0}")]
    Screen(#[from] halftone_core::CoreError),
    #[error("{0}")]
    Geometry(#[from] halftone_geometry::GeometryError),
    #[error("layout: {0}")]
    Layout(String),
    #[error("cannot read {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("{path}: {message}")]
    Json { path: PathBuf, message: String },
    #[error("internal error: generated SVG failed validation:\n{}", .0.join("\n"))]
    InvalidOutput(Vec<String>),
}

fn format_issues(issues: &[Issue]) -> String {
    issues.iter().map(|i| format!("  {i}")).collect::<Vec<_>>().join("\n")
}

/// BLAKE3 of a file, hex encoded.
pub fn hash_file(path: &std::path::Path) -> Result<String, Error> {
    let io = |source| Error::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut h = blake3::Hasher::new();
    h.update_reader(std::fs::File::open(path).map_err(io)?).map_err(io)?;
    Ok(h.finalize().to_hex().to_string())
}
