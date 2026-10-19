//! `PlayerProgression.MiscProgressionData` (`MISCPROG<progDivB>...`), entries split by
//! `<mpdA>`, fields by `<mpdB>`.
//!
//! The port reads and writes the selected slugcat and `INTEGERS` (the Hunter unlock and
//! the tutorial counters a wipe resets) and the conditional shelters a wipe edits; every
//! other entry is written back as read, in place (the game regenerates them in its own
//! order: docs/divergences/).

use rw_assets::{ActiveMods, SlugcatName};

/// The sleep and death screens' watch counters (`integers[1..=4]`), which make a
/// screen's first viewings wait for its animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchedScreen {
    /// `watchedSleepScreens`.
    Sleep,
    /// `watchedDeathScreens` (deaths and starves).
    Death,
    /// `watchedDeathScreensWithFlower`.
    DeathWithFlower,
    /// `watchedMalnourishScreens`.
    Malnourish,
}

impl WatchedScreen {
    fn index(self) -> usize {
        match self {
            Self::Sleep => 1,
            Self::Death => 2,
            Self::DeathWithFlower => 3,
            Self::Malnourish => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MiscProgression {
    /// The entries as read, `CURRENTSLUGCAT` and `INTEGERS` rewritten from the fields.
    entries: Vec<String>,
    /// `currentlySelectedSinglePlayerSlugcat`.
    pub current_slugcat: SlugcatName,
    /// `integers` (11 with every DLC off), then any extra read (`unrecognizedIntegers`).
    integers: Vec<i32>,
}

impl Default for MiscProgression {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            current_slugcat: SlugcatName::White,
            integers: vec![0; Self::INTEGERS],
        }
    }
}

impl MiscProgression {
    const INTEGERS: usize = 11;
    const STARVATION_TUTORIAL_COUNTER: usize = 5;
    const WARNED_ABOUT_KARMA_LOSS_ON_EXIT: usize = 6;
    const RED_UNLOCKED: usize = 8;

    /// `FromString` onto a new instance.
    #[must_use]
    pub fn from_string(s: &str) -> Self {
        let mut misc = Self::default();
        for entry in s.split("<mpdA>") {
            let parts: Vec<&str> = entry.split("<mpdB>").collect();
            match parts[0] {
                "CURRENTSLUGCAT" => {
                    // An unregistered name (a DLC's, with it off) falls back to White.
                    misc.current_slugcat = parts
                        .get(1)
                        .and_then(|v| SlugcatName::from_value(v, ActiveMods::NONE))
                        .unwrap_or(SlugcatName::White);
                }
                "INTEGERS" => {
                    let read: Vec<i32> = parts
                        .get(1)
                        .map(|v| {
                            v.split(',')
                                .map(|n| n.trim().parse().unwrap_or(0))
                                .collect()
                        })
                        .unwrap_or_default();
                    for (i, n) in read.into_iter().enumerate() {
                        match misc.integers.get_mut(i) {
                            Some(slot) => *slot = n,
                            None => misc.integers.push(n),
                        }
                    }
                }
                _ => {}
            }
            misc.entries.push(entry.to_owned());
        }
        misc
    }

