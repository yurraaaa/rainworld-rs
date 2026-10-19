//! The `AmbientSounds:` line of room settings files (`RoomSettings.LoadAmbientSounds`,
//! `OmniDirectionalSound`, `DirectionalSound`, `SpotSound`; `rainworld-re/notes/sound.md`
//! section 2.5). Entries are `", "`-separated, fields `"><"`-separated.
//!
//! Mirrors: RoomSettings.cs (LoadAmbientSounds), OmniDirectionalSound.cs, DirectionalSound.cs,
//!   SpotSound.cs

use crate::SoundTable;

/// How an ambient sound is placed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmbientKind {
    /// `OMNI`: constant and centred.
    Omni,
    /// `DIR`: constant, panned by `direction.x` (`Custom.DegToVec` of the angle).
    Directional { direction: [f32; 2] },
    /// `SPOT`: positional, full volume within `rad` of `pos`, fading out to
    /// [`AmbientSound::taper_rad`].
    Spot { pos: [f32; 2], rad: f32, taper: f32 },
}

/// One ambient sound of a room or template.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbientSound {
    /// File name in the ambient bundle, with `.ogg`.
    pub sample: String,
    pub volume: f32,
    pub pitch: f32,
    /// 0 for `OMNI` and for old `DIR`/`SPOT` lines without the field.
    pub doppler_fac: f32,
    pub kind: AmbientKind,
}

/// The `"><"`-separated fields of one entry.
struct AmbientFields<'a> {
    entry: &'a str,
    s: Vec<&'a str>,
}

impl AmbientFields<'_> {
    fn float(&self, i: usize) -> Result<f32, String> {
        let value = self
            .s
            .get(i)
            .ok_or_else(|| format!("{:?}: no field {i}", self.entry))?;
        SoundTable::float(value)
    }
}

impl AmbientSound {
    /// `SPOT` defaults when the line has no doppler and taper fields.
    const DEFAULT_TAPER: f32 = 0.1;

    /// Parses the value of an `AmbientSounds:` line; unknown types and empty entries
    /// are skipped as in the game.
    ///
    /// # Errors
    /// A message where the game would throw (missing field, bad number).
    pub fn parse_list(value: &str) -> Result<Vec<Self>, String> {
        let spaced = if value.ends_with(',') {
            format!("{value} ")
        } else {
            value.to_owned()
        };
        spaced
            .split(", ")
            .filter_map(|entry| Self::parse(entry).transpose())
            .collect()
    }

    fn parse(entry: &str) -> Result<Option<Self>, String> {
        let fields = AmbientFields {
            entry,
            s: entry.split("><").collect(),
        };
        let s = &fields.s;
        let kind = match s[0] {
            "OMNI" => AmbientKind::Omni,
            "DIR" => {
                let rad = fields.float(6)?.to_radians();
                AmbientKind::Directional {
                    direction: [rad.sin(), rad.cos()],
                }
            }
            "SPOT" => {
                let handle = [fields.float(8)?, fields.float(9)?];
                AmbientKind::Spot {
                    pos: [fields.float(6)?, fields.float(7)?],
                    rad: handle[0].hypot(handle[1]),
                    taper: if s.len() > 10 {
                        fields.float(11)?
                    } else {
                        Self::DEFAULT_TAPER
                    },
                }
            }
            _ => return Ok(None),
        };
        let doppler_fac = match kind {
            AmbientKind::Omni => 0.0,
            AmbientKind::Directional { .. } if s.len() > 7 => fields.float(7)?,
            AmbientKind::Spot { .. } if s.len() > 10 => fields.float(10)?,
            _ => 0.0,
        };
        Ok(Some(Self {
            sample: (*s.get(1).ok_or("ambient sound without sample")?).to_owned(),
            volume: fields.float(4)?,
            pitch: fields.float(5)?,
            doppler_fac,
            kind,
        }))
    }

    /// `SpotSound.TaperRad`: where a spot sound falls silent.
    #[must_use]
    pub fn taper_rad(&self) -> Option<f32> {
        match self.kind {
            AmbientKind::Spot { rad, taper, .. } => Some(rad + (rad * 2.0 + 1000.0) * taper),
            _ => None,
        }
    }

    /// Whether two entries are the same sound for inheritance: sample and type.
    #[must_use]
    pub fn same_sound(&self, other: &Self) -> bool {
        self.sample == other.sample
            && std::mem::discriminant(&self.kind) == std::mem::discriminant(&other.kind)
    }

    /// `RoomSettings.InheritAmbientSounds`: the room's own entries, then each parent
    /// entry the room does not override with the same sample and type.
    #[must_use]
    pub fn inherit(own: &[Self], parent: &[Self]) -> Vec<Self> {
        let mut all = own.to_vec();
        all.extend(
            parent
                .iter()
                .filter(|p| !own.iter().any(|o| o.same_sound(p)))
                .cloned(),
        );
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_three_types() {
        let list = AmbientSound::parse_list(
            "OMNI><AM_ENV-Forest.ogg><10><20><0.5><1, \
             DIR><wind.ogg><0><0><0.3><1.1><90><0.5><extra, \
             SPOT><drip.ogg><0><0><0.8><1><100><200><30><40><0.2><0.5, \
             SPOT><old.ogg><0><0><1><1><5><6><3><4, NOPE><x, ",
        )
        .unwrap();
        assert_eq!(list.len(), 4);
        assert_eq!(list[0].kind, AmbientKind::Omni);
        assert_eq!(
            (list[0].sample.as_str(), list[0].volume),
            ("AM_ENV-Forest.ogg", 0.5)
        );
        let AmbientKind::Directional { direction } = list[1].kind else {
            panic!()
        };
        assert!((direction[0] - 1.0).abs() < 1e-6 && direction[1].abs() < 1e-6);
        assert_eq!((list[1].pitch, list[1].doppler_fac), (1.1, 0.5));
        assert_eq!(
            list[2].kind,
            AmbientKind::Spot {
                pos: [100.0, 200.0],
                rad: 50.0,
                taper: 0.5
            }
        );
        assert_eq!(list[2].doppler_fac, 0.2);
        assert_eq!(list[2].taper_rad(), Some(50.0 + 1100.0 * 0.5));
        // Old spot lines: no doppler, default taper.
        assert_eq!(list[3].doppler_fac, 0.0);
        assert_eq!(list[3].taper_rad(), Some(5.0 + 1010.0 * 0.1));
    }

    #[test]
    fn missing_fields_fail() {
        assert!(AmbientSound::parse_list("OMNI><a.ogg><1><2><3").is_err());
        assert!(AmbientSound::parse_list("SPOT><a><0><0><1><1><5><6><3><4><0.1").is_err());
    }

    #[test]
    fn room_entries_override_parent_entries_of_the_same_type() {
        let parent = AmbientSound::parse_list(
            "OMNI><a.ogg><0><0><0.2><1, OMNI><b.ogg><0><0><0.3><1, DIR><a.ogg><0><0><1><1><0",
        )
        .unwrap();
        let own = AmbientSound::parse_list("OMNI><a.ogg><0><0><0.9><1").unwrap();
        let all = AmbientSound::inherit(&own, &parent);
        let summary: Vec<(&str, f32)> = all.iter().map(|a| (a.sample.as_str(), a.volume)).collect();
        assert_eq!(summary, [("a.ogg", 0.9), ("b.ogg", 0.3), ("a.ogg", 1.0)]);
    }
}
