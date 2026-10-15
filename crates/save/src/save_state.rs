//! `SaveState`: one campaign's saved game (`SAVE STATE<progDivB>...`), entries split by
//! `<svA>`, key and value by `<svB>`.
//!
//! The port reads the fields its campaign start uses (`LoadGame`) and keeps every entry
//! as read: until hibernation is ported the port never writes a whole save state
//! (`SaveToString`), only its death-persistent data, which
//! `SaveDeathPersistentDataOfCurrentState` splices into the stored line.

use rw_assets::{ActiveMods, SlugcatName};

use crate::death_persistent::{DeathPersistentSaveData, ParseError};

#[derive(Debug, Clone, PartialEq)]
pub struct SaveState {
    pub save_state_number: SlugcatName,
    pub den_position: String,
    pub last_vanilla_den: String,
    pub cycle_number: i32,
    pub food: i32,
    /// `SEED`; a new state's is drawn by its constructor.
    pub seed: i32,
    /// `NEXTID` as read, which `LoadGame` gives the game's `nextIssuedId`.
    pub loaded_next_issued_id: Option<i32>,
    pub the_glow: bool,
    pub guide_overseer_dead: bool,
    pub malnourished: bool,
    red_extra_cycles: bool,
    pub death_persistent: DeathPersistentSaveData,
    /// Whether this state came from a stored line.
    pub loaded: bool,
}

impl SaveState {
    /// `RedsIllness.RedsCycles` with Remix off.
    #[must_use]
    pub fn reds_cycles(extra_cycles: bool) -> i32 {
        if extra_cycles { 24 } else { 19 }
    }

