//! `DeathPersistentSaveData`: what a death does not undo (karma, the mark, passages,
//! deaths and quits), entries split by `<dpA>`, key and value by `<dpB>`.
//!
//! The fields the port uses are typed; the rest keep their text as read, in the slot
//! `SaveToString` writes them. Keys the base game does not read with every DLC off
//! (More Slugcats, Remix, Watcher, mods) are its `unrecognizedSaveStrings`, written
//! back at the end.

use rw_assets::SlugcatName;

use crate::win_state::WinState;

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
    pub songs_play_records: Option<String>,
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
            songs_play_records: None,
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
                "SONGSPLAYRECORDS" => self.songs_play_records = owned,
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
        Self::push_entry(
            &mut t,
            "SONGSPLAYRECORDS",
            self.songs_play_records.as_deref(),
        );
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
    fn reinforced_karma_is_spent_instead() {
        let mut d = loaded();
        d.reinforced_karma = true;
        let s = d.save_to_string(true, false);
        assert!(s.starts_with("REDSDEATH<dpA>REINFORCEDKARMA<dpB>0<dpA>KARMA<dpB>3<dpA>"));
        assert!(s.contains("QUITS<dpB>1<dpA>"));
    }
}
