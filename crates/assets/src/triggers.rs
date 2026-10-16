//! Room settings `Triggers:` entries: `EventTrigger`, `SpotTrigger` and the
//! `TriggeredEvent`s they fire (`MusicEvent`, `StopMusicEvent`,
//! `ShowProjectedImageEvent` and the plain ones), as `RoomSettings.LoadTriggers` and
//! their `FromString`s read them. Behaviour: `rainworld-re/notes/music.md` section 4.2.
//!
//! A trigger is `<type><tA>key<tB>value<tA>...`; its event is `<type><eA>key<eB>value...`.
//! Only `Spot` makes a `SpotTrigger`; every other type (`SeeCreature` included) is a
//! plain `EventTrigger`.

use crate::identity::ext_enum;
use crate::{ActiveMods, SettingsFile, SlugcatName, Timeline};

ext_enum! {
    /// `EventTrigger.TriggerType`.
    pub enum TriggerType {
        Base {
            Spot = "Spot",
            SeeCreature = "SeeCreature",
            PreRegionBump = "PreRegionBump",
            RegionBump = "RegionBump",
        }
    }
}

ext_enum! {
    /// `TriggeredEvent.EventType`.
    pub enum EventType {
        Base {
            MusicEvent = "MusicEvent",
            StopMusicEvent = "StopMusicEvent",
            PoleMimicsSubtleReveal = "PoleMimicsSubtleReveal",
            ShowProjectedImageEvent = "ShowProjectedImageEvent",
            PickUpObjectInstruction = "PickUpObjectInstruction",
            RoomSpecificTextMessage = "RoomSpecificTextMessage",
            BringPlayerGuideToRoom = "BringPlayerGuideToRoom",
        }
    }
}

ext_enum! {
    /// `StopMusicEvent.Type`.
    pub enum StopMusicType {
        Base {
            AllSongs = "AllSongs",
            SpecificSong = "SpecificSong",
            AllButSpecific = "AllButSpecific",
        }
    }
}

impl TriggerType {
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.value() == value)
    }
}

impl EventType {
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.value() == value)
    }
}

impl StopMusicType {
    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.value() == value)
    }

    /// `BackwardsCompatibilityRemix.ParseStopMusicType`: 1 and 2 are the specific kinds,
    /// anything else every song.
    #[must_use]
    pub fn from_legacy(number: i32) -> Self {
        match number {
            1 => Self::SpecificSong,
            2 => Self::AllButSpecific,
            _ => Self::AllSongs,
        }
    }
}

/// `MusicEvent`: a room trigger's song request (`MusicPlayer.GameRequestsSong`).
#[derive(Debug, Clone, PartialEq)]
pub struct MusicEvent {
    pub song_name: String,
    pub prio: f32,
    pub max_threat_level: f32,
    /// `droneTolerance`: saved, never read by the game.
    pub drone_tolerance: f32,
    pub volume: f32,
    pub fade_in_time: f32,
    pub looped: bool,
    pub one_song_per_cycle: bool,
    pub stop_at_death: bool,
    pub stop_at_gate: bool,
    pub rooms_range: i32,
    /// Cycles before the song may play again; negative is never.
    pub cycles_rest: i32,
}

impl Default for MusicEvent {
    fn default() -> Self {
        Self {
            song_name: "NO SONG".to_owned(),
            prio: 0.5,
            max_threat_level: 1.0,
            drone_tolerance: 0.0,
            volume: 0.3,
            fade_in_time: 0.0,
            looped: false,
            one_song_per_cycle: false,
            stop_at_death: false,
            stop_at_gate: false,
            rooms_range: -1,
            cycles_rest: 5,
        }
    }
}

/// `StopMusicEvent` (`MusicPlayer.GameRequestsSongStop`).
#[derive(Debug, Clone, PartialEq)]
pub struct StopMusicEvent {
    pub song_name: String,
    pub prio: f32,
    pub fade_out_time: f32,
    /// `None` for a value the game does not register, which stops like `AllSongs`.
    pub stop_type: Option<StopMusicType>,
}

