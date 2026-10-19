//! `DeathPersistentSaveData`: what a death does not undo (karma, the mark, passages,
//! deaths and quits), entries split by `<dpA>`, key and value by `<dpB>`.
//!
//! The fields the port uses are typed; the rest keep their text as read, in the slot
//! `SaveToString` writes them. Keys the base game does not read with every DLC off
//! (More Slugcats, Remix, Watcher, mods) are its `unrecognizedSaveStrings`, written
//! back at the end.
//!
//! Mirrors: DeathPersistentSaveData.cs

use rw_assets::SlugcatName;

use crate::coordinate::SavedCoordinate;
use crate::songs::SongPlayRecord;
use crate::win_state::{EndgameId, WinState};

/// `DeathPersistentSaveData.Tutorial` (base game).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tutorial {
    GoExplore,
    ScavToll,
    ScavMerchant,
    DangleFruitInWater,
    KarmaFlower,
    PoleMimic,
}

impl Tutorial {
    /// The digit order of the old format (`ParseTutorialMessages`).
    pub const LEGACY_ORDER: [Self; 6] = [
        Self::GoExplore,
        Self::ScavToll,
        Self::ScavMerchant,
        Self::DangleFruitInWater,
        Self::KarmaFlower,
        Self::PoleMimic,
    ];

    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::GoExplore => "GoExplore",
            Self::ScavToll => "ScavToll",
            Self::ScavMerchant => "ScavMerchant",
            Self::DangleFruitInWater => "DangleFruitInWater",
            Self::KarmaFlower => "KarmaFlower",
            Self::PoleMimic => "PoleMimic",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::LEGACY_ORDER.into_iter().find(|t| t.value() == value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeathPersistentSaveData {
    pub karma: i32,
    pub karma_cap: i32,
    pub reinforced_karma: bool,
    pub the_mark: bool,
    pub reds_death: bool,
    pub red_extra_cycles: bool,
    pub ascended: bool,
    /// `pebblesHasIncreasedRedsKarmaCap` (`PHIRKC`).
    pub pebbles_increased_reds_karma_cap: bool,
    /// `FLOWERPOS` as read.
    pub karma_flower_position: Option<String>,
    /// `GHOSTS`' value as read (always written).
    pub ghosts: String,
    pub songs_play_records: Vec<SongPlayRecord>,
    pub session_records: Option<String>,
    pub win_state: WinState,
    pub consumed_flowers: Option<String>,
    /// `TUTMESSAGES`' value (always written).
    pub tutorial_messages: String,
    /// `METERSSHOWN`' value (always written).
    pub meters_shown: String,
    pub food_replenish_bonus: i32,
    /// `DDWORLDVERSION`.
    pub world_version: i32,
    pub deaths: i32,
    pub survives: i32,
    pub quits: i32,
    pub unlocked_gates: Option<String>,
    pub death_positions: Option<String>,
    pub warp_points: Option<String>,
    pub new_warp_points: Option<String>,
    pub unrecognized: Vec<String>,
}

/// A malformed number in a save string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

impl ParseError {
    /// `int.Parse(s, NumberStyles.Any, InvariantCulture)` for the plain integers saves
    /// hold.
    pub fn int(key: &str, value: Option<&str>) -> Result<i32, Self> {
        value
            .map(str::trim)
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| Self(format!("{key}: not an integer: {value:?}")))
    }
}

impl DeathPersistentSaveData {
    /// The constructor with every DLC off, then `SaveState`'s for `slugcat`.
    #[must_use]
    pub fn new(slugcat: SlugcatName) -> Self {
        let mut d = Self {
            karma: 0,
            karma_cap: 4,
            reinforced_karma: false,
            the_mark: false,
            reds_death: false,
            red_extra_cycles: false,
            ascended: false,
            pebbles_increased_reds_karma_cap: false,
            karma_flower_position: None,
            ghosts: String::new(),
            songs_play_records: Vec::new(),
            session_records: None,
            win_state: WinState::default(),
            consumed_flowers: None,
            tutorial_messages: String::new(),
            meters_shown: String::new(),
            food_replenish_bonus: 0,
            world_version: 0,
            deaths: 0,
            survives: 0,
            quits: 0,
            // `CanUseUnlockedGates`: the Monk's list exists (empty) from the start.
            unlocked_gates: None,
            death_positions: None,
            warp_points: None,
            new_warp_points: None,
            unrecognized: Vec::new(),
        };
        if slugcat != SlugcatName::White && slugcat != SlugcatName::Yellow {
            // PoleMimicEverSeen, ScavTollMessage, KarmaFlowerMessage, GoExploreMessage.
            d.tutorial_messages = "PoleMimic,ScavToll,KarmaFlower,GoExplore".to_owned();
        }
        if slugcat == SlugcatName::Red {
            d.the_mark = true;
            d.karma = 2;
        }
        d
    }

