//! A placed `LightSource`'s data (`PlacedObject.LightSourceData`).
//!
//! Mirrors: PlacedObject.cs (LightSourceData)

use crate::identity::ext_enum;
use crate::placed::DataFields;

ext_enum! {
    /// `PlacedObject.LightSourceData.ColorType`.
    pub enum LightColorType {
        Base {
            Environment = "Environment",
            White = "White",
            EffectColor1 = "EffectColor1",
            EffectColor2 = "EffectColor2",
        }
    }
}

ext_enum! {
    /// `PlacedObject.LightSourceData.BlinkType`.
    pub enum LightBlinkType {
        Base {
            None = "None",
            Flash = "Flash",
            Fade = "Fade",
        }
    }
}

impl LightColorType {
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.value() == value)
    }

    /// `(int)colorType`: its `Index`, -1 for a value no mod registers.
    #[must_use]
    pub fn index(color: Option<Self>) -> i32 {
        color
            .and_then(|c| Self::ALL.iter().position(|&a| a == c))
            .and_then(|i| i32::try_from(i).ok())
            .unwrap_or(-1)
    }
}

impl LightBlinkType {
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.value() == value)
    }
}

/// `PlacedObject.LightSourceData`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightSourceData {
    pub strength: f32,
    /// `None` for a value no mod registers (it equals none of the game's).
    pub color_type: Option<LightColorType>,
    pub handle: [f32; 2],
    pub panel: [f32; 2],
    pub fade_with_sun: bool,
    pub flat: bool,
    /// `None` both for a missing field (the constructor's `None`) and an unknown
    /// value.
    pub blink_type: Option<LightBlinkType>,
    pub blink_rate: f32,
    pub night_light: bool,
}

impl LightSourceData {
    /// `FromString`: `strength~colorType~handle.x~handle.y~panel.x~panel.y~fadeWithSun~
    /// flat[~blinkType~blinkRate[~nightLight]]~...`. `None` where the game's parsing
    /// would throw.
    #[must_use]
    pub fn parse(data: &str) -> Option<Self> {
        let f = DataFields(data.split('~').collect());
        let (blink_type, blink_rate) = if f.0.len() > 8 {
            (LightBlinkType::from_value(f.get(8)?), f.float(9)?)
        } else {
            (Some(LightBlinkType::None), 0.0)
        };
        let night_light = if f.0.len() > 10 {
            f.int(10)? > 0
        } else {
            false
        };
        Some(Self {
            strength: f.float(0)?,
            color_type: LightColorType::from_value(f.get(1)?),
            handle: [f.float(2)?, f.float(3)?],
            panel: [f.float(4)?, f.float(5)?],
            fade_with_sun: f.int(6)? > 0,
            flat: f.int(7)? > 0,
            blink_type,
            blink_rate,
            night_light,
        })
    }

    /// `Rad`: the handle's length.
    #[must_use]
    pub fn rad(&self) -> f32 {
        (self.handle[0] * self.handle[0] + self.handle[1] * self.handle[1]).sqrt()
    }

    /// `Room.Loaded`'s `effectColor`: `Max(-1, (int)colorType - 2)` (0 and 1 for the
    /// two effect colours).
    #[must_use]
    pub fn effect_color(&self) -> i32 {
        (LightColorType::index(self.color_type) - 2).max(-1)
    }
}

#[cfg(test)]
mod tests {
    use super::{LightBlinkType, LightColorType, LightSourceData};

    /// `su_a34`'s first light (`su_a34_settings.txt`), a made-up light with a 3-4-5
    /// handle and a blinking night light: every field, rad the handle's length.
    #[test]
    fn parses_the_short_and_long_forms() {
        let d =
            LightSourceData::parse("0.3369565~Environment~0~100~-225~164.6025~1~0").expect("data");
        assert_eq!(
            (d.strength, d.rad(), d.panel),
            (0.3369565, 100.0, [-225.0, 164.6025])
        );
        let d = LightSourceData::parse("0.75~Environment~-40~-30~65~37.5~1~0").expect("data");
        assert_eq!(d.strength, 0.75);
        assert_eq!(d.color_type, Some(LightColorType::Environment));
        assert_eq!((d.handle, d.panel), ([-40.0, -30.0], [65.0, 37.5]));
        assert!(d.fade_with_sun && !d.flat && !d.night_light);
        assert_eq!(
            (d.blink_type, d.blink_rate),
            (Some(LightBlinkType::None), 0.0)
        );
        assert_eq!((d.rad(), d.effect_color()), (50.0, -1));
        let d = LightSourceData::parse("1~EffectColor2~0~100~0~0~0~1~Fade~0.5~1").expect("data");
        assert_eq!(
            (d.effect_color(), d.flat, d.fade_with_sun),
            (1, true, false)
        );
        assert_eq!(
            (d.blink_type, d.blink_rate),
            (Some(LightBlinkType::Fade), 0.5)
        );
        assert!(d.night_light);
    }

    /// `Max(-1, Index - 2)`: Environment and White -1, the effect colours 0 and 1, an
    /// unregistered value (`Index` -1) -1.
    #[test]
    fn effect_color_index() {
        let colors = [
            "Environment",
            "White",
            "EffectColor1",
            "EffectColor2",
            "Neon",
        ]
        .map(|c| LightSourceData::parse(&format!("1~{c}~0~100~0~0~1~0")).map(|d| d.effect_color()));
        assert_eq!(colors, [-1, -1, 0, 1, -1].map(Some));
    }

    /// Where `float.Parse`/`int.Parse` or the array index would throw.
    #[test]
    fn unreadable_data() {
        assert!(LightSourceData::parse("1~White~0~100~0~0~1").is_none());
        assert!(LightSourceData::parse("x~White~0~100~0~0~1~0").is_none());
        assert!(LightSourceData::parse("1~White~0~100~0~0~1~0~Flash").is_none());
    }
}
