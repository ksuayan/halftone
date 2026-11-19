//! Print profiles: conservative starting points, all advisory until confirmed by
//! the Experiment E print tests.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    ScreenPrint,
    Linocut,
}

pub struct Defaults {
    pub mesh_count: Option<f64>,
    pub mesh_ratio: f64,
    /// LPI when no mesh count is known.
    pub lpi: f64,
    pub angle_deg: f64,
    pub min_dot_mm: f64,
    pub max_coverage: f64,
    pub printer_margin_mm: f64,
    pub tolerance_um: f64,
    pub px_per_cell: f64,
    pub notes: &'static [&'static str],
}

impl Profile {
    pub fn name(self) -> &'static str {
        match self {
            Profile::ScreenPrint => "screen-print",
            Profile::Linocut => "linocut",
        }
    }

    pub fn defaults(self) -> Defaults {
        match self {
            Profile::ScreenPrint => Defaults {
                mesh_count: Some(110.0),
                mesh_ratio: 5.0,
                lpi: 22.0,
                angle_deg: 22.5,
                min_dot_mm: 0.13,
                max_coverage: 1.0,
                printer_margin_mm: 6.35,
                tolerance_um: 10.0,
                px_per_cell: 2.0,
                notes: &[
                    "LPI is mesh_count / mesh_ratio, a rule of thumb; confirm with a test print on your screen, ink and substrate.",
                    "min_dot_mm 0.13 (about 3 printer dots at 600 dpi) is a starting hypothesis; confirm with the test chart.",
                ],
            },
            Profile::Linocut => Defaults {
                mesh_count: None,
                mesh_ratio: 5.0,
                lpi: 12.0,
                angle_deg: 45.0,
                min_dot_mm: 0.5,
                max_coverage: 0.95,
                printer_margin_mm: 6.35,
                tolerance_um: 10.0,
                px_per_cell: 2.0,
                notes: &[
                    "Linocut values are provisional (decision D16): large minimum dot and capped shadows so features can be carved.",
                ],
            },
        }
    }
}

impl std::str::FromStr for Profile {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "screen-print" => Ok(Profile::ScreenPrint),
            "linocut" => Ok(Profile::Linocut),
            other => Err(format!("unknown profile '{other}' (expected screen-print or linocut)")),
        }
    }
}
