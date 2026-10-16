//! `SaveState`: one campaign's saved game (`SAVE STATE<progDivB>...`), entries split by
//! `<svA>`, key and value by `<svB>`.
//!
//! The base game's entries are typed or kept as their text, and `save_to_string` writes
//! them in the game's order; every other entry (DLC, Remix, mods) is kept as read and
//! written at the end, where the game writes `unrecognizedSaveStrings`.

use rw_assets::{ActiveMods, SlugcatName};

use crate::death_persistent::{DeathPersistentSaveData, ParseError};
use crate::dreams::DreamsState;
use crate::misc_world::MiscWorldSaveData;

/// `RainWorld.gameVersion`.
pub const GAME_VERSION: i32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct SaveState {
    pub save_state_number: SlugcatName,
    /// `currentTimelinePosition` as written (`TIMELINE`).
    pub timeline: String,
    pub den_position: String,
    pub last_vanilla_den: String,
    pub cycle_number: i32,
    pub food: i32,
    /// `SEED`; a new state's is drawn by its constructor.
    pub seed: i32,
    /// `NEXTID` as read, which `LoadGame` gives the game's `nextIssuedId`.
    pub loaded_next_issued_id: Option<i32>,
    /// `nextIssuedID`, written as `NEXTID` (set by `BringUpToDate`).
    pub next_issued_id: i32,
    pub game_version: i32,
    pub initiated_in_game_version: i32,
    /// `worldVersion` (`World/worldVersion.txt` for a new state).
    pub world_version: i32,
    pub the_glow: bool,
    pub guide_overseer_dead: bool,
    pub malnourished: bool,
    pub last_malnourished: bool,
    red_extra_cycles: bool,
    /// `respawnCreatures` and `waitRespawnCreatures`: spawner ids.
    pub respawn_creatures: Vec<i32>,
    pub wait_respawn_creatures: Vec<i32>,
    /// `creatureCommunitiesString`.
    pub communities: Option<String>,
    /// `REGIONSTATE` values: the regions' in `regions.txt` order, unknown regions last.
    pub region_states: Vec<String>,
    /// `SWALLOWEDITEMS`, `UNRECOGNIZEDSWALLOWED`, `PLAYERGRASPS` and
    /// `UNRECOGNIZEDPLAYERGRASPS` values as read; `Win` rewrites the first and third
    /// (`BringUpToDate`).
    pub swallowed_items: Option<String>,
    unrecognized_swallowed: Option<String>,
    pub player_grasps: Option<String>,
    unrecognized_player_grasps: Option<String>,
    pub death_persistent: DeathPersistentSaveData,
    pub misc_world: MiscWorldSaveData,
    /// The Survivor's and the Monk's.
    pub dreams: Option<DreamsState>,
    pub tot_food: i32,
    /// Seconds.
    pub tot_time: i32,
    pub cycles_in_current_world_version: i32,
    /// `kills`: each kill symbol (`IconSymbolData` as text) and how many.
    pub kills: Vec<(String, i32)>,
    /// `unrecognizedKills`: entries of creature types the game does not know.
    unrecognized_kills: Vec<String>,
    /// `skipNextCycleFoodDrain` (`JUSTBEATGAME`).
    pub skip_next_cycle_food_drain: bool,
    /// Every other entry, whole, in read order.
    unrecognized: Vec<String>,
    /// `deathPersistentSaveData.howWellIsPlayerDoing` after loading.
    how_well_is_player_doing: f32,
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
    /// `Random.Range(0, 10000)`. The world version is 0 until set from the game's file.
    #[must_use]
    pub fn new(slugcat: SlugcatName, seed: i32) -> Self {
        let dreams =
            matches!(slugcat, SlugcatName::White | SlugcatName::Yellow).then(DreamsState::default);
        Self {
            save_state_number: slugcat,
            timeline: slugcat.timeline().value().to_owned(),
            den_position: String::new(),
            last_vanilla_den: String::new(),
            cycle_number: 0,
            food: 0,
            seed,
            loaded_next_issued_id: None,
            next_issued_id: 0,
            game_version: GAME_VERSION,
            initiated_in_game_version: GAME_VERSION,
            world_version: 0,
            the_glow: false,
            guide_overseer_dead: false,
            malnourished: false,
            last_malnourished: false,
            red_extra_cycles: false,
            respawn_creatures: Vec::new(),
            wait_respawn_creatures: Vec::new(),
            communities: None,
            region_states: Vec::new(),
            swallowed_items: None,
            unrecognized_swallowed: None,
            player_grasps: None,
            unrecognized_player_grasps: None,
            death_persistent: DeathPersistentSaveData::new(slugcat),
            misc_world: MiscWorldSaveData::default(),
            dreams,
            tot_food: 0,
            tot_time: 0,
            cycles_in_current_world_version: 0,
            kills: Vec::new(),
            unrecognized_kills: Vec::new(),
            skip_next_cycle_food_drain: false,
            unrecognized: Vec::new(),
            how_well_is_player_doing: 0.0,
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
        self.initiated_in_game_version = 0;
        self.unrecognized.clear();
        if s.is_empty() {
            self.den_position = start_den.to_owned();
            if vanilla {
                self.last_vanilla_den = start_den.to_owned();
            }
            return Ok(());
        }
        for entry in s.split("<svA>") {
            let parts: Vec<&str> = entry.split("<svB>").collect();
            if parts[0].is_empty() {
                continue;
            }
            self.load_entry(entry, &parts)?;
        }
        if self.death_persistent.reds_death
            && self.cycle_number < Self::reds_cycles(self.red_extra_cycles())
        {
            self.death_persistent.reds_death = false;
        }
        Ok(())
    }

    fn load_entry(&mut self, entry: &str, parts: &[&str]) -> Result<(), ParseError> {
        let value = parts.get(1).copied();
        // Everything after the key, for the multi-value entries.
        let rest = entry.split_once("<svB>").map(|(_, r)| r.to_owned());
        match parts[0] {
            "SAV STATE NUMBER" => {}
            "TIMELINE" => self.timeline = value.unwrap_or_default().to_owned(),
            "DENPOS" => self.den_position = value.unwrap_or_default().to_owned(),
            "LASTVDENPOS" => self.last_vanilla_den = value.unwrap_or_default().to_owned(),
            "CYCLENUM" => self.cycle_number = ParseError::int("CYCLENUM", value)?,
            "FOOD" => self.food = ParseError::int("FOOD", value)?,
            "NEXTID" => {
                let id = ParseError::int("NEXTID", value)?;
                self.loaded_next_issued_id = Some(id);
                self.next_issued_id = id;
            }
            "SEED" => self.seed = ParseError::int("SEED", value)?,
            "VERSION" => self.game_version = ParseError::int("VERSION", value)?,
            "INITVERSION" => {
                self.initiated_in_game_version = ParseError::int("INITVERSION", value)?;
            }
            "WORLDVERSION" => self.world_version = ParseError::int("WORLDVERSION", value)?,
            "HASTHEGLOW" => self.the_glow = true,
            "GUIDEOVERSEERDEAD" => self.guide_overseer_dead = true,
            "REDEXTRACYCLES" => self.red_extra_cycles = true,
            "JUSTBEATGAME" => self.skip_next_cycle_food_drain = true,
            "RESPAWNS" => Self::load_ids(&mut self.respawn_creatures, value)?,
            "WAITRESPAWNS" => Self::load_ids(&mut self.wait_respawn_creatures, value)?,
            "REGIONSTATE" => self
                .region_states
                .push(value.unwrap_or_default().to_owned()),
            "COMMUNITIES" => self.communities = value.map(str::to_owned),
            "MISCWORLDSAVEDATA" => {
                self.misc_world = MiscWorldSaveData::from_string(value.unwrap_or_default());
            }
            "DREAMSSTATE" if self.dreams.is_some() => {
                if let Some(d) = &mut self.dreams {
                    d.from_string(value.unwrap_or_default());
                }
            }
            "TOTFOOD" => self.tot_food = ParseError::int("TOTFOOD", value)?,
            "TOTTIME" => self.tot_time = ParseError::int("TOTTIME", value)?,
            "CURRVERCYCLES" => {
                self.cycles_in_current_world_version = ParseError::int("CURRVERCYCLES", value)?;
            }
            "KILLS" => self.load_kills(value.unwrap_or_default())?,
            "SWALLOWEDITEMS" => self.swallowed_items = rest,
            "UNRECOGNIZEDSWALLOWED" => self.unrecognized_swallowed = rest,
            "PLAYERGRASPS" => self.player_grasps = rest,
            "UNRECOGNIZEDPLAYERGRASPS" => self.unrecognized_player_grasps = rest,
            "DEATHPERSISTENTSAVEDATA" => {
                self.death_persistent
                    .from_string(value.unwrap_or_default())?;
                self.how_well_is_player_doing = match self.save_state_number {
                    SlugcatName::Yellow => -1.0,
                    SlugcatName::Red => 1.0,
                    _ => self.death_persistent.how_well_is_player_doing(),
                };
            }
            _ => self.unrecognized.push(entry.to_owned()),
        }
        Ok(())
    }

    /// `KILLS`: entries `symbol<svD>count` by `<svC>`; a symbol whose creature type is
    /// not a base-game one is kept unrecognized. (The game also reads the symbols'
    /// numeric pre-1.9 form; here it stays unrecognized.)
    fn load_kills(&mut self, value: &str) -> Result<(), ParseError> {
        self.kills.clear();
        for entry in value.split("<svC>").filter(|e| !e.is_empty()) {
            let mut parts = entry.split("<svD>");
            let symbol = parts.next().unwrap_or_default();
            let kind = symbol.split('-').next().unwrap_or_default();
            if rw_assets::CreatureType::from_value(kind, ActiveMods::NONE).is_none() {
                self.unrecognized_kills.push(entry.to_owned());
                continue;
            }
            let count = ParseError::int("KILLS", parts.next())?;
            self.kills.push((symbol.to_owned(), count));
        }
        Ok(())
    }

    /// `AppendKills`: each kill of the cycle counts once more for its symbol.
    pub fn append_kills(&mut self, kills: &[crate::session::KillRecord]) {
        log::info!("appending {} kills to kills record", kills.len());
        for k in kills {
            let symbol = k.symbol();
            match self.kills.iter_mut().find(|(s, _)| *s == symbol) {
                Some((_, n)) => *n += 1,
                None => self.kills.push((symbol, 1)),
            }
        }
    }

    fn load_ids(ids: &mut Vec<i32>, value: Option<&str>) -> Result<(), ParseError> {
        for id in value.unwrap_or_default().split('.') {
            if !id.is_empty() {
                ids.push(ParseError::int("RESPAWNS", Some(id))?);
            }
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

    /// The session's `difficulty` (`StoryGameSession`'s constructor): from
    /// `howWellIsPlayerDoing` when the save had communities, else 0.
    #[must_use]
    pub fn session_difficulty(&self) -> f32 {
        if self.communities.is_none() {
            return 0.0;
        }
        let t = ((self.how_well_is_player_doing + 1.0) / 2.0).clamp(0.0, 1.0);
        -1.0 + 2.0 * t * t
    }

    /// Player `player`'s `swallowedItems` entry, unless empty or `"0"` (nothing swallowed).
    #[must_use]
    pub fn swallowed_item(&self, player: usize) -> Option<&str> {
        self.swallowed_items
            .as_deref()?
            .split("<svB>")
            .nth(player)
            .filter(|s| !s.is_empty() && *s != "0")
    }

    /// The saved `REGIONSTATE` of `region` (`regionLoadStrings`).
    #[must_use]
    pub fn region_state(&self, region: &str) -> Option<&str> {
        self.region_states
            .iter()
            .find(|s| Self::region_state_name(s).is_some_and(|n| n.eq_ignore_ascii_case(region)))
            .map(String::as_str)
    }

    /// `regionStates[region].roomsVisited`, as saved (`ROOMSVISITED`).
    #[must_use]
    pub fn rooms_visited(&self, region: &str) -> Vec<String> {
        self.region_state(region)
            .and_then(|s| {
                s.split("<rgA>")
                    .find_map(|e| e.strip_prefix("ROOMSVISITED<rgB>"))
            })
            .map(|v| {
                v.split(',')
                    .filter(|r| !r.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `REGIONNAME` of a region state's text.
    fn region_state_name(s: &str) -> Option<&str> {
        s.split("<rgA>")
            .find_map(|e| e.strip_prefix("REGIONNAME<rgB>"))
    }

    /// Stores `region`'s state: in place of its saved one, else among the regions in
    /// `region_order` (`regions.txt`), unknown regions after.
    pub fn set_region_state(&mut self, region: &str, value: String, region_order: &[String]) {
        if let Some(slot) = self
            .region_states
            .iter_mut()
            .find(|s| Self::region_state_name(s).is_some_and(|n| n.eq_ignore_ascii_case(region)))
        {
            *slot = value;
            return;
        }
        let rank = Self::region_rank(region, region_order);
        let at = self
            .region_states
            .iter()
            .position(|s| {
                Self::region_rank(Self::region_state_name(s).unwrap_or(""), region_order) > rank
            })
            .unwrap_or(self.region_states.len());
        self.region_states.insert(at, value);
    }

    fn region_rank(region: &str, region_order: &[String]) -> usize {
        region_order
            .iter()
            .position(|r| r.eq_ignore_ascii_case(region))
            .unwrap_or(region_order.len())
    }

    /// `SaveToString` with every DLC off and spawns not clean.
    #[must_use]
    pub fn save_to_string(&mut self) -> String {
        let mut t = String::new();
        let n = self.save_state_number.value();
        t.push_str(&format!("SAV STATE NUMBER<svB>{n}<svA>"));
        t.push_str(&format!("TIMELINE<svB>{}<svA>", self.timeline));
        t.push_str(&format!("SEED<svB>{}<svA>", self.seed));
        t.push_str(&format!("VERSION<svB>{}<svA>", self.game_version));
        t.push_str(&format!(
            "INITVERSION<svB>{}<svA>",
            self.initiated_in_game_version
        ));
        t.push_str(&format!("WORLDVERSION<svB>{}<svA>", self.world_version));
        t.push_str(&format!("DENPOS<svB>{}<svA>", self.den_position));
        t.push_str(&format!("LASTVDENPOS<svB>{}<svA>", self.last_vanilla_den));
        t.push_str(&format!("CYCLENUM<svB>{}<svA>", self.cycle_number));
        t.push_str(&format!("FOOD<svB>{}<svA>", self.food));
        t.push_str(&format!("NEXTID<svB>{}<svA>", self.next_issued_id));
        if self.the_glow {
            t.push_str("HASTHEGLOW<svA>");
        }
        if self.guide_overseer_dead {
            t.push_str("GUIDEOVERSEERDEAD<svA>");
        }
        Self::push_ids(&mut t, "RESPAWNS", &self.respawn_creatures);
        Self::push_ids(&mut t, "WAITRESPAWNS", &self.wait_respawn_creatures);
        Self::push_entry(&mut t, "COMMUNITIES", self.communities.as_deref());
        for r in &self.region_states {
            Self::push_entry(&mut t, "REGIONSTATE", Some(r));
        }
        Self::push_entry(&mut t, "SWALLOWEDITEMS", self.swallowed_items.as_deref());
        Self::push_entry(
            &mut t,
            "UNRECOGNIZEDSWALLOWED",
            self.unrecognized_swallowed.as_deref(),
        );
        Self::push_entry(&mut t, "PLAYERGRASPS", self.player_grasps.as_deref());
        Self::push_entry(
            &mut t,
            "UNRECOGNIZEDPLAYERGRASPS",
            self.unrecognized_player_grasps.as_deref(),
        );
        let dp = self.death_persistent.save_to_string(false, false);
        Self::push_entry(&mut t, "DEATHPERSISTENTSAVEDATA", Some(&dp));
        let misc = self.misc_world.save_string();
        if !misc.is_empty() {
            Self::push_entry(&mut t, "MISCWORLDSAVEDATA", Some(&misc));
        }
        if let Some(d) = &self.dreams {
            Self::push_entry(&mut t, "DREAMSSTATE", Some(&d.save_string()));
        }
        t.push_str(&format!("TOTFOOD<svB>{}<svA>", self.tot_food));
        t.push_str(&format!("TOTTIME<svB>{}<svA>", self.tot_time));
        t.push_str(&format!(
            "CURRVERCYCLES<svB>{}<svA>",
            self.cycles_in_current_world_version
        ));
        if !self.kills.is_empty() {
            let list: Vec<String> = self
                .kills
                .iter()
                .map(|(symbol, n)| format!("{symbol}<svD>{n}"))
                .chain(self.unrecognized_kills.iter().cloned())
                .collect();
            t.push_str(&format!("KILLS<svB>{}<svA>", list.join("<svC>")));
        }
        if self.red_extra_cycles {
            t.push_str("REDEXTRACYCLES<svA>");
        }
        if self.skip_next_cycle_food_drain {
            t.push_str("JUSTBEATGAME<svA>");
        }
        for u in &self.unrecognized {
            t.push_str(u);
            t.push_str("<svA>");
        }
        t
    }

    fn push_entry(t: &mut String, key: &str, value: Option<&str>) {
        if let Some(v) = value {
            t.push_str(key);
            t.push_str("<svB>");
            t.push_str(v);
            t.push_str("<svA>");
        }
    }

    fn push_ids(t: &mut String, key: &str, ids: &[i32]) {
        if ids.is_empty() {
            return;
        }
        let ids: Vec<String> = ids.iter().map(i32::to_string).collect();
        Self::push_entry(t, key, Some(&ids.join(".")));
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
    fn rooms_visited_reads_the_region_states_list() {
        let mut s = SaveState::new(SlugcatName::White, 5);
        s.region_states = vec![
            "REGIONNAME<rgB>SU<rgA>LASTCYCLEUPDATED<rgB>3<rgA>ROOMSVISITED<rgB>SU_C04,SU_A41<rgA>"
                .to_owned(),
            "REGIONNAME<rgB>HI<rgA>ROOMSVISITED<rgB><rgA>".to_owned(),
        ];
        assert_eq!(s.rooms_visited("su"), ["SU_C04", "SU_A41"]);
        assert!(s.rooms_visited("HI").is_empty());
        assert!(s.rooms_visited("DS").is_empty());
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

    #[test]
    fn written_in_the_game_order_with_unknown_entries_last() {
        let line = "SAV STATE NUMBER<svB>White<svA>SEED<svB>5206<svA>VERSION<svB>2<svA>\
            INITVERSION<svB>0<svA>WORLDVERSION<svB>2<svA>DENPOS<svB>CC_S04<svA>LASTVDENPOS<svB>CC_S04<svA>\
            CYCLENUM<svB>113<svA>FOOD<svB>3<svA>NEXTID<svB>28881<svA>HASTHEGLOW<svA>RESPAWNS<svB>5009.5020<svA>\
            COMMUNITIES<svB>SCAVSHY<coB>0<coA>All<coB>EVERY:0<svA>REGIONSTATE<svB>REGIONNAME<rgB>CC<rgA>X<rgB>1<rgA><svA>\
            SWALLOWEDITEMS<svB>ID.-1.2<oA>DataPearl<oA>CC_S04.26.24.0<svA>UNRECOGNIZEDSWALLOWED<svB>0<svB>0<svA>\
            DEATHPERSISTENTSAVEDATA<svB>REINFORCEDKARMA<dpB>0<dpA>KARMA<dpB>3<dpA>KARMACAP<dpB>9<dpA>GHOSTS<dpB><dpA>\
            TUTMESSAGES<dpB><dpA>METERSSHOWN<dpB><dpA>DEATHS<dpB>1<dpA>SURVIVES<dpB>2<dpA>QUITS<dpB>0<dpA><svA>\
            DREAMSSTATE<svB>integersArray<dsB>6.55.58.4.5.84.1.1.0<dsA><svA>TOTFOOD<svB>378<svA>\
            FORCEPUPS<svB>0<svA>TOTTIME<svB>34295<svA>CURRVERCYCLES<svB>93<svA>KILLS<svB>Fly-Creature-0<svD>25<svA>\
            OBJECTS<svB>ID.-1.19384<oA>DataPearl<svC><svA>";
        let mut s = SaveState::new(SlugcatName::White, 0);
        s.load_game(line, "SU_C04", true).unwrap();
        assert_eq!(s.respawn_creatures, [5009, 5020]);
        assert_eq!(
            s.region_state("cc"),
            Some("REGIONNAME<rgB>CC<rgA>X<rgB>1<rgA>")
        );
        assert_eq!(
            s.save_to_string(),
            "SAV STATE NUMBER<svB>White<svA>TIMELINE<svB>White<svA>SEED<svB>5206<svA>VERSION<svB>2<svA>\
            INITVERSION<svB>0<svA>WORLDVERSION<svB>2<svA>DENPOS<svB>CC_S04<svA>LASTVDENPOS<svB>CC_S04<svA>\
            CYCLENUM<svB>113<svA>FOOD<svB>3<svA>NEXTID<svB>28881<svA>HASTHEGLOW<svA>RESPAWNS<svB>5009.5020<svA>\
            COMMUNITIES<svB>SCAVSHY<coB>0<coA>All<coB>EVERY:0<svA>REGIONSTATE<svB>REGIONNAME<rgB>CC<rgA>X<rgB>1<rgA><svA>\
            SWALLOWEDITEMS<svB>ID.-1.2<oA>DataPearl<oA>CC_S04.26.24.0<svA>UNRECOGNIZEDSWALLOWED<svB>0<svB>0<svA>\
            DEATHPERSISTENTSAVEDATA<svB>REINFORCEDKARMA<dpB>0<dpA>KARMA<dpB>3<dpA>KARMACAP<dpB>9<dpA>GHOSTS<dpB><dpA>\
            TUTMESSAGES<dpB><dpA>METERSSHOWN<dpB><dpA>DEATHS<dpB>1<dpA>SURVIVES<dpB>2<dpA>QUITS<dpB>0<dpA><svA>\
            MISCWORLDSAVEDATA<svB>playerGuideState<mwB>integersArray<pgsB>0.0.0.0.0.0.0<pgsA>itemTypes<pgsB><pgsA>\
            creatureTypes<pgsB><pgsA>likesPlayer<pgsB>0.4<pgsA>directionHandHolding<pgsB>0.825<pgsA><mwA><svA>\
            DREAMSSTATE<svB>integersArray<dsB>6.55.58.4.5.84.1.1.0<dsA><svA>TOTFOOD<svB>378<svA>\
            TOTTIME<svB>34295<svA>CURRVERCYCLES<svB>93<svA>KILLS<svB>Fly-Creature-0<svD>25<svA>\
            FORCEPUPS<svB>0<svA>OBJECTS<svB>ID.-1.19384<oA>DataPearl<svC><svA>"
        );
    }

    #[test]
    fn region_states_go_in_region_order() {
        let order: Vec<String> = ["CC", "DS", "HI", "SU", "SL"].map(str::to_owned).to_vec();
        let mut s = SaveState::new(SlugcatName::White, 0);
        s.region_states = vec![
            "REGIONNAME<rgB>CC<rgA>".to_owned(),
            "REGIONNAME<rgB>SL<rgA>".to_owned(),
            "REGIONNAME<rgB>XX<rgA>".to_owned(),
        ];
        s.set_region_state("SU", "REGIONNAME<rgB>SU<rgA>A".to_owned(), &order);
        s.set_region_state("CC", "REGIONNAME<rgB>CC<rgA>B".to_owned(), &order);
        assert_eq!(
            s.region_states,
            [
                "REGIONNAME<rgB>CC<rgA>B",
                "REGIONNAME<rgB>SU<rgA>A",
                "REGIONNAME<rgB>SL<rgA>",
                "REGIONNAME<rgB>XX<rgA>"
            ]
        );
    }

    #[test]
    fn kills_are_appended_by_symbol_and_unknown_ones_kept() {
        // `KILLS<svB>symbol<svD>count<svC>...`: `AppendKills` counts each record once
        // more for its symbol; a type the game does not know stays as read, last.
        let line = "SAV STATE NUMBER<svB>White<svA>KILLS<svB>Fly-Creature-0<svD>3<svC>\
            Inspector-Creature-0<svD>1<svC>Overseer-Creature-0<svD>1<svA>";
        let mut s = SaveState::new(SlugcatName::White, 5);
        s.load_game(line, "SU_C04", true).unwrap();
        assert_eq!(
            s.kills,
            [
                ("Fly-Creature-0".to_owned(), 3),
                ("Overseer-Creature-0".to_owned(), 1)
            ]
        );
        let overseer =
            crate::session::KillRecord::new(rw_assets::CreatureType::Overseer, 0, 1, false);
        let lizard =
            crate::session::KillRecord::new(rw_assets::CreatureType::PinkLizard, 0, 2, true);
        assert_eq!(lizard.lizard_index, Some(1));
        s.append_kills(&[overseer, lizard]);
        assert!(s.save_to_string().contains(
            "KILLS<svB>Fly-Creature-0<svD>3<svC>Overseer-Creature-0<svD>2<svC>\
             PinkLizard-Creature-0<svD>1<svC>Inspector-Creature-0<svD>1<svA>"
        ));
    }
}