impl Default for StopMusicEvent {
    fn default() -> Self {
        Self {
            song_name: "NO SONG".to_owned(),
            prio: 1.0,
            fade_out_time: 200.0,
            stop_type: Some(StopMusicType::AllSongs),
        }
    }
}

/// `ShowProjectedImageEvent`: the guide shows the room's pictures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShowProjectedImageEvent {
    pub after_encounter: bool,
    pub only_when_showing_direction: bool,
    /// Saved, never read by the game.
    pub from_cycle: i32,
}

/// A trigger's `tEvent`.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggeredEvent {
    Music(MusicEvent),
    StopMusic(StopMusicEvent),
    ShowProjectedImage(ShowProjectedImageEvent),
    /// A plain `TriggeredEvent`; `None` for a type the game does not register.
    Plain(Option<EventType>),
}

impl TriggeredEvent {
    /// `type`.
    #[must_use]
    pub fn event_type(&self) -> Option<EventType> {
        match self {
            Self::Music(_) => Some(EventType::MusicEvent),
            Self::StopMusic(_) => Some(EventType::StopMusicEvent),
            Self::ShowProjectedImage(_) => Some(EventType::ShowProjectedImageEvent),
            Self::Plain(t) => *t,
        }
    }

    /// `EventTrigger.FromString`'s `event` value, split on `<eA>`.
    fn parse(value: &str) -> Result<Option<Self>, String> {
        if value == "NONE" {
            return Ok(None);
        }
        let fields: Vec<&str> = value.split("<eA>").collect();
        let kind = EventType::from_value(fields[0]);
        Ok(Some(match kind {
            Some(EventType::MusicEvent) => Self::Music(MusicEvent::parse(&fields)?),
            Some(EventType::StopMusicEvent) => Self::StopMusic(StopMusicEvent::parse(&fields)?),
            Some(EventType::ShowProjectedImageEvent) => {
                Self::ShowProjectedImage(ShowProjectedImageEvent::parse(&fields)?)
            }
            other => Self::Plain(other),
        }))
    }
}

/// One `key<eB>value` of an event.
struct EventField<'a> {
    key: &'a str,
    value: Option<&'a str>,
}

impl<'a> EventField<'a> {
    fn split(field: &'a str) -> Self {
        let mut parts = field.split("<eB>");
        Self {
            key: parts.next().unwrap_or(""),
            value: parts.next(),
        }
    }

    /// `array[1]`, which throws in the game when missing.
    fn value(&self) -> Result<&'a str, String> {
        self.value
            .ok_or_else(|| format!("event field {:?} without a value", self.key))
    }

    fn float(&self) -> Result<f32, String> {
        SettingsFile::float(self.value()?)
    }

    fn int(&self) -> Result<i32, String> {
        SettingsFile::int(self.value()?)
    }

    fn flag(&self) -> Result<bool, String> {
        Ok(self.value()? == "1")
    }
}

impl MusicEvent {
    fn parse(fields: &[&str]) -> Result<Self, String> {
        let mut e = Self::default();
        for field in fields {
            let f = EventField::split(field);
            match f.key {
                "songName" => f.value()?.clone_into(&mut e.song_name),
                "vol" => e.volume = f.float()?,
                "prio" => e.prio = f.float()?,
                "mxThrt" => e.max_threat_level = f.float()?,
                "loop" => e.looped = f.flag()?,
                "droneTol" => e.drone_tolerance = f.float()?,
                "rooms" => e.rooms_range = f.int()?,
                "rest" => e.cycles_rest = f.int()?,
                "fadeIn" => e.fade_in_time = f.float()?,
                "oneSongPerCycle" => e.one_song_per_cycle = f.flag()?,
                "stopAtDeath" => e.stop_at_death = f.flag()?,
                "stopAtGate" => e.stop_at_gate = f.flag()?,
                _ => {}
            }
        }
        Ok(e)
    }
}