    /// `FromString` onto the constructed data.
    ///
    /// # Errors
    /// On a malformed number, where the game's `int.Parse` throws.
    pub fn from_string(&mut self, s: &str) -> Result<(), ParseError> {
        self.reds_death = false;
        self.red_extra_cycles = false;
        self.ascended = false;
        self.pebbles_increased_reds_karma_cap = false;
        self.unrecognized.clear();
        for entry in s.split("<dpA>") {
            let parts: Vec<&str> = entry.split("<dpB>").collect();
            let value = parts.get(1).copied();
            let owned = value.map(str::to_owned);
            match parts[0] {
                "KARMA" => self.karma = ParseError::int("KARMA", value)?,
                "KARMACAP" => self.karma_cap = ParseError::int("KARMACAP", value)?,
                "REINFORCEDKARMA" => self.reinforced_karma = value == Some("1"),
                "FLOWERPOS" => self.karma_flower_position = owned,
                "GHOSTS" => self.ghosts = owned.unwrap_or_default(),
                "SONGSPLAYRECORDS" => {
                    SongPlayRecord::parse_into(value.unwrap_or(""), &mut self.songs_play_records)?
                }
                "SESSIONRECORDS" => self.session_records = owned,
                "WINSTATE" => self.win_state = WinState::from_string(value.unwrap_or("")),
                "CONSUMEDFLOWERS" => self.consumed_flowers = owned,
                "HASTHEMARK" => self.the_mark = true,
                "TUTMESSAGES" => self.tutorial_messages = owned.unwrap_or_default(),
                "METERSSHOWN" => self.meters_shown = owned.unwrap_or_default(),
                "FOODREPBONUS" => {
                    self.food_replenish_bonus = ParseError::int("FOODREPBONUS", value)?;
                }
                "DDWORLDVERSION" => self.world_version = ParseError::int("DDWORLDVERSION", value)?,
                "DEATHS" => self.deaths = ParseError::int("DEATHS", value)?,
                "SURVIVES" => self.survives = ParseError::int("SURVIVES", value)?,
                "QUITS" => self.quits = ParseError::int("QUITS", value)?,
                "DEATHPOSS" => self.death_positions = owned,
                "REDSDEATH" => self.reds_death = true,
                "ASCENDED" => self.ascended = true,
                "PHIRKC" => self.pebbles_increased_reds_karma_cap = true,
                "REDEXTRACYCLES" => self.red_extra_cycles = true,
                "UNLOCKEDGATES" => self.unlocked_gates = owned,
                "WARPPOINTS" => self.warp_points = owned,
                "NEWWARPPOINTS" => self.new_warp_points = owned,
                _ if !entry.trim().is_empty() => self.unrecognized.push(entry.to_owned()),
                _ => {}
            }
        }
        self.karma = self.karma.clamp(0, self.karma_cap.max(0));
        Ok(())
    }

