//! `halftone`: photographs to single-ink halftone SVG positives.

mod args;
mod report;

use anyhow::Context;
use args::{AnalyzeArgs, Cli, Command, GenerateArgs};
use clap::Parser;
use halftone::config_io;
use halftone::{Config, Error, GenerateOptions, Limits};
use halftone_types::validate::{has_errors, validate};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Exit codes (spec S§13.3).
mod exit {
    pub const INPUT: u8 = 3;
    pub const CONFIG: u8 = 4;
    pub const PROCESSING: u8 = 5;
    pub const OUTPUT: u8 = 6;
    pub const INTERNAL: u8 = 70;
}

/// An error plus the exit code it maps to.
struct Failure(u8, anyhow::Error);

impl From<Error> for Failure {
    fn from(e: Error) -> Self {
        let code = match &e {
            Error::Image(_) | Error::Io { .. } => exit::INPUT,
            Error::Config(_) | Error::Json { .. } => exit::CONFIG,
            Error::Screen(_) | Error::Geometry(_) | Error::Layout(_) => exit::PROCESSING,
            Error::InvalidOutput(_) => exit::INTERNAL,
        };
        Failure(code, e.into())
    }
}

fn output_failure(e: impl Into<anyhow::Error>) -> Failure {
    Failure(exit::OUTPUT, e.into())
}

/// Write via a temporary file in the same directory, then rename: a failed run
/// never leaves a partial file behind.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)
        .with_context(|| format!("cannot write in {}", dir.display()))
        .map_err(output_failure)?;
    tmp.write_all(bytes)
        .and_then(|_| tmp.flush())
        .with_context(|| format!("cannot write {}", path.display()))
        .map_err(output_failure)?;
    tmp.persist(path)
        .with_context(|| format!("cannot create {}", path.display()))
        .map_err(output_failure)?;
    Ok(())
}

fn print_issues(issues: &[halftone::Issue]) {
    for i in issues {
        eprintln!("{i}");
    }
}

fn config_dir_of(path: &Path) -> PathBuf {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn analyze(a: &AnalyzeArgs, config_out: &Path, limits: &Limits) -> Result<Config, Failure> {
    let req = a
        .request(config_dir_of(config_out))
        .map_err(|e| Failure(exit::CONFIG, anyhow::anyhow!(e)))?;
    let (config, issues) = halftone::analyze(&req, limits)?;
    print_issues(&issues);
    Ok(config)
}

fn generate(
    config: Config,
    config_dir: &Path,
    g: &GenerateArgs,
    limits: &Limits,
    verbose: bool,
) -> Result<(), Failure> {
    let config = g.apply(config).map_err(|e| Failure(exit::CONFIG, anyhow::anyhow!(e)))?;
    let generated = halftone::generate(&config, config_dir, limits, &GenerateOptions::default())?;
    print_issues(&generated.warnings);
    write_atomic(&g.output, generated.svg.as_bytes())?;
    if let Some(png) = &g.preview {
        let bytes = generated
            .preview_png(g.preview_dpi)
            .map_err(|e| output_failure(anyhow::anyhow!(e)))?;
        write_atomic(png, &bytes)?;
    }
    if let Some(path) = &g.save_config {
        write_atomic(path, config_io::to_json(&generated.config).as_bytes())?;
    }
    report::summary(&generated, &g.output, verbose);
    Ok(())
}

fn run(cli: Cli) -> Result<(), Failure> {
    if let Some(n) = cli.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()
            .map_err(|e| Failure(exit::INTERNAL, e.into()))?;
    }
    let limits = Limits::default();
    match cli.command {
        Command::Analyze { analyze: a, output } => {
            let config = analyze(&a, &output, &limits)?;
            write_atomic(&output, config_io::to_json(&config).as_bytes())?;
            report::analysis(&config, &output);
        }
        Command::Generate { config, generate: g } => {
            let c = config_io::load(&config)?;
            generate(c, &config_dir_of(&config), &g, &limits, cli.verbose)?;
        }
        Command::Convert {
            analyze: a,
            generate: g,
        } => {
            // Same APIs as analyze + generate; the config lives next to the SVG.
            let anchor = g.save_config.clone().unwrap_or_else(|| g.output.clone());
            let config = analyze(&a, &anchor, &limits)?;
            generate(config, &config_dir_of(&anchor), &g, &limits, cli.verbose)?;
        }
        Command::Validate { config, svg } => {
            let c = config_io::load(&config)?;
            let issues = validate(&c);
            print_issues(&issues);
            if has_errors(&issues) {
                return Err(Failure(
                    exit::CONFIG,
                    anyhow::anyhow!("{} is not valid", config.display()),
                ));
            }
            if let Some(svg_path) = svg {
                let text = std::fs::read_to_string(&svg_path)
                    .with_context(|| format!("cannot read {}", svg_path.display()))
                    .map_err(|e| Failure(exit::INPUT, e))?;
                let expect = halftone_svg::Expect {
                    page_width_mm: c.layout.page_width_mm,
                    page_height_mm: c.layout.page_height_mm,
                    ink: &c.output.ink,
                    registration_marks: c.output.registration_marks,
                };
                let r = halftone_svg::validate(&text, &expect).map_err(|p| {
                    Failure(
                        exit::CONFIG,
                        anyhow::anyhow!("{} failed validation:\n  {}", svg_path.display(), p.join("\n  ")),
                    )
                })?;
                println!(
                    "{}: valid ({} subpaths, {} points)",
                    svg_path.display(),
                    r.subpaths,
                    r.coords
                );
            }
            println!("{}: valid", config.display());
        }
        Command::Schema => print!("{}", config_io::schema()),
        Command::Version => println!("halftone {}", halftone::VERSION),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure(code, e)) => {
            eprintln!("error: {e:#}");
            ExitCode::from(code)
        }
    }
}
