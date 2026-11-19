//! Shared domain types for Halftone.
//!
//! Conventions used across the workspace:
//! - Configuration lengths are millimetres (`f64`, fields suffixed `_mm`).
//! - Geometry is integer micrometres (`i32`, 1 unit = 1 µm), x right, y down.
//! - Luminance is linear light, `0.0` = black, `1.0` = white.
//! - Coverage is the fraction of a screen cell covered by ink, `0.0..=1.0`.

pub mod config;
pub mod geometry;
pub mod limits;
pub mod paper;
pub mod units;
pub mod validate;

pub use config::Config;
pub use geometry::{Mark, MultiPolygon, Polygon, RegistrationTarget};
pub use units::Units;
pub use validate::{Issue, Severity};
