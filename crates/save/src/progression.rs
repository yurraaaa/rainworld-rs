//! `PlayerProgression`: one save slot's data. Its `"save"` value is a checksum and the
//! progression lines, split by `<progDivA>` (`SAVE STATE`, `MAP`, `MAPUPDATE`,
//! `MISCPROG`, and the DLCs' `MAP_<slugcat>` lines), key and value by `<progDivB>`.
//!
//! Every change is written to the slot file at once (`UserData.WriteMode.Immediate`).
//! Lines the port does not own are kept as read (docs/DIVERGENCES.md).

use std::path::{Path, PathBuf};

use rw_assets::SlugcatName;

use crate::checksum::ChecksumSalt;
use crate::death_persistent::ParseError;
use crate::misc::MiscProgression;
use crate::save_state::SaveState;
use crate::user_data::{UserDataError, UserDataFile};

const SAVE_KEY: &str = "save";

#[derive(Debug)]
pub enum SaveError {
    Io(std::io::Error),
    Format(UserDataError),
    Parse(ParseError),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Format(e) => write!(f, "{e}"),
            Self::Parse(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<UserDataError> for SaveError {
    fn from(e: UserDataError) -> Self {
        Self::Format(e)
    }
}

impl From<ParseError> for SaveError {
    fn from(e: ParseError) -> Self {
        Self::Parse(e)
    }
}

/// Where a campaign starts and with what, for `GetOrInitiateSaveState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignStart<'a> {
    pub slugcat: SlugcatName,
    /// `GetStoryDenPosition`.
    pub den: &'a str,
    /// Whether `den` is also `lastVanillaDen`.
    pub vanilla: bool,
}

#[derive(Debug)]
pub struct PlayerProgression {
    /// The slot file; `None` keeps everything in memory.
    path: Option<PathBuf>,
    /// `saveFileDataInMemory`.
    file: UserDataFile,
    /// Without it the port cannot write saves the game accepts, so it writes none.
    salt: Option<ChecksumSalt>,
    pub current_save_state: Option<SaveState>,
    pub misc: MiscProgression,
    /// `mapDiscoveryTextures`: region to its texture as base64 PNG (owned by the HUD
    /// map; its lines are written by `SaveToDisk` with maps).
    pub map_discovery: Vec<(String, String)>,
    /// `mapLastUpdatedTime`.
    pub map_last_updated: Vec<(String, i64)>,
    /// `gameTinkeredWith`: a stored checksum did not match.
    pub game_tinkered_with: bool,
    /// `starvedSaveState`: the state a starved cycle set aside.
    pub starved_save_state: Option<SaveState>,
    /// `tempSheltersDiscovered`.
    temp_shelters_discovered: Vec<String>,
}

impl PlayerProgression {
    /// No file: nothing saved, nothing written.
    #[must_use]
    pub fn in_memory(salt: Option<ChecksumSalt>) -> Self {
        Self {
            path: None,
            file: UserDataFile::new(),
            salt,
            current_save_state: None,
            misc: MiscProgression::default(),
            map_discovery: Vec::new(),
            map_last_updated: Vec::new(),
            game_tinkered_with: false,
            starved_save_state: None,
            temp_shelters_discovered: Vec::new(),
        }
    }

