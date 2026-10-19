//! `HSLColor`, `Custom.HSL2RGB` and the menu palette (`Menu.MenuColor`).
//!
//! Mirrors: HSLColor.cs, RWCustom/Custom.cs (HSL2RGB), Menu/Menu.cs (MenuColor)

use rw_sim::FloatExt;

use crate::stage::StageColor;

/// `HSLColor`: hue wrapped into `[0, 1)`, saturation and lightness clamped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HslColor {
    pub hue: f32,
    pub saturation: f32,
    pub lightness: f32,
}

impl HslColor {
    #[must_use]
    pub fn new(hue: f32, saturation: f32, lightness: f32) -> Self {
        let mut hue = hue;
        if hue < 0.0 {
            hue += hue.floor() + 3.0;
        }
        Self {
            hue: hue - hue.floor(),
            saturation: saturation.clamp(0.0, 1.0),
            lightness: lightness.clamp(0.0, 1.0),
        }
    }

    /// `HSLColor.Lerp`: hue by `Mathf.LerpAngle` in degrees.
    #[must_use]
    pub fn lerp(self, to: Self, t: f32) -> Self {
        Self::new(
            Self::lerp_angle(self.hue * 360.0, to.hue * 360.0, t) / 360.0,
            self.saturation.lerp(to.saturation, t),
            self.lightness.lerp(to.lightness, t),
        )
    }

    /// `Mathf.LerpAngle`.
    fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
        let mut delta = Self::repeat(b - a, 360.0);
        if delta > 180.0 {
            delta -= 360.0;
        }
        a + delta * t.clamp(0.0, 1.0)
    }

    /// `Mathf.Repeat`.
    fn repeat(t: f32, length: f32) -> f32 {
        (t - (t / length).floor() * length).clamp(0.0, length)
    }

    /// `Custom.HSL2RGB`.
    #[must_use]
    pub fn rgb(self) -> StageColor {
        let (h, sl, l) = (self.hue, self.saturation, self.lightness);
        let v = if l <= 0.5 {
            l * (1.0 + sl)
        } else {
            l + sl - l * sl
        };
        if v <= 0.0 {
            return StageColor::rgb(l, l, l);
        }
        let m = l + l - v;
        let sv = (v - m) / v;
        let h = h * 6.0;
        let sextant = h as i32;
        let fract = h - sextant as f32;
        let vsf = v * sv * fract;
        let mid1 = m + vsf;
        let mid2 = v - vsf;
        let (r, g, b) = match sextant {
            0 => (v, mid1, m),
            1 => (mid2, v, m),
            2 => (m, v, mid1),
            3 => (m, mid2, v),
            4 => (mid1, m, v),
            5 => (v, m, mid2),
            _ => (l, l, l),
        };
        StageColor::rgb(r, g, b)
    }
}

/// `Menu.MenuColors`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuColor {
    White,
    MediumGrey,
    DarkGrey,
    VeryDarkGrey,
    Colored,
    Black,
    DarkRed,
    SaturatedGold,
}

impl MenuColor {
    /// `Menu.MenuColor`.
    #[must_use]
    pub fn hsl(self) -> HslColor {
        match self {
            Self::White | Self::Colored => HslColor::new(0.0, 0.0, 1.0),
            Self::MediumGrey => HslColor::new(0.730_555_5, 0.08, 0.67),
            Self::DarkGrey => HslColor::new(0.730_555_5, 0.08, 0.3),
            Self::VeryDarkGrey => HslColor::new(0.730_555_5, 0.08, 0.15),
            Self::Black => HslColor::new(0.730_555_5, 0.0, 0.0),
            Self::DarkRed => HslColor::new(1.0 / 30.0, 0.65, 0.235),
            Self::SaturatedGold => HslColor::new(17.0 / 180.0, 1.0, 0.685),
        }
    }

    /// `Menu.MenuRGB`.
    #[must_use]
    pub fn rgb(self) -> StageColor {
        self.hsl().rgb()
    }
}

impl StageColor {
    /// `Color.Lerp`: every channel, `t` clamped.
    #[must_use]
    pub fn lerp(self, to: Self, t: f32) -> Self {
        Self {
            r: self.r.lerp(to.r, t),
            g: self.g.lerp(to.g, t),
            b: self.b.lerp(to.b, t),
            a: self.a.lerp(to.a, t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_greys_are_bluish() {
        let c = MenuColor::MediumGrey.rgb();
        // l 0.67, s 0.08: v = l + s - l s = 0.6964, m = 0.6436.
        assert!((c.b - 0.6964).abs() < 1e-4, "{c:?}");
        assert!(c.b > c.g && c.r > c.g, "{c:?}");
        assert_eq!(MenuColor::White.rgb(), StageColor::WHITE);
        assert_eq!(MenuColor::Black.rgb(), StageColor::BLACK);
    }

    #[test]
    fn hue_lerp_takes_the_short_way_round() {
        let a = HslColor::new(0.9, 1.0, 0.5);
        let b = HslColor::new(0.1, 1.0, 0.5);
        let mid = a.lerp(b, 0.5);
        assert!(
            mid.hue.abs() < 1e-5 || (mid.hue - 1.0).abs() < 1e-5,
            "{mid:?}"
        );
    }
}