impl StopMusicEvent {
    fn parse(fields: &[&str]) -> Result<Self, String> {
        let mut e = Self::default();
        for field in fields {
            let f = EventField::split(field);
            match f.key {
                "songName" => f.value()?.clone_into(&mut e.song_name),
                "prio" => e.prio = f.float()?,
                "type" => {
                    let v = f.value()?;
                    e.stop_type = match SettingsFile::int(v) {
                        Ok(n) => Some(StopMusicType::from_legacy(n)),
                        Err(_) => StopMusicType::from_value(v),
                    };
                }
                "fadeOut" => e.fade_out_time = f.float()?,
                _ => {}
            }
        }
        Ok(e)
    }
}

impl ShowProjectedImageEvent {
    fn parse(fields: &[&str]) -> Result<Self, String> {
        let mut e = Self::default();
        for field in fields {
            let f = EventField::split(field);
            match f.key {
                "afterEncounter" => e.after_encounter = f.flag()?,
                "fromCycle" => e.from_cycle = f.int()?,
                "onlyWhenShowingDirection" => e.only_when_showing_direction = f.flag()?,
                _ => {}
            }
        }
        Ok(e)
    }
}

/// One `key<tB>value[<tB>value]` of a trigger.
struct TriggerField<'a>(Vec<&'a str>);

impl<'a> TriggerField<'a> {
    /// `array[i]`, which throws in the game when missing.
    fn value(&self, i: usize) -> Result<&'a str, String> {
        self.0
            .get(i)
            .copied()
            .ok_or_else(|| format!("trigger field {:?} without a value", self.0[0]))
    }

    fn int(&self) -> Result<i32, String> {
        SettingsFile::int(self.value(1)?)
    }

    fn vector(&self) -> Result<[f32; 2], String> {
        Ok([
            SettingsFile::float(self.value(1)?)?,
            SettingsFile::float(self.value(2)?)?,
        ])
    }
}

/// A `SpotTrigger`'s circle, in room pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpotArea {
    pub pos: [f32; 2],
    /// `radHandlePosition`.
    pub handle: [f32; 2],
    /// `rad`: the handle's length.
    pub rad: f32,
}

impl Default for SpotArea {
    /// The constructor's handle, `DegToVec(-135) * 50`.
    fn default() -> Self {
        let d = std::f32::consts::FRAC_1_SQRT_2 * 50.0;
        Self {
            pos: [0.0, 0.0],
            handle: [-d, -d],
            rad: 50.0,
        }
    }
}

/// `EventTrigger` (a `SpotTrigger` when [`Self::spot`] is set).
#[derive(Debug, Clone, PartialEq)]
pub struct EventTrigger {
    /// `None` for a type the game does not register.
    pub trigger_type: Option<TriggerType>,
    pub event: Option<TriggeredEvent>,
    pub active_to_cycle: i32,
    pub active_from_cycle: i32,
    pub delay: i32,
    pub multi_use: bool,
    pub fire_chance: f32,
    /// The node the player must have entered by, -1 for any.
    pub entrance: i32,
    pub karma: i32,
    /// `slugcats`' value as written: excluded names separated by `|`, or the legacy
    /// digit string; `None` without the key.
    pub slugcats: Option<String>,
    pub panel_position: [f32; 2],
    pub spot: Option<SpotArea>,
}

impl EventTrigger {
    /// `new EventTrigger(type)`'s defaults.
    #[must_use]
    pub fn new(trigger_type: Option<TriggerType>) -> Self {
        Self {
            trigger_type,
            event: None,
            active_to_cycle: -1,
            active_from_cycle: 0,
            delay: 0,
            multi_use: false,
            fire_chance: 1.0,
            entrance: -1,
            karma: 0,
            slugcats: None,
            panel_position: [0.0, 0.0],
            spot: (trigger_type == Some(TriggerType::Spot)).then(SpotArea::default),
        }
    }

    /// `LoadTriggers`: the value split on `", "` after `ValidateSpacedDelimiter`; empty
    /// entries are skipped.
    ///
    /// # Errors
    /// On a malformed number, where the game throws.
    pub fn parse_list(value: &str) -> Result<Vec<Self>, String> {
        let spaced = if value.ends_with(',') {
            format!("{value} ")
        } else {
            value.to_owned()
        };
        spaced
            .split(", ")
            .filter(|s| !s.split("<tA>").next().unwrap_or("").is_empty())
            .map(Self::parse)
            .collect()
    }