    /// `SaveToString(saveAsIfPlayerDied, saveAsIfPlayerQuit)` with every DLC off.
    #[must_use]
    pub fn save_to_string(&self, died: bool, quit: bool) -> String {
        let mut t = String::new();
        if died || quit || self.reds_death {
            t.push_str("REDSDEATH<dpA>");
        }
        if self.ascended {
            t.push_str("ASCENDED<dpA>");
        }
        if died {
            t.push_str("REINFORCEDKARMA<dpB>0<dpA>");
            let karma = if self.reinforced_karma {
                self.karma
            } else {
                self.karma - 1
            };
            t.push_str(&format!("KARMA<dpB>{karma}<dpA>"));
        } else {
            let reinforced = if self.reinforced_karma { "1" } else { "0" };
            t.push_str(&format!(
                "REINFORCEDKARMA<dpB>{reinforced}<dpA>KARMA<dpB>{}<dpA>",
                self.karma
            ));
        }
        t.push_str(&format!("KARMACAP<dpB>{}<dpA>", self.karma_cap));
        if self.the_mark {
            t.push_str("HASTHEMARK<dpA>");
        }
        Self::push_entry(&mut t, "FLOWERPOS", self.karma_flower_position.as_deref());
        Self::push_entry(&mut t, "GHOSTS", Some(&self.ghosts));
        let songs = SongPlayRecord::save_string(&self.songs_play_records);
        Self::push_entry(&mut t, "SONGSPLAYRECORDS", songs.as_deref());
        Self::push_entry(&mut t, "SESSIONRECORDS", self.session_records.as_deref());
        let win = self.win_state.save_to_string(died);
        if !win.is_empty() {
            Self::push_entry(&mut t, "WINSTATE", Some(&win));
        }
        Self::push_entry(&mut t, "CONSUMEDFLOWERS", self.consumed_flowers.as_deref());
        Self::push_entry(&mut t, "TUTMESSAGES", Some(&self.tutorial_messages));
        Self::push_entry(&mut t, "METERSSHOWN", Some(&self.meters_shown));
        if self.food_replenish_bonus > 0 {
            t.push_str(&format!(
                "FOODREPBONUS<dpB>{}<dpA>",
                self.food_replenish_bonus
            ));
        }
        if self.world_version > 0 {
            t.push_str(&format!("DDWORLDVERSION<dpB>{}<dpA>", self.world_version));
        }
        t.push_str(&format!(
            "DEATHS<dpB>{}<dpA>SURVIVES<dpB>{}<dpA>QUITS<dpB>{}<dpA>",
            self.deaths,
            self.survives,
            self.quits + i32::from(quit)
        ));
        if self.pebbles_increased_reds_karma_cap {
            t.push_str("PHIRKC<dpA>");
        }
        if self.red_extra_cycles {
            t.push_str("REDEXTRACYCLES<dpA>");
        }
        Self::push_entry(
            &mut t,
            "UNLOCKEDGATES",
            self.unlocked_gates.as_deref().filter(|g| !g.is_empty()),
        );
        Self::push_entry(&mut t, "DEATHPOSS", self.death_positions.as_deref());
        Self::push_entry(&mut t, "WARPPOINTS", self.warp_points.as_deref());
        Self::push_entry(&mut t, "NEWWARPPOINTS", self.new_warp_points.as_deref());
        for u in &self.unrecognized {
            t.push_str(u);
            t.push_str("<dpA>");
        }
        t
    }

