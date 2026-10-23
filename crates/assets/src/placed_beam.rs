//! A placed `LightBeam`'s data (`LightBeam.LightBeamData`, a `QuadObjectData` with a
//! panel position, alpha, two colour mixes, the sun flag and the blink fields). A
//! `SpotLight`'s data is the plain [`QuadData`].
//!
//! Mirrors: LightBeam.cs (LightBeamData), PlacedObject.cs (QuadObjectData)

use crate::identity::ext_enum;
use crate::placed::{DataFields, QuadData};

ext_enum! {
    /// `LightBeam.LightBeamData.BlinkType`.
    pub enum BeamBlinkType {
        Base {
            None = "None",
            Flash = "Flash",
            Fade = "Fade",
        }
    }
}

impl BeamBlinkType {
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.value() == value)
    }
}

/// `LightBeam.LightBeamData`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightBeamData {
    pub quad: QuadData,
    pub panel_pos: [f32; 2],
    pub alpha: f32,
    /// Mix of the palette's lit colour toward white.
    pub color_a: f32,
    /// Mix toward the camera picture's colour at the beam's second corner.
    pub color_b: f32,
    pub sun: bool,
    /// `None` for a value no mod registers.
    pub blink_type: Option<BeamBlinkType>,
    pub blink_rate: f32,
    pub night_light: bool,
}

impl LightBeamData {
    /// `h0.x~h0.y~h1.x~h1.y~h2.x~h2.y~panel.x~panel.y~alpha~colorA~colorB[~sun[~blinkType~
    /// blinkRate~nightLight]]~...`: `sun` is `== "1"` when there are more than 11 fields,
    /// the blink fields (all three required) when there are more than 12. `None` where
    /// the game's parsing would throw. Constructor defaults: `sun`, no blink.
    #[must_use]
    pub fn parse(data: &str) -> Option<Self> {
        let f = DataFields(data.split('~').collect());
        let quad = QuadData::parse(data)?;
        let sun = if f.0.len() > 11 {
            f.get(11)? == "1"
        } else {
            true
        };
        let (blink_type, blink_rate, night_light) = if f.0.len() > 12 {
            (
                BeamBlinkType::from_value(f.get(12)?),
                f.float(13)?,
                f.int(14)? > 0,
            )
        } else {
            (Some(BeamBlinkType::None), 0.0, false)
        };
        Some(Self {
            quad,
            panel_pos: [f.float(6)?, f.float(7)?],
            alpha: f.float(8)?,
            color_a: f.float(9)?,
            color_b: f.float(10)?,
            sun,
            blink_type,
            blink_rate,
            night_light,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{BeamBlinkType, LightBeamData};

    /// A base-game line (11 fields plus `sun`): every field, the defaults for the
    /// blink fields it lacks.
    #[test]
    fn parses_a_base_line() {
        let d = LightBeamData::parse("-119~329~-98~342~40~0~-356~-226~0.4108694~0.2608696~0~1")
            .expect("data");
        assert_eq!(
            d.quad.handles,
            [[-119.0, 329.0], [-98.0, 342.0], [40.0, 0.0]]
        );
        assert_eq!(d.panel_pos, [-356.0, -226.0]);
        assert_eq!((d.alpha, d.color_a, d.color_b), (0.4108694, 0.2608696, 0.0));
        assert!(d.sun && !d.night_light);
        assert_eq!(
            (d.blink_type, d.blink_rate),
            (Some(BeamBlinkType::None), 0.0)
        );
    }

    /// `sun` is `== "1"`; the blink fields need all three; fewer than 11 fields throw.
    #[test]
    fn sun_blink_and_short_lines() {
        let d = LightBeamData::parse("0~1~2~3~4~5~6~7~0.5~0~0~0").expect("data");
        assert!(!d.sun);
        let d = LightBeamData::parse("0~1~2~3~4~5~6~7~0.5~0~0").expect("data");
        assert!(d.sun);
        let d = LightBeamData::parse("0~1~2~3~4~5~6~7~0.5~0~0~1~Fade~0.5~1").expect("data");
        assert_eq!(
            (d.blink_type, d.blink_rate, d.night_light),
            (Some(BeamBlinkType::Fade), 0.5, true)
        );
        assert!(LightBeamData::parse("0~1~2~3~4~5~6~7~0.5~0~0~1~Fade~0.5").is_none());
        assert!(LightBeamData::parse("0~1~2~3~4~5~6~7~0.5~0").is_none());
    }
}