    /// Reads the slot file at `path` (an absent file is a new, empty slot) and its
    /// `MISCPROG` (`LoadProgression`).
    ///
    /// # Errors
    /// When the file cannot be read or is not a save file.
    pub fn load(path: &Path, salt: Option<ChecksumSalt>) -> Result<Self, SaveError> {
        let mut p = Self::in_memory(salt);
        p.path = Some(path.to_owned());
        match std::fs::read(path) {
            Ok(bytes) => p.file = UserDataFile::parse(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        for line in p.prog_lines() {
            let parts: Vec<&str> = line.split("<progDivB>").collect();
            if parts[0] == "MISCPROG" {
                p.misc = MiscProgression::from_string(parts.get(1).unwrap_or(&""));
            }
        }
        Ok(p)
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// `HasSaveData`.
    #[must_use]
    pub fn has_save_data(&self) -> bool {
        self.file.contains(SAVE_KEY)
    }

    /// `GetProgLinesFromMemory`: the lines after the checksum, which is checked.
    pub fn prog_lines(&mut self) -> Vec<String> {
        let Some(text) = self.file.get(SAVE_KEY) else {
            return Vec::new();
        };
        if text.is_empty() {
            return Vec::new();
        }
        let (sum, rest) = text.split_at(text.len().min(32));
        if let Some(salt) = &self.salt
            && salt.md5_sum(rest) != sum
        {
            log::warn!("save checksum wrong");
            self.game_tinkered_with = true;
        }
        rest.split("<progDivA>").map(str::to_owned).collect()
    }

    /// The slugcats of the stored `SAVE STATE` lines (unchecked).
    #[must_use]
    pub fn stored_save_numbers(&self) -> Vec<SlugcatName> {
        let Some(text) = self.file.get(SAVE_KEY) else {
            return Vec::new();
        };
        text.get(32..)
            .unwrap_or("")
            .split("<progDivA>")
            .filter_map(|l| l.strip_prefix("SAVE STATE<progDivB>"))
            .filter_map(SaveState::parse_save_number)
            .collect()
    }

    /// The stored `SAVE STATE` line value of `slugcat`, if any.
    pub fn stored_state(&mut self, slugcat: SlugcatName) -> Option<String> {
        self.prog_lines().into_iter().find_map(|line| {
            let parts: Vec<&str> = line.split("<progDivB>").collect();
            (parts.len() == 2
                && parts[0] == "SAVE STATE"
                && SaveState::parse_save_number(parts[1]) == Some(slugcat))
            .then(|| parts[1].to_owned())
        })
    }

    /// `IsThereASavedGame`.
    pub fn is_there_a_saved_game(&mut self, slugcat: SlugcatName) -> bool {
        if self
            .current_save_state
            .as_ref()
            .is_some_and(|s| s.save_state_number == slugcat)
        {
            return true;
        }
        self.stored_state(slugcat).is_some()
    }

    /// The state the last cycle left in memory for `slugcat` (the current one, else a
    /// starved one `GetOrInitiateSaveState` takes up), which a new cycle reuses.
    #[must_use]
    pub fn state_in_memory(&self, slugcat: SlugcatName) -> Option<&SaveState> {
        self.current_save_state
            .as_ref()
            .or(self.starved_save_state.as_ref())
            .filter(|s| s.save_state_number == slugcat)
    }

    /// `MineForSaveData`'s source: the stored state of `slugcat` read as `LoadGame`
    /// reads it, without making it current.
    ///
    /// # Errors
    /// On a malformed stored state.
    pub fn peek_save_state(
        &mut self,
        start: CampaignStart<'_>,
    ) -> Result<Option<SaveState>, SaveError> {
        if let Some(s) = self.state_in_memory(start.slugcat) {
            return Ok(Some(s.clone()));
        }
        let Some(line) = self.stored_state(start.slugcat) else {
            return Ok(None);
        };
        let mut state = SaveState::new(start.slugcat, 0);
        state.load_game(&line, start.den, start.vanilla)?;
        Ok(Some(state))
    }

    /// `GetOrInitiateSaveState` for a story game: the stored state when `load` (the
    /// menu's `LoadInitCondition`) and one exists, else a new one with `seed`; with
    /// `save_as_death_or_quit`, then saved as a death and a quit, so a game that never
    /// exits cleanly counts as both.
    ///
    /// # Errors
    /// On a malformed stored state, or when the save file cannot be written.
    pub fn get_or_initiate_save_state(
        &mut self,
        start: CampaignStart<'_>,
        seed: i32,
        load: bool,
        save_as_death_or_quit: bool,
    ) -> Result<&mut SaveState, SaveError> {
        if self.current_save_state.is_none()
            && let Some(starved) = self.starved_save_state.take()
        {
            log::info!("loading the starved state");
            self.current_save_state = Some(starved);
        }
        let current = self
            .current_save_state
            .as_ref()
            .is_some_and(|s| s.save_state_number == start.slugcat);
        if !current {
            let mut state = SaveState::new(start.slugcat, seed);
            let line = if load {
                self.stored_state(start.slugcat)
            } else {
                None
            };
            state.load_game(line.as_deref().unwrap_or(""), start.den, start.vanilla)?;
            self.current_save_state = Some(state);
        }
        if save_as_death_or_quit {
            self.save_death_persistent_data_of_current_state(true, true)?;
        }
        Ok(self.current_save_state.as_mut().expect("set above"))
    }

    /// `SaveDeathPersistentDataOfCurrentState`: the current state's death-persistent
    /// data replaces that in its stored line, if it has one. Each call adds an empty
    /// `<svA>` entry to the line, as the game's does.
    ///
    /// # Errors
    /// When the save file cannot be written.
    pub fn save_death_persistent_data_of_current_state(
        &mut self,
        died: bool,
        quit: bool,
    ) -> Result<(), SaveError> {
        let Some(state) = &self.current_save_state else {
            log::warn!("no current save state to save death-persistent data of");
            return Ok(());
        };
        let slugcat = state.save_state_number;
        let data = state.death_persistent.save_to_string(died, quit);
        let mut text = String::new();
        for line in self.prog_lines() {
            let parts: Vec<&str> = line.split("<progDivB>").collect();
            if parts[0] == "SAVE STATE"
                && parts.get(1).and_then(|s| SaveState::parse_save_number(s)) == Some(slugcat)
            {
                for entry in line.split("<svA>") {
                    if entry.split("<svB>").next() == Some("DEATHPERSISTENTSAVEDATA") {
                        text.push_str("DEATHPERSISTENTSAVEDATA<svB>");
                        text.push_str(&data);
                    } else {
                        text.push_str(entry);
                    }
                    text.push_str("<svA>");
                }
                text.push_str("<progDivA>");
            } else if !line.is_empty() {
                text.push_str(&line);
                text.push_str("<progDivA>");
            }
        }
        self.store(text)
    }

    /// `SaveToDisk(false, saveMaps, saveMiscProg)`, as `SaveProgression` calls it.
    ///
    /// # Errors
    /// When the save file cannot be written.
    pub fn save_progression(&mut self, save_maps: bool, save_misc: bool) -> Result<(), SaveError> {
        self.save_to_disk(false, save_maps, save_misc)
    }

    /// `SaveWorldStateAndProgression(malnourished)`: a starved cycle saves only the
    /// progression and the death-persistent data (as a death) and sets the state aside
    /// (`starvedSaveState`); otherwise `SaveToDisk(true, true, true)`.
    ///
    /// # Errors
    /// When the save file cannot be written.
    pub fn save_world_state_and_progression(
        &mut self,
        malnourished: bool,
    ) -> Result<(), SaveError> {
        if malnourished {
            self.save_progression_and_death_persistent_data_of_current_state(true, false)?;
            self.starved_save_state = self.current_save_state.take();
            return Ok(());
        }
        self.save_to_disk(true, true, true)
    }

    /// `TempDiscoverShelter`: a shelter the player was spat out into, saved with the
    /// next map save.
    pub fn temp_discover_shelter(&mut self, shelter: &str) {
        if !self.temp_shelters_discovered.iter().any(|s| s == shelter) {
            self.temp_shelters_discovered.push(shelter.to_owned());
        }
    }

    /// `SaveToDisk(saveCurrentState, saveMaps, saveMiscProg)` (the mod-state check
    /// always passes: the port loads and saves with every DLC off).
    fn save_to_disk(
        &mut self,
        save_state: bool,
        save_maps: bool,
        save_misc: bool,
    ) -> Result<(), SaveError> {
        if !save_state && !save_maps && !save_misc {
            return Ok(());
        }
        if save_maps {
            let slugcat = self.misc.current_slugcat;
            for shelter in &self.temp_shelters_discovered {
                self.misc.save_discovered_shelter(shelter, slugcat);
            }
        }
        self.temp_shelters_discovered.clear();
        let current = if save_state {
            self.current_save_state
                .as_mut()
                .map(|s| (s.save_state_number, s.save_to_string()))
        } else {
            None
        };
        let mut state_written = false;
        let lines = self.prog_lines();
        let map_regions: Vec<String> = lines
            .iter()
            .filter_map(|l| l.strip_prefix("MAP<progDivB>"))
            .map(|rest| rest.split("<progDivB>").next().unwrap_or("").to_owned())
            .collect();
        let mut text = String::new();
        let mut misc_written = false;
        for line in lines {
            let parts: Vec<&str> = line.split("<progDivB>").collect();
            let region = parts.get(1).copied().unwrap_or("");
            match parts[0] {
                "MAP" => match self.map_texture(region).filter(|_| save_maps) {
                    Some(png) => {
                        text.push_str(&format!("MAP<progDivB>{region}<progDivB>{png}"));
                        if let Some(time) = self.map_update(region) {
                            text.push_str(&format!(
                                "<progDivA>MAPUPDATE<progDivB>{region}<progDivB>{time}"
                            ));
                        }
                    }
                    None => text.push_str(&line),
                },
                // The game drops every stored time; the port only those it rewrites.
                "MAPUPDATE" if self.writes_map_update(region, save_maps, &map_regions) => {
                    continue;
                }
                "MISCPROG" if save_misc => {
                    text.push_str("MISCPROG<progDivB>");
                    text.push_str(&self.misc.to_save_string());
                    misc_written = true;
                }
                "SAVE STATE"
                    if current
                        .as_ref()
                        .is_some_and(|(n, _)| SaveState::parse_save_number(region) == Some(*n)) =>
                {
                    text.push_str("SAVE STATE<progDivB>");
                    text.push_str(current.as_ref().map_or("", |(_, s)| s.as_str()));
                    state_written = true;
                }
                "" => continue,
                // Other states, another slugcat's maps, and what the port does not own.
                _ => text.push_str(&line),
            }
            text.push_str("<progDivA>");
        }
        if let Some((_, state)) = current.filter(|_| !state_written) {
            text.push_str("SAVE STATE<progDivB>");
            text.push_str(&state);
            text.push_str("<progDivA>");
        }
        if save_maps {
            for (region, png) in &self.map_discovery {
                if !map_regions.contains(region) {
                    text.push_str(&format!("MAP<progDivB>{region}<progDivB>{png}<progDivA>"));
                }
            }
            for (region, time) in &self.map_last_updated {
                if !map_regions.contains(region) {
                    text.push_str(&format!(
                        "MAPUPDATE<progDivB>{region}<progDivB>{time}<progDivA>"
                    ));
                }
            }
        }
        if save_misc && !misc_written {
            text.push_str("MISCPROG<progDivB>");
            text.push_str(&self.misc.to_save_string());
            text.push_str("<progDivA>");
        }
        self.store(text)
    }

    /// Whether `save_progression` writes `region`'s time: after its fresh `MAP` line,
    /// or with the times of regions without one.
    fn writes_map_update(&self, region: &str, save_maps: bool, map_regions: &[String]) -> bool {
        save_maps
            && self.map_update(region).is_some()
            && (self.map_texture(region).is_some() || !map_regions.iter().any(|r| r == region))
    }

    fn map_texture(&self, region: &str) -> Option<&str> {
        self.map_discovery
            .iter()
            .find(|(r, _)| r == region)
            .map(|(_, png)| png.as_str())
    }

    fn map_update(&self, region: &str) -> Option<i64> {
        self.map_last_updated
            .iter()
            .find(|(r, _)| r == region)
            .map(|(_, t)| *t)
    }

    /// `LoadMapTexture`'s source: the stored `MAP` texture (base64 PNG) and
    /// `MAPUPDATE` time of `region`.
    pub fn stored_map(&mut self, region: &str) -> (Option<String>, Option<i64>) {
        let mut texture = None;
        let mut time = None;
        for line in self.prog_lines() {
            let parts: Vec<&str> = line.split("<progDivB>").collect();
            if parts.get(1) != Some(&region) {
                continue;
            }
            match parts[0] {
                "MAP" if texture.is_none() => texture = parts.get(2).map(|s| (*s).to_owned()),
                "MAPUPDATE" if time.is_none() => {
                    time = parts.get(2).and_then(|s| s.trim().parse().ok());
                }
                _ => {}
            }
        }
        (texture, time)
    }

    /// `SaveProgressionAndDeathPersistentDataOfCurrentState`: what the pause menu's exit
    /// saves (`RainWorldGame.ExitGame`).
    ///
    /// # Errors
    /// When the save file cannot be written.
    pub fn save_progression_and_death_persistent_data_of_current_state(
        &mut self,
        died: bool,
        quit: bool,
    ) -> Result<(), SaveError> {
        self.save_progression(true, true)?;
        self.save_death_persistent_data_of_current_state(died, quit)
    }

    /// `WipeSaveState`: drops `slugcat`'s stored state (a new game), and `Revert`.
    ///
    /// # Errors
    /// When the save file cannot be written.
    pub fn wipe_save_state(&mut self, slugcat: SlugcatName) -> Result<(), SaveError> {
        self.misc.wipe_save_state(slugcat);
        let mut text = String::new();
        for line in self.prog_lines() {
            let parts: Vec<&str> = line.split("<progDivB>").collect();
            if parts[0] == "SAVE STATE"
                && parts.get(1).and_then(|s| SaveState::parse_save_number(s)) == Some(slugcat)
            {
                continue;
            }
            text.push_str(&line);
            text.push_str("<progDivA>");
        }
        self.store(text)?;
        self.current_save_state = None;
        self.map_discovery.clear();
        self.map_last_updated.clear();
        Ok(())
    }

    /// `WipeAll`: backs the save up as `_WipeBackup`, keeps only `MISCPROG` (wiped of
    /// its campaign data) and reads the slot again.
    ///
    /// # Errors
    /// When the save file cannot be written or read back.
    pub fn wipe_all(&mut self) -> Result<(), SaveError> {
        self.back_up_save("_WipeBackup")?;
        self.misc.wipe_all();
        self.temp_shelters_discovered.clear();
        let text = format!("MISCPROG<progDivB>{}<progDivA>", self.misc.to_save_string());
        self.store(text)?;
        let misc = self.misc.clone();
        *self = match &self.path {
            Some(path) => Self::load(path, self.salt.clone())?,
            None => Self {
                misc,
                file: self.file.clone(),
                ..Self::in_memory(self.salt.clone())
            },
        };
        Ok(())
    }

    /// `BackUpSave(appendName)`: `save_{appendName}` gets the current `"save"` value.
    ///
    /// # Errors
    /// When the save file cannot be written.
    pub fn back_up_save(&mut self, append_name: &str) -> Result<(), SaveError> {
        if !self.file.contains(SAVE_KEY) {
            return Ok(());
        }
        self.file
            .set_same_as(&format!("save_{append_name}"), SAVE_KEY);
        self.write()
    }

    /// `saveFileDataInMemory.Set("save", Md5Sum(text) + text, Immediate)`.
    fn store(&mut self, text: String) -> Result<(), SaveError> {
        let Some(salt) = &self.salt else {
            log::warn!("no checksum salt: not saving");
            return Ok(());
        };
        let value = salt.md5_sum(&text) + &text;
        self.file.set(SAVE_KEY, value);
        self.write()
    }

    fn write(&self) -> Result<(), SaveError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.file.to_bytes())?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// The `"save"` value (checksum and lines).
    #[must_use]
    pub fn save_string(&self) -> Option<&str> {
        self.file.get(SAVE_KEY)
    }

    /// The slot's container, as stored.
    #[must_use]
    pub fn file(&self) -> &UserDataFile {
        &self.file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: &str = "SAVE STATE<progDivB>SAV STATE NUMBER<svB>White<svA>DENPOS<svB>SU_S03<svA>\
        CYCLENUM<svB>4<svA>FOOD<svB>2<svA>DEATHPERSISTENTSAVEDATA<svB>REINFORCEDKARMA<dpB>0<dpA>\
        KARMA<dpB>3<dpA>KARMACAP<dpB>5<dpA>GHOSTS<dpB><dpA>TUTMESSAGES<dpB><dpA>METERSSHOWN<dpB>\
        <dpA>DEATHS<dpB>1<dpA>SURVIVES<dpB>2<dpA>QUITS<dpB>0<dpA><svA>MODKEY<svB>x<svA>";
    const RIVULET: &str = "SAVE STATE<progDivB>SAV STATE NUMBER<svB>Rivulet<svA>FOOD<svB>1<svA>";
    const START: CampaignStart<'static> = CampaignStart {
        slugcat: SlugcatName::White,
        den: "SU_C04",
        vanilla: true,
    };

    fn progression(dir: &Path) -> PlayerProgression {
        let salt = ChecksumSalt::new("salt");
        let text = format!(
            "{WHITE}<progDivA>{RIVULET}<progDivA>MAP_Rivulet<progDivB>SU<progDivB>AAAA<progDivA>\
             MAPUPDATE<progDivB>SU<progDivB>9<progDivA>MISCPROG<progDivB>CURRENTSLUGCAT<mpdB>White\
             <mpdA>INTEGERS<mpdB>0,0,0,0,0,3,1,0,0,0,0<mpdA><progDivA>"
        );
        let mut file = UserDataFile::new();
        file.set("save", salt.md5_sum(&text) + &text);
        let path = dir.join("sav");
        std::fs::write(&path, file.to_bytes()).unwrap();
        PlayerProgression::load(&path, Some(salt)).unwrap()
    }

    fn lines(p: &mut PlayerProgression) -> Vec<String> {
        p.prog_lines()
    }

    #[test]
    fn continue_saves_as_death_and_quit_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = progression(dir.path());
        assert!(!p.game_tinkered_with);
        assert!(p.is_there_a_saved_game(SlugcatName::White));
        assert!(!p.is_there_a_saved_game(SlugcatName::Yellow));
        let state = p.get_or_initiate_save_state(START, 99, true, true).unwrap();
        assert_eq!(
            (state.den_position.as_str(), state.food, state.seed),
            ("SU_S03", 2, 99)
        );
        assert_eq!(state.death_persistent.karma, 3);
        // Read back from the file: karma down, a quit, REDSDEATH, an extra `<svA>`.
        let mut again = PlayerProgression::load(&dir.path().join("sav"), p.salt.clone()).unwrap();
        assert!(!again.game_tinkered_with);
        let l = lines(&mut again);
        assert_eq!(
            l[0],
            "SAVE STATE<progDivB>SAV STATE NUMBER<svB>White<svA>DENPOS<svB>SU_S03<svA>\
             CYCLENUM<svB>4<svA>FOOD<svB>2<svA>DEATHPERSISTENTSAVEDATA<svB>REDSDEATH<dpA>\
             REINFORCEDKARMA<dpB>0<dpA>KARMA<dpB>2<dpA>KARMACAP<dpB>5<dpA>GHOSTS<dpB><dpA>\
             TUTMESSAGES<dpB><dpA>METERSSHOWN<dpB><dpA>DEATHS<dpB>1<dpA>SURVIVES<dpB>2<dpA>\
             QUITS<dpB>1<dpA><svA>MODKEY<svB>x<svA><svA>"
        );
        assert_eq!(l[1], RIVULET);
        assert_eq!(l.len(), 6);
        // A quick exit undoes it: neither a death nor a quit.
        p.save_progression_and_death_persistent_data_of_current_state(false, false)
            .unwrap();
        let l = lines(&mut p);
        assert!(l[0].contains("REINFORCEDKARMA<dpB>0<dpA>KARMA<dpB>3<dpA>"));
        assert!(l[0].contains("QUITS<dpB>0<dpA>"));
        assert!(l[0].ends_with("MODKEY<svB>x<svA><svA><svA>"));
        // Lines the port does not own stay.
        assert!(
            l.iter()
                .any(|x| x == "MAP_Rivulet<progDivB>SU<progDivB>AAAA")
        );
        assert!(l.iter().any(|x| x == "MAPUPDATE<progDivB>SU<progDivB>9"));
    }

    #[test]
    fn new_game_wipes_the_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = progression(dir.path());
        p.wipe_save_state(SlugcatName::White).unwrap();
        assert!(!p.is_there_a_saved_game(SlugcatName::White));
        let state = p.get_or_initiate_save_state(START, 7, false, true).unwrap();
        assert_eq!((state.den_position.as_str(), state.seed), ("SU_C04", 7));
        // No stored line: the death save changes nothing stored.
        let l = lines(&mut p);
        assert_eq!(l[0], RIVULET);
        assert!(p.is_there_a_saved_game(SlugcatName::White));
        // The wipe's misc changes reach the file with the next progression save.
        p.save_progression(true, true).unwrap();
        let l = lines(&mut p);
        assert!(
            l.iter()
                .any(|x| x.ends_with("INTEGERS<mpdB>0,0,0,0,0,0,0,0,0,0,0<mpdA>"))
        );
    }