    /// One trigger's `FromString` (and `SpotTrigger.FromString`'s `pos`/`handlePos`).
    ///
    /// # Errors
    /// On a malformed number.
    pub fn parse(s: &str) -> Result<Self, String> {
        let fields: Vec<&str> = s.split("<tA>").collect();
        let mut t = Self::new(TriggerType::from_value(fields[0]));
        for field in &fields {
            let f = TriggerField(field.split("<tB>").collect());
            match f.0[0] {
                "event" => t.event = TriggeredEvent::parse(f.value(1)?)?,
                "multiUse" => t.multi_use = f.value(1)? == "1",
                "toCycle" => t.active_to_cycle = f.int()?,
                "fromCycle" => t.active_from_cycle = f.int()?,
                "delay" => t.delay = f.int()?,
                "panPos" => t.panel_position = f.vector()?,
                "fireChance" => t.fire_chance = SettingsFile::float(f.value(1)?)?,
                "entrance" => t.entrance = f.int()?,
                "karma" => t.karma = f.int()?,
                "slugcats" => t.slugcats = Some(f.value(1)?.to_owned()),
                "pos" => {
                    if let Some(spot) = &mut t.spot {
                        spot.pos = f.vector()?;
                    }
                }
                "handlePos" => {
                    if let Some(spot) = &mut t.spot {
                        spot.handle = f.vector()?;
                    }
                }
                _ => {}
            }
        }
        if let Some(spot) = &mut t.spot {
            let [x, y] = spot.handle;
            spot.rad = (x * x + y * y).sqrt();
        }
        Ok(t)
    }

    /// `slugcats` with `mods` active: the legacy digits (`ParsePlayerAvailability`: digit
    /// i set allows White, Yellow, Red, Night), else every registered playable name not
    /// excluded; with More Slugcats the Survivor brings Inv.
    #[must_use]
    pub fn available_slugcats(&self, mods: ActiveMods) -> Vec<SlugcatName> {
        let list = self.slugcats.as_deref().unwrap_or("");
        let mut names: Vec<SlugcatName> =
            if !list.is_empty() && list.chars().all(|c| c.is_ascii_digit()) {
                const LEGACY: [SlugcatName; 4] = [
                    SlugcatName::White,
                    SlugcatName::Yellow,
                    SlugcatName::Red,
                    SlugcatName::Night,
                ];
                list.chars()
                    .zip(LEGACY)
                    .filter(|&(c, _)| c == '1')
                    .map(|(_, n)| n)
                    .collect()
            } else {
                let excluded: Vec<&str> = list.split('|').collect();
                SlugcatName::registered(mods)
                    .into_iter()
                    .filter(|n| !n.hidden_or_unplayable() && !excluded.contains(&n.value()))
                    .collect()
            };
        if mods.msc && names.contains(&SlugcatName::White) {
            names.push(SlugcatName::Sofanthiel);
        }
        names
    }

