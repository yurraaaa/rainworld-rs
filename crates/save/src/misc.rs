//! `PlayerProgression.MiscProgressionData` (`MISCPROG<progDivB>...`), entries split by
//! `<mpdA>`, fields by `<mpdB>`.
//!
//! The port reads and writes the selected slugcat and `INTEGERS` (the Hunter unlock and
//! the tutorial counters a wipe resets) and the conditional shelters a wipe edits; every
//! other entry is written back as read, in place (the game regenerates them in its own
//! order: docs/DIVERGENCES.md).

use rw_assets::{ActiveMods, SlugcatName};

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
            None => entries.push(integers),
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

    /// `AreThereAnyDiscoveredShelters`: a non-empty `SHELTERLIST`.
    #[must_use]
    pub fn any_discovered_shelters(&self) -> bool {
        self.entries.iter().any(|e| {
            let parts: Vec<&str> = e.split("<mpdB>").collect();
            parts[0] == "SHELTERLIST" && parts.get(2).is_some_and(|l| !l.is_empty())
        })
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
    fn new_progression_writes_the_defaults() {
        assert_eq!(
            MiscProgression::default().to_save_string(),
            "CURRENTSLUGCAT<mpdB>White<mpdA>INTEGERS<mpdB>0,0,0,0,0,0,0,0,0,0,0<mpdA>"
        );
    }
}