    #[test]
    fn maps_from_the_hud_replace_stored_ones() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = progression(dir.path());
        assert_eq!(p.stored_map("SU"), (None, Some(9)));
        p.map_discovery.push(("SU".to_owned(), "PNG1".to_owned()));
        p.map_last_updated.push(("SU".to_owned(), 12));
        p.save_progression(true, false).unwrap();
        assert_eq!(p.stored_map("SU"), (Some("PNG1".to_owned()), Some(12)));
        let l = lines(&mut p);
        assert!(!l.iter().any(|x| x == "MAPUPDATE<progDivB>SU<progDivB>9"));
    }

    #[test]
    fn backup_shares_the_value() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = progression(dir.path());
        p.back_up_save("_Backup").unwrap();
        let bytes = std::fs::read(dir.path().join("sav")).unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("z:Ref="));
        let read = UserDataFile::parse(&bytes).unwrap();
        assert_eq!(read.get("save__Backup"), read.get("save"));
    }

    /// `WipeAll` (106810): only `MISCPROG` stays, its shelters and the Hunter-era
    /// integers reset, the Hunter unlock kept; the old save is `save__WipeBackup`.
    #[test]
    fn wipe_all_keeps_only_the_wiped_misc_progression() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = progression(dir.path());
        p.misc.set_red_unlocked(true);
        p.misc.save_discovered_shelter("SU_S01", SlugcatName::White);
        p.wipe_all().unwrap();
        assert!(p.stored_save_numbers().is_empty());
        let lines = lines(&mut p);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(
            lines[0],
            "MISCPROG<progDivB>CURRENTSLUGCAT<mpdB>White<mpdA>INTEGERS<mpdB>0,0,0,0,0,0,0,0,1,0,0<mpdA>"
        );
        assert!(p.misc.red_unlocked() && !p.misc.any_discovered_shelters());
        let read = UserDataFile::parse(&std::fs::read(dir.path().join("sav")).unwrap()).unwrap();
        assert!(read.get("save__WipeBackup").unwrap().contains("SAVE STATE"));
    }
}