    /// `ToString`: the entries in place, `CURRENTSLUGCAT` first and `INTEGERS` added if
    /// they were missing.
    #[must_use]
    pub fn to_save_string(&self) -> String {
        let current = format!("CURRENTSLUGCAT<mpdB>{}", self.current_slugcat.value());
        let integers = format!(
            "INTEGERS<mpdB>{}",
            self.integers
                .iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut entries: Vec<String> = self.entries.clone();
        if entries.last().is_some_and(String::is_empty) {
            entries.pop();
        }
        match entries
            .iter()
            .position(|e| Self::key(e) == "CURRENTSLUGCAT")
        {
            Some(i) => entries[i] = current,
            None => entries.insert(0, current),
        }
        match entries.iter().position(|e| Self::key(e) == "INTEGERS") {
            Some(i) => entries[i] = integers,
            // Before what the game writes after it.
            None => {
                let at = entries
                    .iter()
                    .position(|e| matches!(Self::key(e), "REDSFLOWER" | "MENUREGION"))
                    .unwrap_or(entries.len());
                entries.insert(at, integers);
            }
        }
        let mut s = String::new();
        for e in entries {
            s.push_str(&e);
            s.push_str("<mpdA>");
        }
        s
    }

    fn key(entry: &str) -> &str {
        entry.split("<mpdB>").next().unwrap_or("")
    }

    /// `redUnlocked`.
    #[must_use]
    pub fn red_unlocked(&self) -> bool {
        self.integers[Self::RED_UNLOCKED] == 1
    }

    pub fn set_red_unlocked(&mut self, unlocked: bool) {
        self.integers[Self::RED_UNLOCKED] = i32::from(unlocked);
    }

    /// `warnedAboutKarmaLossOnExit`: how often the pause menu warned of the exit's karma loss.
    #[must_use]
    pub fn warned_about_karma_loss_on_exit(&self) -> i32 {
        self.integers[Self::WARNED_ABOUT_KARMA_LOSS_ON_EXIT]
    }

    pub fn set_warned_about_karma_loss_on_exit(&mut self, times: i32) {
        self.integers[Self::WARNED_ABOUT_KARMA_LOSS_ON_EXIT] = times;
    }

    #[must_use]
    pub fn watched(&self, screen: WatchedScreen) -> i32 {
        self.integers[screen.index()]
    }

    pub fn set_watched(&mut self, screen: WatchedScreen, n: i32) {
        self.integers[screen.index()] = n;
    }

    /// `starvationTutorialCounter`.
    #[must_use]
    pub fn starvation_tutorial_counter(&self) -> i32 {
        self.integers[Self::STARVATION_TUTORIAL_COUNTER]
    }

    pub fn set_starvation_tutorial_counter(&mut self, n: i32) {
        self.integers[Self::STARVATION_TUTORIAL_COUNTER] = n;
    }

    /// The index of the entry with `key`.
    fn entry_index(&self, key: &str) -> Option<usize> {
        self.entries.iter().position(|e| Self::key(e) == key)
    }

    /// Where a new entry goes: after the last of `after` present, else at `fallback`.
    fn insert_index(&self, after: &[&str], fallback: usize) -> usize {
        self.entries
            .iter()
            .rposition(|e| after.contains(&Self::key(e)))
            .map_or(fallback.min(self.entries.len()), |i| i + 1)
    }

    /// `menuRegion` (`MENUREGION`, written after `INTEGERS` and `REDSFLOWER`).
    pub fn set_menu_region(&mut self, region: &str) {
        let entry = format!("MENUREGION<mpdB>{region}");
        match self.entry_index("MENUREGION") {
            Some(i) => self.entries[i] = entry,
            None => {
                let at = self.insert_index(
                    &[
                        "INTEGERS",
                        "INTEGERSMMF",
                        "INTEGERSMSC",
                        "INTEGERSWATCHER",
                        "REDSFLOWER",
                    ],
                    self.entries.len(),
                );
                self.entries.insert(at, entry);
            }
        }
    }

    /// `ShelterDoor.Close`'s `regionsVisited[region].Add(slugcat)` (`VISITED`, written
    /// before `INTEGERS`).
    pub fn add_region_visited(&mut self, region: &str, slugcat: SlugcatName) {
        let i = match self.entry_index("VISITED") {
            Some(i) => i,
            None => {
                let at = self.entry_index("INTEGERS").unwrap_or(self.entries.len());
                self.entries.insert(at, "VISITED<mpdB>".to_owned());
                at
            }
        };
        let body = self.entries[i]
            .strip_prefix("VISITED<mpdB>")
            .unwrap_or("")
            .to_owned();
        let mut regions: Vec<(String, Vec<String>)> = body
            .split("<mpdB>")
            .filter(|r| !r.is_empty())
            .map(|r| {
                let (name, list) = r.split_once("<mpdC>").unwrap_or((r, ""));
                let list = list
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect();
                (name.to_owned(), list)
            })
            .collect();
        match regions.iter_mut().find(|(r, _)| r == region) {
            Some((_, list)) if list.iter().any(|s| s == slugcat.value()) => return,
            Some((_, list)) => list.push(slugcat.value().to_owned()),
            None => regions.push((region.to_owned(), vec![slugcat.value().to_owned()])),
        }
        let parts: Vec<String> = regions
            .iter()
            .map(|(r, l)| format!("{r}<mpdC>{}", l.join(",")))
            .collect();
        self.entries[i] = format!("VISITED<mpdB>{}", parts.join("<mpdB>"));
    }

    /// `SaveDiscoveredShelter(room)`: the shelter joins its region's `SHELTERLIST` and
    /// `slugcat` its `CONDITIONALSHELTERDATA` entry.
    pub fn save_discovered_shelter(&mut self, room: &str, slugcat: SlugcatName) {
        let Some((region, _)) = room.split_once('_') else {
            return;
        };
        // `updateConditionalShelters`.
        let shelters = match self.entry_index("CONDITIONALSHELTERDATA") {
            Some(i) => i,
            None => {
                let at = self.insert_index(&["SHELTERLIST", "CURRENTSLUGCAT"], 0);
                self.entries
                    .insert(at, "CONDITIONALSHELTERDATA<mpdB>".to_owned());
                at
            }
        };
        let data = self.entries[shelters]
            .strip_prefix("CONDITIONALSHELTERDATA<mpdB>")
            .unwrap_or("")
            .to_owned();
        let mut list: Vec<Vec<String>> = data
            .split("<mpdC>")
            .filter(|s| s.len() > 1)
            .map(|s| {
                s.split(" : ")
                    .enumerate()
                    .filter(|(i, f)| *i == 0 || !f.trim().is_empty())
                    .map(|(_, f)| f.to_owned())
                    .collect()
            })
            .collect();
        match list.iter_mut().find(|s| s[0] == room) {
            Some(s) if s[1..].iter().any(|n| n == slugcat.value()) => {}
            Some(s) => s.push(slugcat.value().to_owned()),
            None => list.push(vec![room.to_owned(), slugcat.value().to_owned()]),
        }
        let mut text = "CONDITIONALSHELTERDATA<mpdB>".to_owned();
        for s in list {
            text.push_str(&s.join(" : "));
            text.push_str(" : <mpdC>");
        }
        self.entries[shelters] = text;
        let prefix = format!("SHELTERLIST<mpdB>{region}<mpdB>");
        match self.entries.iter().position(|e| e.starts_with(&prefix)) {
            Some(i) => {
                let names = self.entries[i][prefix.len()..].to_owned();
                if !names.split("<mpdC>").any(|n| n == room) {
                    let sep = if names.is_empty() { "" } else { "<mpdC>" };
                    self.entries[i] = format!("{prefix}{names}{sep}{room}");
                }
            }
            None => {
                let at = self.insert_index(&["SHELTERLIST", "CURRENTSLUGCAT"], 0);
                self.entries.insert(at, format!("{prefix}{room}"));
            }
        }
    }

    /// `AreThereAnyDiscoveredShelters`: a non-empty `SHELTERLIST`.
    #[must_use]
    pub fn any_discovered_shelters(&self) -> bool {
        self.entries.iter().any(|e| {
            let parts: Vec<&str> = e.split("<mpdB>").collect();
            parts[0] == "SHELTERLIST" && parts.get(2).is_some_and(|l| !l.is_empty())
        })
    }

    /// `WipeAll`'s part with every DLC off: the discovered and conditional shelters,
    /// the menu region, the Hunter's flower and the campaign timers go, and the base
    /// `INTEGERS` it resets (the watched screens, tutorial counters, karma warning,
    /// `redHasVisitedPebbles`, `redMeatEatTutorial`) are 0. The DLCs' integer lists are
    /// kept as read.
    pub fn wipe_all(&mut self) {
        self.entries.retain(|e| {
            !matches!(
                Self::key(e),
                "SHELTERLIST"
                    | "MENUREGION"
                    | "CONDITIONALSHELTERDATA"
                    | "REDSFLOWER"
                    | "CAMPAIGNTIME"
            )
        });
        for i in [1, 2, 3, 4, 5, 6, 7, 10] {
            self.integers[i] = 0;
        }
    }

    /// `WipeSaveState`'s part with every DLC off: `slugcat` leaves the conditional
    /// shelters (and empty ones go), and the Survivor's and Monk's tutorial counters
    /// reset.
    pub fn wipe_save_state(&mut self, slugcat: SlugcatName) {
        for entry in &mut self.entries {
            let Some(data) = entry.strip_prefix("CONDITIONALSHELTERDATA<mpdB>") else {
                continue;
            };
            let mut kept = String::new();
            for shelter in data.split("<mpdC>").filter(|s| s.len() > 1) {
                let mut fields = shelter.split(" : ");
                let name = fields.next().unwrap_or("");
                let found: Vec<&str> = fields
                    .filter(|f| !f.trim().is_empty() && *f != slugcat.value())
                    .collect();
                if found.is_empty() {
                    continue;
                }
                kept.push_str(name);
                for f in found {
                    kept.push_str(" : ");
                    kept.push_str(f);
                }
                kept.push_str(" : <mpdC>");
            }
            *entry = format!("CONDITIONALSHELTERDATA<mpdB>{kept}");
        }
        // An emptied list is not written (`ConditionalShelterDiscovery.Count > 0`).
        self.entries
            .retain(|e| e.as_str() != "CONDITIONALSHELTERDATA<mpdB>");
        if slugcat == SlugcatName::White || slugcat == SlugcatName::Yellow {
            self.integers[Self::STARVATION_TUTORIAL_COUNTER] = 0;
            self.integers[Self::WARNED_ABOUT_KARMA_LOSS_ON_EXIT] = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_unknown_entries_and_rewrites_known_ones() {
        let s = "CURRENTSLUGCAT<mpdB>Rivulet<mpdA>SHELTERLIST<mpdB>SU<mpdB>SU_S01<mpdA>\
                 CONDITIONALSHELTERDATA<mpdB>SU_S04 : White : Red : <mpdC>LF_S01 : White : <mpdC><mpdA>\
                 INTEGERS<mpdB>0,1,2,3,4,5,6,7,1,9,10,77<mpdA>INTEGERSMSC<mpdB>3,4<mpdA>";
        let mut m = MiscProgression::from_string(s);
        assert_eq!(m.current_slugcat, SlugcatName::White);
        assert!(m.red_unlocked());
        assert!(m.any_discovered_shelters());
        m.current_slugcat = SlugcatName::Rivulet;
        assert_eq!(m.to_save_string(), s);
        m.current_slugcat = SlugcatName::Yellow;
        m.wipe_save_state(SlugcatName::White);
        assert_eq!(
            m.to_save_string(),
            "CURRENTSLUGCAT<mpdB>Yellow<mpdA>SHELTERLIST<mpdB>SU<mpdB>SU_S01<mpdA>\
             CONDITIONALSHELTERDATA<mpdB>SU_S04 : Red : <mpdC><mpdA>\
             INTEGERS<mpdB>0,1,2,3,4,0,0,7,1,9,10,77<mpdA>INTEGERSMSC<mpdB>3,4<mpdA>"
        );
    }

    #[test]
    fn regions_visited_and_menu_region() {
        let mut m = MiscProgression::from_string(
            "CURRENTSLUGCAT<mpdB>White<mpdA>VISITED<mpdB>CC<mpdC>Red<mpdA>INTEGERS<mpdB>0,0,0,0,0,0,0,0,0,0,0<mpdA>",
        );
        m.add_region_visited("SU", SlugcatName::White);
        m.add_region_visited("CC", SlugcatName::White);
        m.add_region_visited("CC", SlugcatName::White);
        m.set_menu_region("SU");
        assert_eq!(
            m.to_save_string(),
            "CURRENTSLUGCAT<mpdB>White<mpdA>VISITED<mpdB>CC<mpdC>Red,White<mpdB>SU<mpdC>White<mpdA>\
             INTEGERS<mpdB>0,0,0,0,0,0,0,0,0,0,0<mpdA>MENUREGION<mpdB>SU<mpdA>"
        );
    }

    #[test]
    fn watched_screens_are_integers_one_to_four() {
        let mut m = MiscProgression::from_string("INTEGERS<mpdB>0,1,2,3,4,5,6,7,8,9,10<mpdA>");
        assert_eq!(m.watched(WatchedScreen::Sleep), 1);
        assert_eq!(m.watched(WatchedScreen::Malnourish), 4);
        m.set_watched(WatchedScreen::Death, 7);
        m.set_watched(WatchedScreen::DeathWithFlower, 8);
        assert!(
            m.to_save_string()
                .contains("INTEGERS<mpdB>0,1,7,8,4,5,6,7,8,9,10<mpdA>")
        );
    }

    #[test]
    fn new_progression_writes_the_defaults() {
        assert_eq!(
            MiscProgression::default().to_save_string(),
            "CURRENTSLUGCAT<mpdB>White<mpdA>INTEGERS<mpdB>0,0,0,0,0,0,0,0,0,0,0<mpdA>"
        );
    }
}
