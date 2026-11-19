//! Tone response: linear luminance -> target ink coverage.
//!
//! 1. Normalize: n = clamp((Y - black_point) / (white_point - black_point), 0, 1)
//! 2. Gamma:     n' = n^(1/gamma)   (gamma > 1 lightens midtones)
//! 3. Direction: coverage = 1 - n'  (dark source -> large marks), or n' when inverted
//! 4. Cap:       coverage = min(coverage, max_coverage)
//!
//! With defaults, coverage = 1 - Y: the reflectance of a printed patch matches the
//! source's linear luminance (no dot gain). Print compensation is a later curve.

use halftone_types::config::Tone;

#[derive(Clone, Copy, Debug)]
pub struct ToneCurve {
    pub invert: bool,
    pub black_point: f64,
    pub white_point: f64,
    pub gamma: f64,
    pub max_coverage: f64,
}

impl From<&Tone> for ToneCurve {
    fn from(t: &Tone) -> Self {
        Self {
            invert: t.invert,
            black_point: t.black_point,
            white_point: t.white_point,
            gamma: t.gamma,
            max_coverage: t.max_coverage,
        }
    }
}

impl ToneCurve {
    pub fn coverage(&self, luminance: f64) -> f64 {
        let span = self.white_point - self.black_point;
        let n = ((luminance - self.black_point) / span).clamp(0.0, 1.0);
        let n = if self.gamma == 1.0 { n } else { n.powf(1.0 / self.gamma) };
        let c = if self.invert { n } else { 1.0 - n };
        c.clamp(0.0, self.max_coverage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn curve() -> ToneCurve {
        ToneCurve {
            invert: false,
            black_point: 0.0,
            white_point: 1.0,
            gamma: 1.0,
            max_coverage: 1.0,
        }
    }

    #[test]
    fn defaults_are_one_minus_luminance() {
        let c = curve();
        assert_eq!(c.coverage(0.0), 1.0);
        assert_eq!(c.coverage(1.0), 0.0);
        assert!((c.coverage(0.2159) - 0.7841).abs() < 1e-12);
    }

    #[test]
    fn invert_black_white_points_and_cap() {
        let inv = ToneCurve {
            invert: true,
            ..curve()
        };
        assert_eq!(inv.coverage(1.0), 1.0);
        let pts = ToneCurve {
            black_point: 0.1,
            white_point: 0.9,
            ..curve()
        };
        assert_eq!(pts.coverage(0.05), 1.0);
        assert_eq!(pts.coverage(0.95), 0.0);
        assert!((pts.coverage(0.5) - 0.5).abs() < 1e-12);
        let capped = ToneCurve {
            max_coverage: 0.9,
            ..curve()
        };
        assert_eq!(capped.coverage(0.0), 0.9);
        let light = ToneCurve { gamma: 2.0, ..curve() };
        assert!(light.coverage(0.25) < curve().coverage(0.25));
    }

    proptest! {
        #[test]
        fn monotonic_and_bounded(a in 0.0f64..1.0, b in 0.0f64..1.0, g in 0.2f64..5.0, inv: bool) {
            let c = ToneCurve { gamma: g, invert: inv, ..curve() };
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            let (cl, ch) = (c.coverage(lo), c.coverage(hi));
            prop_assert!((0.0..=1.0).contains(&cl) && (0.0..=1.0).contains(&ch));
            if inv { prop_assert!(cl <= ch) } else { prop_assert!(cl >= ch) }
        }
    }
}