    /// `timelines.Contains(timeline)` (`RefreshTimelineList`).
    #[must_use]
    pub fn active_in(&self, timeline: Timeline, mods: ActiveMods) -> bool {
        self.available_slugcats(mods)
            .iter()
            .any(|n| n.timeline() == timeline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B04: &str = "Spot<tA>event<tB>MusicEvent<eA>songName<eB>RW_1 - Urban Jungle<eA>vol<eB>0.2999997<eA>prio<eB>0.5<eA>mxThrt<eB>1<eA>loop<eB>0<eA>droneTol<eB>0<eA>rooms<eB>-1<eA>rest<eB>5<eA>fadeIn<eB>45.65217<eA>oneSongPerCycle<eB>0<eA>stopAtDeath<eB>0<eA>stopAtGate<eB>0<tA>multiUse<tB>0<tA>toCycle<tB>-1<tA>fromCycle<tB>0<tA>delay<tB>0<tA>panPos<tB>611.3611<tB>594.4163<tA>fireChance<tB>0.4565217<tA>entrance<tB>-1<tA>karma<tB>0<tA>slugcats<tB><tA>pos<tB>1670.361<tB>255.4163<tA>handlePos<tB>-338.3553<tB>68.64465, ";

    #[test]
    fn su_b04_music_trigger() {
        let list = EventTrigger::parse_list(B04).expect("parses");
        assert_eq!(list.len(), 1);
        let t = &list[0];
        assert_eq!(t.trigger_type, Some(TriggerType::Spot));
        assert_eq!(t.fire_chance, 0.456_521_7);
        assert!(!t.multi_use);
        assert_eq!((t.entrance, t.karma, t.delay), (-1, 0, 0));
        let spot = t.spot.expect("spot");
        assert_eq!(spot.pos, [1670.361, 255.4163]);
        assert!((spot.rad - 345.249).abs() < 0.01, "{}", spot.rad);
        let Some(TriggeredEvent::Music(m)) = &t.event else {
            panic!("music event");
        };
        assert_eq!(m.song_name, "RW_1 - Urban Jungle");
        assert_eq!(
            (m.volume, m.prio, m.fade_in_time),
            (0.299_999_7, 0.5, 45.652_17)
        );
        assert_eq!((m.rooms_range, m.cycles_rest), (-1, 5));
        assert_eq!(
            t.available_slugcats(ActiveMods::NONE),
            [SlugcatName::White, SlugcatName::Yellow, SlugcatName::Red]
        );
    }

    #[test]
    fn plain_and_unknown_events_and_types() {
        let t = EventTrigger::parse(
            "RegionBump<tA>event<tB>PickUpObjectInstruction<tA>slugcats<tB>Red|Yellow",
        )
        .unwrap();
        assert_eq!(t.trigger_type, Some(TriggerType::RegionBump));
        assert_eq!(t.spot, None);
        assert_eq!(
            t.event,
            Some(TriggeredEvent::Plain(Some(
                EventType::PickUpObjectInstruction
            )))
        );
        assert_eq!(t.available_slugcats(ActiveMods::NONE), [SlugcatName::White]);
        assert!(!t.active_in(Timeline::Red, ActiveMods::NONE));
        let u =
            EventTrigger::parse("Odd<tA>event<tB>Strange<eA>x<eB>1<tA>slugcats<tB>0110").unwrap();
        assert_eq!(u.trigger_type, None);
        assert_eq!(u.event, Some(TriggeredEvent::Plain(None)));
        assert_eq!(
            u.available_slugcats(ActiveMods::NONE),
            [SlugcatName::Yellow, SlugcatName::Red]
        );
        let none = EventTrigger::parse("Spot<tA>event<tB>NONE").unwrap();
        assert_eq!(none.event, None);
        assert_eq!(none.spot.map(|s| s.rad), Some(50.0));
    }

    #[test]
    fn stop_music_types() {
        let t = EventTrigger::parse(
            "PreRegionBump<tA>event<tB>StopMusicEvent<eA>songName<eB>NO SONG<eA>prio<eB>1<eA>type<eB>2<eA>fadeOut<eB>81.5",
        )
        .unwrap();
        let Some(TriggeredEvent::StopMusic(s)) = t.event else {
            panic!("stop event");
        };
        assert_eq!(s.stop_type, Some(StopMusicType::AllButSpecific));
        assert_eq!(s.fade_out_time, 81.5);
        let u = EventTrigger::parse("Spot<tA>event<tB>StopMusicEvent<eA>type<eB>Whatever").unwrap();
        assert!(matches!(
            u.event,
            Some(TriggeredEvent::StopMusic(StopMusicEvent {
                stop_type: None,
                ..
            }))
        ));
    }

    #[test]
    fn bad_numbers_fail() {
        assert!(EventTrigger::parse("Spot<tA>delay<tB>x").is_err());
        assert!(EventTrigger::parse("Spot<tA>event<tB>MusicEvent<eA>vol").is_err());
    }
}
