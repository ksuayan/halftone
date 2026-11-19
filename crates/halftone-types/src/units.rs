//! Physical units. Millimetres are canonical; conversion happens only at the
//! CLI/JSON boundary (inches, centimetres) and at the geometry boundary (µm).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MM_PER_INCH: f64 = 25.4;
pub const UM_PER_MM: f64 = 1000.0;

/// Units a user may enter dimensions in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    In,
    Cm,
    Mm,
}

impl Units {
    pub fn to_mm(self, value: f64) -> f64 {
        match self {
            Units::In => value * MM_PER_INCH,
            Units::Cm => value * 10.0,
            Units::Mm => value,
        }
    }

    pub fn from_mm(self, mm: f64) -> f64 {
        match self {
            Units::In => mm / MM_PER_INCH,
            Units::Cm => mm / 10.0,
            Units::Mm => mm,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Units::In => "in",
            Units::Cm => "cm",
            Units::Mm => "mm",
        }
    }
}

impl std::str::FromStr for Units {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "in" | "inch" | "inches" => Ok(Units::In),
            "cm" => Ok(Units::Cm),
            "mm" => Ok(Units::Mm),
            other => Err(format!("unknown unit '{other}' (expected in, cm or mm)")),
        }
    }
}

/// Millimetres to integer micrometres. `None` if not finite or out of `i32` range.
pub fn mm_to_um(mm: f64) -> Option<i32> {
    let um = (mm * UM_PER_MM).round();
    (um.is_finite() && um >= i32::MIN as f64 && um <= i32::MAX as f64).then_some(um as i32)
}

pub fn um_to_mm(um: i32) -> f64 {
    um as f64 / UM_PER_MM
}

/// Pitch of a screen of `lpi` lines per inch, in millimetres.
pub fn pitch_mm(lpi: f64) -> f64 {
    MM_PER_INCH / lpi
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn known_conversions() {
        assert_eq!(Units::In.to_mm(1.0), 25.4);
        assert_eq!(Units::Cm.to_mm(2.5), 25.0);
        assert_eq!(mm_to_um(1.2345), Some(1235));
        assert_eq!(mm_to_um(f64::NAN), None);
        assert_eq!(mm_to_um(1e12), None);
        assert!((pitch_mm(22.0) - 1.154_545).abs() < 1e-6);
    }

    proptest! {
        #[test]
        fn round_trip(v in 0.0f64..10_000.0) {
            for u in [Units::In, Units::Cm, Units::Mm] {
                prop_assert!((u.from_mm(u.to_mm(v)) - v).abs() <= 1e-9 * v.max(1.0));
            }
        }

        #[test]
        fn um_round_trip_within_half_micron(mm in -2000.0f64..2000.0) {
            let um = mm_to_um(mm).unwrap();
            prop_assert!((um_to_mm(um) - mm).abs() <= 0.0005 + 1e-12);
        }
    }
}
