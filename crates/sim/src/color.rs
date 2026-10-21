//! `RWCustom.Custom`'s colour and random-variation helpers: `HSL2RGB`,
//! `RandomDeviation`, `ClampedRandomVariation`, `WrappedRandomVariation`.
//!
//! Mirrors: RWCustom/Custom.cs (HSL2RGB, RandomDeviation, ClampedRandomVariation, WrappedRandomVariation)

use crate::float_ext::SCurve;
use rw_random::UnityRandom;

/// A colour as hue, saturation and lightness, each 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsl {
    pub h: f32,
    pub s: f32,
    pub l: f32,
}

impl Hsl {
    #[must_use]
    pub const fn new(h: f32, s: f32, l: f32) -> Self {
        Self { h, s, l }
    }

    /// `Custom.HSL2RGB`.
    #[must_use]
    pub fn rgb(self) -> [f32; 3] {
        let Self { h, s, l } = self;
        let mut rgb = [l; 3];
        let v = if f64::from(l) <= 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        if v > 0.0 {
            let m = l + l - v;
            let sv = (v - m) / v;
            let h = h * 6.0;
            let sextant = h as i32;
            let fract = h - sextant as f32;
            let vsf = v * sv * fract;
            let mid1 = m + vsf;
            let mid2 = v - vsf;
            rgb = match sextant {
                0 => [v, mid1, m],
                1 => [mid2, v, m],
                2 => [m, v, mid1],
                3 => [m, mid2, v],
                4 => [mid1, m, v],
                5 => [v, m, mid2],
                _ => rgb,
            };
        }
        rgb
    }
}

/// The random variations `Custom` draws from the global stream.
pub trait RandomVariation {
    /// `Custom.RandomDeviation` (two draws).
    fn random_deviation(&mut self, k: f32) -> f32;

    /// `Custom.ClampedRandomVariation`.
    fn clamped_random_variation(&mut self, base: f32, max_deviation: f32, k: f32) -> f32 {
        (base + self.random_deviation(k) * max_deviation).clamp(0.0, 1.0)
    }

    /// `Custom.WrappedRandomVariation`.
    fn wrapped_random_variation(&mut self, base: f32, max_deviation: f32, k: f32) -> f32 {
        let v = base + self.random_deviation(k) * max_deviation + 1.0;
        v - v.floor()
    }
}

impl RandomVariation for UnityRandom {
    fn random_deviation(&mut self, k: f32) -> f32 {
        let curve = (self.value() * 0.5).s_curve(k) * 2.0;
        curve * if self.value() < 0.5 { 1.0 } else { -1.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_to_rgb_matches_the_sextants() {
        assert_eq!(Hsl::new(0.0, 1.0, 0.5).rgb(), [1.0, 0.0, 0.0]);
        assert_eq!(Hsl::new(1.0 / 3.0, 1.0, 0.5).rgb()[1], 1.0);
        assert_eq!(Hsl::new(0.5, 0.0, 0.25).rgb(), [0.25, 0.25, 0.25]);
    }

    #[test]
    fn a_variation_makes_two_draws() {
        let mut rng = UnityRandom::new(5);
        let v = rng.wrapped_random_variation(0.32, 0.1, 0.6);
        let mut m = UnityRandom::new(5);
        m.value();
        m.value();
        assert_eq!(rng.state(), m.state());
        assert!((0.22..=0.42).contains(&v));
    }
}