    /// `sessionTrackRecord` entries (`survived`/`travelled` digits and any extra text).
    fn session_record_list(&self) -> Vec<String> {
        self.session_records
            .as_deref()
            .unwrap_or("")
            .split("<dpC>")
            .filter(|r| !r.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// `sessionTrackRecord.Add(new SessionRecord(survived, travelled))`, dropping the
    /// oldest past 20.
    pub fn add_session_record(&mut self, survived: bool, travelled: bool) {
        let mut list = self.session_record_list();
        list.push(format!("{}{}", u8::from(survived), u8::from(travelled)));
        if list.len() > 20 {
            list.remove(0);
        }
        self.session_records = Some(list.join("<dpC>"));
    }

    /// `UpdateDynamicDifficulty` with Remix off: the survived minus the died sessions,
    /// mapped from -7..7 to -1..1.
    #[must_use]
    pub fn how_well_is_player_doing(&self) -> f32 {
        let list = self.session_record_list();
        if list.is_empty() {
            return 0.0;
        }
        let score: i32 = list
            .iter()
            .map(|r| if r.starts_with('1') { 1 } else { -1 })
            .sum();
        let t = ((score as f32 + 7.0) / 14.0).clamp(0.0, 1.0);
        -1.0 + 2.0 * t
    }

    /// `SessionEnded`'s death positions: each a cycle older (`abstractNode + 1`), gone
    /// at 7.
    pub fn age_death_positions(&mut self) {
        let Some(text) = &self.death_positions else {
            return;
        };
        let mut kept = Vec::new();
        for pos in text.split("<dpC>").filter(|p| !p.is_empty()) {
            let Some((head, node)) = pos.rsplit_once('.') else {
                kept.push(pos.to_owned());
                continue;
            };
            let Ok(node) = node.trim().parse::<i32>() else {
                kept.push(pos.to_owned());
                continue;
            };
            if node + 1 < 7 {
                kept.push(format!("{head}.{}", node + 1));
            }
        }
        self.death_positions = (!kept.is_empty()).then(|| kept.join("<dpC>"));
    }

    /// `deathPositions` as `FromString` reads them (unreadable ones left out).
    #[must_use]
    pub fn death_position_list(&self) -> Vec<SavedCoordinate> {
        self.death_positions
            .as_deref()
            .unwrap_or("")
            .split("<dpC>")
            .filter(|p| !p.is_empty())
            .filter_map(SavedCoordinate::from_string)
            .collect()
    }

    /// `karmaFlowerPosition`.
    #[must_use]
    pub fn karma_flower_coordinate(&self) -> Option<SavedCoordinate> {
        self.karma_flower_position
            .as_deref()
            .and_then(SavedCoordinate::from_string)
    }

    /// `endGameMetersEverShown`: the passage IDs as written (a DLC or mod passage
    /// stays a name), the old digit-string format read as the game reads it.
    #[must_use]
    pub fn meters_ever_shown(&self) -> Vec<String> {
        let s = &self.meters_shown;
        if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
            return s
                .chars()
                .zip(EndgameId::ALL)
                .filter(|(c, _)| *c != '0')
                .map(|(_, id)| id.value().to_owned())
                .collect();
        }
        s.split(',')
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// `tutorialMessages` as names (a DLC or mod one stays as written), the old
    /// digit-string format read as `ParseTutorialMessages` reads it.
    #[must_use]
    pub fn tutorial_message_list(&self) -> Vec<String> {
        let s = &self.tutorial_messages;
        if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
            return s
                .chars()
                .zip(Tutorial::LEGACY_ORDER)
                .filter(|(c, _)| *c != '0')
                .map(|(_, t)| t.value().to_owned())
                .collect();
        }
        s.split(',')
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// `tutorialMessages.Contains(tutorial)` (the `...Message` properties).
    #[must_use]
    pub fn tutorial_message(&self, tutorial: Tutorial) -> bool {
        self.tutorial_message_list()
            .iter()
            .any(|t| t == tutorial.value())
    }

    /// `SetTutorialValue`: written back as the game writes it.
    pub fn set_tutorial_message(&mut self, tutorial: Tutorial, flag: bool) {
        let mut list = self.tutorial_message_list();
        let at = list.iter().position(|t| t == tutorial.value());
        match at {
            None if flag => list.push(tutorial.value().to_owned()),
            Some(i) if !flag => {
                list.remove(i);
            }
            _ => {}
        }
        self.tutorial_messages = list.join(",");
    }

    /// `endGameMetersEverShown.Add`: written back as the game writes it.
    pub fn add_meter_shown(&mut self, id: EndgameId) {
        let mut shown = self.meters_ever_shown();
        shown.push(id.value().to_owned());
        self.meters_shown = shown.join(",");
    }

    /// `AddDeathPosition(room, pos)`: the tile `RoundToInt(pos / 20)` (y at least -50 px)
    /// at node 0, saved as `<room>.<x>.<y>.0`; `room` is the name `SaveToString` gives.
    pub fn add_death_position(&mut self, room: &str, pos: [f32; 2]) {
        let x = (pos[0] / 20.0).round_ties_even() as i32;
        let y = (pos[1].max(-50.0) / 20.0).round_ties_even() as i32;
        let entry = format!("{room}.{x}.{y}.0");
        self.death_positions = Some(match self.death_positions.take() {
            Some(text) if !text.is_empty() => format!("{text}<dpC>{entry}"),
            _ => entry,
        });
    }

    /// `TickFlowerDepletion(ticks)`: consumed flowers' waits (the last field) count
    /// down; those below 1 go.
    pub fn tick_flower_depletion(&mut self, ticks: i32) {
        let Some(text) = &self.consumed_flowers else {
            return;
        };
        let mut kept = Vec::new();
        for item in text.split("<dpC>").filter(|p| !p.is_empty()) {
            match item
                .rsplit_once('.')
                .and_then(|(h, w)| Some((h, w.trim().parse::<i32>().ok()?)))
            {
                Some((head, wait)) if wait > -1 => {
                    if wait - ticks >= 1 {
                        kept.push(format!("{head}.{}", wait - ticks));
                    }
                }
                _ => kept.push(item.to_owned()),
            }
        }
        self.consumed_flowers = (!kept.is_empty()).then(|| kept.join("<dpC>"));
    }

    fn push_entry(t: &mut String, key: &str, value: Option<&str>) {
        if let Some(v) = value {
            t.push_str(key);
            t.push_str("<dpB>");
            t.push_str(v);
            t.push_str("<dpA>");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAVED: &str = "REINFORCEDKARMA<dpB>0<dpA>KARMA<dpB>3<dpA>KARMACAP<dpB>5<dpA>\
        GHOSTS<dpB>CC:1<dpA>WINSTATE<dpB>Hunter<egA>0<egA>2<wsA><dpA>TUTMESSAGES<dpB>\
        GoExplore<dpA>METERSSHOWN<dpB><dpA>DEATHS<dpB>7<dpA>SURVIVES<dpB>9<dpA>\
        QUITS<dpB>1<dpA>DEATHTIME<dpB>55<dpA>MODKEY<dpB>x<dpA>";

    fn loaded() -> DeathPersistentSaveData {
        let mut d = DeathPersistentSaveData::new(SlugcatName::White);
        d.from_string(SAVED).unwrap();
        d
    }

    #[test]
    fn round_trip_keeps_unknown_keys() {
        let d = loaded();
        assert_eq!((d.karma, d.karma_cap, d.deaths, d.quits), (3, 5, 7, 1));
        assert_eq!(d.unrecognized, ["DEATHTIME<dpB>55", "MODKEY<dpB>x"]);
        assert_eq!(d.save_to_string(false, false), SAVED);
    }

    #[test]
    fn saved_as_death_and_quit() {
        let s = loaded().save_to_string(true, true);
        assert_eq!(
            s,
            "REDSDEATH<dpA>REINFORCEDKARMA<dpB>0<dpA>KARMA<dpB>2<dpA>KARMACAP<dpB>5<dpA>\
             GHOSTS<dpB>CC:1<dpA>WINSTATE<dpB>Hunter<egA>0<egA>1<wsA><dpA>TUTMESSAGES<dpB>\
             GoExplore<dpA>METERSSHOWN<dpB><dpA>DEATHS<dpB>7<dpA>SURVIVES<dpB>9<dpA>\
             QUITS<dpB>2<dpA>DEATHTIME<dpB>55<dpA>MODKEY<dpB>x<dpA>"
        );
        // Read back, the death save's REDSDEATH stays (the cycle check is SaveState's).
        let mut again = DeathPersistentSaveData::new(SlugcatName::White);
        again.from_string(&s).unwrap();
        assert!(again.reds_death);
        assert_eq!(again.karma, 2);
    }

    #[test]
    fn meters_shown_old_digits_and_names() {
        let mut d = loaded();
        assert!(d.meters_ever_shown().is_empty());
        d.meters_shown = "1001".to_owned();
        assert_eq!(d.meters_ever_shown(), ["Survivor", "Traveller"]);
        d.add_meter_shown(EndgameId::Monk);
        assert_eq!(d.meters_shown, "Survivor,Traveller,Monk");
        d.meters_shown = "Hunter,Nomad".to_owned();
        assert_eq!(d.meters_ever_shown(), ["Hunter", "Nomad"]);
    }

    #[test]
    fn death_positions_and_flower_read_as_coordinates() {
        let mut d = loaded();
        d.death_positions = Some("SU_A22.3.4.6<dpC>bad<dpC>SU_C04.10.2.0".to_owned());
        let list = d.death_position_list();
        assert_eq!(list.len(), 2);
        assert_eq!(
            (list[1].room.as_str(), list[1].x, list[1].node),
            ("SU_C04", 10, 0)
        );
        d.karma_flower_position = Some("SU_A41.12.5.-1".to_owned());
        assert_eq!(d.karma_flower_coordinate().map(|c| c.y), Some(5));
    }

    #[test]
    fn reinforced_karma_is_spent_instead() {
        let mut d = loaded();
        d.reinforced_karma = true;
        let s = d.save_to_string(true, false);
        assert!(s.starts_with("REDSDEATH<dpA>REINFORCEDKARMA<dpB>0<dpA>KARMA<dpB>3<dpA>"));
        assert!(s.contains("QUITS<dpB>1<dpA>"));
    }

    #[test]
    fn tutorial_messages_and_song_records() {
        let mut d = DeathPersistentSaveData::new(SlugcatName::White);
        d.from_string("TUTMESSAGES<dpB>000101<dpA>SONGSPLAYRECORDS<dpB>NA_01 - Proxima<dpD>2<dpA>")
            .unwrap();
        assert!(d.tutorial_message(Tutorial::DangleFruitInWater));
        assert!(!d.tutorial_message(Tutorial::KarmaFlower));
        d.set_tutorial_message(Tutorial::GoExplore, true);
        assert_eq!(
            d.tutorial_messages,
            "DangleFruitInWater,PoleMimic,GoExplore"
        );
        d.set_tutorial_message(Tutorial::PoleMimic, false);
        assert_eq!(d.tutorial_messages, "DangleFruitInWater,GoExplore");
        assert_eq!(d.songs_play_records[0].cycle_last_played, 2);
        assert!(
            d.save_to_string(false, false)
                .contains("SONGSPLAYRECORDS<dpB>NA_01 - Proxima<dpD>2<dpA>")
        );
    }
}
