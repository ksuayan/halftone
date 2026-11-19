//! Configuration validation (spec S§12.3). Shared by JSON loading and CLI
//! overrides so both paths enforce identical rules.

use crate::config::{Config, SCHEMA_VERSION};
use crate::units::pitch_mm;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub severity: Severity,
    /// Dotted path of the setting, e.g. `screen.lpi`.
    pub field: String,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{s}: {}: {}", self.field, self.message)
    }
}

pub fn has_errors(issues: &[Issue]) -> bool {
    issues.iter().any(|i| i.severity == Severity::Error)
}

/// Normalize an angle to [0, 90): a square lattice repeats every 90 degrees.
pub fn normalize_angle(deg: f64) -> f64 {
    let a = deg.rem_euclid(90.0);
    if a >= 90.0 { 0.0 } else { a }
}

/// Parse `#RRGGBB`.
pub fn parse_color(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let b = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([b(0)?, b(2)?, b(4)?])
}

struct Checker(Vec<Issue>);

impl Checker {
    fn push(&mut self, severity: Severity, field: &str, message: String) {
        self.0.push(Issue {
            severity,
            field: field.into(),
            message,
        });
    }
    fn error(&mut self, field: &str, message: String) {
        self.push(Severity::Error, field, message);
    }
    fn warn(&mut self, field: &str, message: String) {
        self.push(Severity::Warning, field, message);
    }
    /// Finite and within [lo, hi]; returns whether the value is usable.
    fn range(&mut self, field: &str, v: f64, lo: f64, hi: f64, unit: &str) -> bool {
        if !v.is_finite() {
            self.error(field, format!("must be a finite number, got {v}"));
            return false;
        }
        if v < lo || v > hi {
            self.error(
                field,
                format!("{v}{unit} is outside the supported range {lo}{unit}..{hi}{unit}"),
            );
            return false;
        }
        true
    }
    fn color(&mut self, field: &str, s: &str) -> Option<[u8; 3]> {
        let c = parse_color(s);
        if c.is_none() {
            self.error(field, format!("'{s}' is not a colour; expected #RRGGBB, e.g. #000000"));
        }
        c
    }
}

pub fn validate(c: &Config) -> Vec<Issue> {
    let mut k = Checker(Vec::new());
    if c.schema_version != SCHEMA_VERSION {
        k.error(
            "schema_version",
            format!(
                "version {} is not supported (expected {SCHEMA_VERSION})",
                c.schema_version
            ),
        );
    }
    if c.source.path.trim().is_empty() {
        k.error("source.path", "must name the source image".into());
    }
    output(&mut k, c);
    screen(&mut k, c);
    tone(&mut k, c);
    k.range("geometry.tolerance_um", c.geometry.tolerance_um, 1.0, 100.0, " µm");
    k.range("geometry.px_per_cell", c.geometry.px_per_cell, 1.0, 8.0, "");
    k.0
}

fn output(k: &mut Checker, c: &Config) {
    let o = &c.output;
    let w = k.range("output.paper_width_mm", o.paper_width_mm, 20.0, 2000.0, " mm");
    let h = k.range("output.paper_height_mm", o.paper_height_mm, 20.0, 2000.0, " mm");
    let m = k.range("output.printer_margin_mm", o.printer_margin_mm, 0.0, 50.0, " mm");
    if w && h && m && 2.0 * o.printer_margin_mm >= o.paper_width_mm.min(o.paper_height_mm) {
        k.error(
            "output.printer_margin_mm",
            "margins leave no printable area; reduce the margin or use larger paper".into(),
        );
    }
    if let Some(rgb) = k.color("output.ink", &o.ink)
        && rgb != [0, 0, 0]
    {
        k.warn(
            "output.ink",
            format!(
                "{} is not pure black; laser printers screen non-black ink into their own pattern",
                o.ink
            ),
        );
    }
}

fn screen(k: &mut Checker, c: &Config) {
    let s = &c.screen;
    let lpi_ok = k.range("screen.lpi", s.lpi, 5.0, 150.0, " lpi");
    if k.range("screen.angle_deg", s.angle_deg, 0.0, 90.0, "°") && s.angle_deg >= 90.0 {
        k.error(
            "screen.angle_deg",
            "must be below 90° (a square screen repeats every 90°)".into(),
        );
    }
    k.range("screen.mesh_ratio", s.mesh_ratio, 3.0, 8.0, "");
    if let Some(mesh) = s.mesh_count
        && k.range("screen.mesh_count", mesh, 40.0, 500.0, "")
        && lpi_ok
    {
        let (lo, hi) = (mesh / 5.0, mesh / 4.0);
        if s.lpi > hi * 1.0001 {
            k.warn(
                "screen.lpi",
                format!(
                    "{} lpi is finer than the advisory {lo:.1}..{hi:.1} lpi for {mesh} mesh; dots may not hold",
                    s.lpi
                ),
            );
        }
    }
}

fn tone(k: &mut Checker, c: &Config) {
    let t = &c.tone;
    let b = k.range("tone.black_point", t.black_point, 0.0, 1.0, "");
    let w = k.range("tone.white_point", t.white_point, 0.0, 1.0, "");
    if b && w && t.black_point >= t.white_point {
        k.error(
            "tone.black_point",
            format!(
                "must be below tone.white_point ({} >= {})",
                t.black_point, t.white_point
            ),
        );
    }
    k.range("tone.gamma", t.gamma, 0.1, 10.0, "");
    k.range("tone.max_coverage", t.max_coverage, 0.05, 1.0, "");
    if k.range("tone.min_dot_mm", t.min_dot_mm, 0.0, 5.0, " mm") && c.screen.lpi.is_finite() && c.screen.lpi > 0.0 {
        let largest = pitch_mm(c.screen.lpi) * std::f64::consts::SQRT_2;
        if t.min_dot_mm >= largest {
            k.error(
                "tone.min_dot_mm",
                format!(
                    "{} mm is not smaller than the largest possible dot ({largest:.3} mm at {} lpi)",
                    t.min_dot_mm, c.screen.lpi
                ),
            );
        }
    }
    k.color("tone.analysis_background", &t.analysis_background);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_normalize() {
        assert_eq!(normalize_angle(22.5), 22.5);
        assert_eq!(normalize_angle(112.5), 22.5);
        assert_eq!(normalize_angle(-67.5), 22.5);
        assert_eq!(normalize_angle(90.0), 0.0);
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#00ff80"), Some([0, 255, 128]));
        assert_eq!(parse_color("000000"), None);
        assert_eq!(parse_color("#12345"), None);
    }
}