    /// `GetFinalFallbackShelter`.
    #[must_use]
    pub fn final_fallback_shelter(slugcat: SlugcatName) -> &'static str {
        if slugcat == SlugcatName::Red {
            "LF_S02"
        } else {
            "SU_S01"
        }
    }

    /// `BackwardsCompatibilityRemix.ParseSaveNumber` of a `SAVE STATE` line's value: a
    /// name, or an old save's player number. `None` for names the base game does not
    /// know, which match no campaign.
    #[must_use]
    pub fn parse_save_number(state: &str) -> Option<SlugcatName> {
        let first = state.split("<svA>").next().unwrap_or(state);
        let value = first.split("<svB>").nth(1)?;
        match value.trim().parse::<i32>() {
            Ok(n) => SlugcatName::from_player_number(n, ActiveMods::NONE),
            Err(_) => SlugcatName::ALL
                .iter()
                .copied()
                .find(|n| n.value() == value),
        }
    }

    /// `new SaveState(saveStateNumber, progression)` with every DLC off; `seed` is its
    /// `Random.Range(0, 10000)`.
    #[must_use]
    pub fn new(slugcat: SlugcatName, seed: i32) -> Self {
        Self {
            save_state_number: slugcat,
            den_position: String::new(),
            last_vanilla_den: String::new(),
            cycle_number: 0,
            food: 0,
            seed,
            loaded_next_issued_id: None,
            the_glow: false,
            guide_overseer_dead: false,
            malnourished: false,
            red_extra_cycles: false,
            death_persistent: DeathPersistentSaveData::new(slugcat),
            loaded: false,
        }
    }

    /// `LoadGame(str, game)` for a story game: an empty string starts clear
    /// (`setDenPosition`: `start_den`, also the last vanilla den when `vanilla`).
    ///
    /// # Errors
    /// On a malformed number, where the game's `int.Parse` throws.
    pub fn load_game(&mut self, s: &str, start_den: &str, vanilla: bool) -> Result<(), ParseError> {
        self.loaded = true;
        self.red_extra_cycles = false;
        if s.is_empty() {
            self.den_position = start_den.to_owned();
            if vanilla {
                self.last_vanilla_den = start_den.to_owned();
            }
            return Ok(());
        }
        for entry in s.split("<svA>") {
            let parts: Vec<&str> = entry.split("<svB>").collect();
            let value = parts.get(1).copied();
            match parts[0] {
                "DENPOS" => self.den_position = value.unwrap_or_default().to_owned(),
                "LASTVDENPOS" => self.last_vanilla_den = value.unwrap_or_default().to_owned(),
                "CYCLENUM" => self.cycle_number = ParseError::int("CYCLENUM", value)?,
                "FOOD" => self.food = ParseError::int("FOOD", value)?,
                "NEXTID" => self.loaded_next_issued_id = Some(ParseError::int("NEXTID", value)?),
                "SEED" => self.seed = ParseError::int("SEED", value)?,
                "HASTHEGLOW" => self.the_glow = true,
                "GUIDEOVERSEERDEAD" => self.guide_overseer_dead = true,
                "REDEXTRACYCLES" => self.red_extra_cycles = true,
                "DEATHPERSISTENTSAVEDATA" => {
                    self.death_persistent
                        .from_string(value.unwrap_or_default())?;
                }
                _ => {}
            }
        }
        if self.death_persistent.reds_death
            && self.cycle_number < Self::reds_cycles(self.red_extra_cycles())
        {
            self.death_persistent.reds_death = false;
        }
        Ok(())
    }

    /// `RedExtraCycles`.
    #[must_use]
    pub fn red_extra_cycles(&self) -> bool {
        self.red_extra_cycles || self.death_persistent.red_extra_cycles
    }

    /// `GetSaveStateDenToUse`: the den, else the last vanilla den, if the world has the
    /// room, else the final fallback shelter.
    #[must_use]
    pub fn den_to_use(&self, room_exists: impl Fn(&str) -> bool) -> String {
        [&self.den_position, &self.last_vanilla_den]
            .into_iter()
            .find(|d| !d.is_empty() && room_exists(d))
            .cloned()
            .unwrap_or_else(|| Self::final_fallback_shelter(self.save_state_number).to_owned())
    }

    /// `SlowFadeIn`: the game's fade-in seconds for a loaded state.
    #[must_use]
    pub fn slow_fade_in(&self) -> f32 {
        let base = if self.malnourished { 4.0 } else { 0.8 };
        let reds = Self::reds_cycles(self.red_extra_cycles());
        let red = if self.save_state_number == SlugcatName::Red && self.cycle_number >= reds {
            let t = ((self.cycle_number - reds) as f32 / 5.0).clamp(0.0, 1.0);
            4.0 + 11.0 * t
        } else {
            0.8
        };
        f32::max(base, red)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: &str = "SAV STATE NUMBER<svB>Yellow<svA>SEED<svB>77<svA>DENPOS<svB>SU_S03<svA>\
        LASTVDENPOS<svB>SU_S01<svA>CYCLENUM<svB>4<svA>FOOD<svB>2<svA>NEXTID<svB>1234<svA>\
        GUIDEOVERSEERDEAD<svA>DEATHPERSISTENTSAVEDATA<svB>REDSDEATH<dpA>KARMA<dpB>3<dpA>\
        KARMACAP<dpB>6<dpA><svA>HASROBO<svA><svA>";

    #[test]
    fn load_game_reads_the_start_fields() {
        assert_eq!(
            SaveState::parse_save_number(STATE),
            Some(SlugcatName::Yellow)
        );
        assert_eq!(
            SaveState::parse_save_number("SAV STATE NUMBER<svB>2<svA>"),
            Some(SlugcatName::Red)
        );
        assert_eq!(
            SaveState::parse_save_number("SAV STATE NUMBER<svB>Gourmand"),
            Some(SlugcatName::Gourmand)
        );
        let mut s = SaveState::new(SlugcatName::Yellow, 5);
        s.load_game(STATE, "SU_C04", true).unwrap();
        assert_eq!(
            (s.den_position.as_str(), s.cycle_number, s.food, s.seed),
            ("SU_S03", 4, 2, 77)
        );
        assert_eq!(s.loaded_next_issued_id, Some(1234));
        assert!(s.guide_overseer_dead);
        assert_eq!(
            (s.death_persistent.karma, s.death_persistent.karma_cap),
            (3, 6)
        );
        // A quit's REDSDEATH is cleared before the Hunter's last cycle.
        assert!(!s.death_persistent.reds_death);
        assert_eq!(s.den_to_use(|r| r == "SU_S01"), "SU_S01");
        assert_eq!(s.den_to_use(|_| false), "SU_S01");
        assert_eq!(s.slow_fade_in(), 0.8);
    }

    #[test]
    fn empty_load_starts_at_the_den() {
        let mut s = SaveState::new(SlugcatName::Red, 5);
        s.load_game("", "LF_H01", true).unwrap();
        assert_eq!(
            (s.den_position.as_str(), s.last_vanilla_den.as_str()),
            ("LF_H01", "LF_H01")
        );
        assert_eq!(
            (s.death_persistent.karma, s.death_persistent.the_mark),
            (2, true)
        );
        s.cycle_number = 21;
        assert!((s.slow_fade_in() - 8.4).abs() < 1e-5);
    }
}
