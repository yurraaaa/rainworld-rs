//! A placed `LightFixture`'s data (`PlacedObject.LightFixtureData`).
//!
//! Mirrors: PlacedObject.cs (LightFixtureData)

use crate::identity::ext_enum;
use crate::placed::DataFields;

ext_enum! {
    /// `PlacedObject.LightFixtureData.Type`.
    pub enum LightFixtureType {
        Base {
            RedLight = "RedLight",
            HolyFire = "HolyFire",
            ZapCoilLight = "ZapCoilLight",
            DeepProcessing = "DeepProcessing",
            SlimeMoldLight = "SlimeMoldLight",
            RedSubmersible = "RedSubmersible",
            GlowWeedLight = "GlowWeedLight",
        }
    }
}

impl LightFixtureType {
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.value() == value)
    }
}

/// `PlacedObject.LightFixtureData`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightFixtureData {
    /// `type`; `None` for a value the game does not register (`Room.Loaded` matches none
    /// of its branches, so nothing is made).
    pub kind: Option<LightFixtureType>,
    pub panel_pos: [f32; 2],
    pub random_seed: i32,
}

impl LightFixtureData {
    /// `FromString`: `type~panelPos.x~panelPos.y~randomSeed[~...]`. `None` where the
    /// game's parsing would throw.
    #[must_use]
    pub fn parse(data: &str) -> Option<Self> {
        let f = DataFields(data.split('~').collect());
        Some(Self {
            kind: LightFixtureType::from_value(f.get(0)?),
            panel_pos: [f.float(1)?, f.float(2)?],
            random_seed: f.int(3)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{LightFixtureData, LightFixtureType};

    /// A red light and a holy fire as the rooms' settings write them, and a type the
    /// game does not register (`ss_c01`'s `LightRod`): every field read.
    #[test]
    fn parses_the_four_fields() {
        let d = LightFixtureData::parse("RedLight~17.32051~-10~79").expect("data");
        assert_eq!(d.kind, Some(LightFixtureType::RedLight));
        assert_eq!((d.panel_pos, d.random_seed), ([17.32051, -10.0], 79));
        let d = LightFixtureData::parse("HolyFire~-258.6795~102~47").expect("data");
        assert_eq!(d.kind, Some(LightFixtureType::HolyFire));
        assert_eq!((d.panel_pos, d.random_seed), ([-258.6795, 102.0], 47));
        let d = LightFixtureData::parse("LightRod~17.32051~-10~30").expect("data");
        assert_eq!((d.kind, d.random_seed), (None, 30));
    }

    /// Where the array index or `float.Parse`/`int.Parse` would throw.
    #[test]
    fn unreadable_data() {
        assert!(LightFixtureData::parse("RedLight~1~2").is_none());
        assert!(LightFixtureData::parse("RedLight~x~2~3").is_none());
        assert!(LightFixtureData::parse("RedLight~1~2~y").is_none());
    }
}
