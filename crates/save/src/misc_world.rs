//! `MiscWorldSaveData` (`MISCWORLDSAVEDATA<svB>...`, entries by `<mwA>`) and its
//! `PlayerGuideState` (`playerGuideState<mwB>...`, entries by `<pgsA>`).
//!
//! Entries are kept as read, in place; only the guide state is parsed, since writing
//! it changes it (`ToString` raises `likesPlayer` and `handHolding`).

use crate::cs_float::CsFloat;

/// `PlayerGuideState`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerGuideState {
    /// Visited Moon, super jumps shown, pickups shown, scav trade done, angry with the
    /// player, displayed anger, guide symbol.
    pub integers: [i32; 7],
    /// `itemTypes` and `creatureTypes` as read (comma lists).
    item_types: String,
    creature_types: String,
    pub likes_player: f32,
    pub hand_holding: f32,
    images_shown: Vec<String>,
    forced_directions: Vec<(i32, i32)>,
    unrecognized: Vec<String>,
    /// Runtime: whether `ToString` raises the like (not saved).
    pub increase_like_on_save: bool,
}

impl Default for PlayerGuideState {
    fn default() -> Self {
        Self {
            integers: [0; 7],
            item_types: String::new(),
            creature_types: String::new(),
            likes_player: 0.3,
            hand_holding: 0.8,
            images_shown: Vec::new(),
            forced_directions: Vec::new(),
            unrecognized: Vec::new(),
            increase_like_on_save: true,
        }
    }
}

impl PlayerGuideState {
    #[must_use]
    pub fn angry_with_player(&self) -> bool {
        self.integers[4] > 0
    }

    /// `FromString` onto this state.
    pub fn from_string(&mut self, s: &str) {
        self.unrecognized.clear();
        for entry in s.split("<pgsA>") {
            let parts: Vec<&str> = entry.split("<pgsB>").collect();
            let value = parts.get(1).copied().unwrap_or("");
            match parts[0] {
                "integersArray" => {
                    for (slot, v) in self.integers.iter_mut().zip(value.split('.')) {
                        *slot = v.trim().parse().unwrap_or(0);
                    }
                }
                "itemTypes" => value.clone_into(&mut self.item_types),
                "creatureTypes" => value.clone_into(&mut self.creature_types),
                "likesPlayer" => self.likes_player = value.trim().parse().unwrap_or(0.0),
                "directionHandHolding" => {
                    self.hand_holding = value.trim().parse().unwrap_or(0.0);
                }
                "imagesShown" => self
                    .images_shown
                    .extend(value.split('.').map(str::to_owned)),
                "forcedDirsGiven" => {
                    for pair in value.split('.') {
                        let mut kv = pair.split(',');
                        let k = kv.next().and_then(|k| k.trim().parse().ok());
                        let v = kv.next().and_then(|v| v.trim().parse().ok());
                        if let (Some(k), Some(v)) = (k, v) {
                            self.forced_directions.push((k, v));
                        }
                    }
                }
                _ if !entry.trim().is_empty() && parts.len() >= 2 => {
                    self.unrecognized.push(entry.to_owned());
                }
                _ => {}
            }
        }
    }

    /// `ToString`, which first raises the like by 0.1 (or zeroes it when angry) and the
    /// hand holding by 0.025.
    pub fn save_string(&mut self) -> String {
        let ints: Vec<String> = self.integers.iter().map(i32::to_string).collect();
        let mut text = format!(
            "integersArray<pgsB>{}<pgsA>itemTypes<pgsB>{}<pgsA>creatureTypes<pgsB>{}<pgsA>",
            ints.join("."),
            self.item_types,
            self.creature_types
        );
        if self.angry_with_player() {
            self.likes_player = 0.0;
        } else if self.increase_like_on_save {
            self.likes_player = (self.likes_player + 0.1).clamp(-1.0, 1.0);
        }
        self.hand_holding = (self.hand_holding + 0.025).clamp(0.0, 1.0);
        text.push_str(&format!(
            "likesPlayer<pgsB>{}<pgsA>directionHandHolding<pgsB>{}<pgsA>",
            self.likes_player.cs_string(),
            self.hand_holding.cs_string()
        ));
        if !self.images_shown.is_empty() {
            text.push_str(&format!(
                "imagesShown<pgsB>{}<pgsA>",
                self.images_shown.join(".")
            ));
        }
        if !self.forced_directions.is_empty() {
            // The game assigns instead of appending here: what came before is lost.
            let dirs: Vec<String> = self
                .forced_directions
                .iter()
                .map(|(k, v)| format!("{k},{v}"))
                .collect();
            text = format!("forcedDirsGiven<pgsB>{}<pgsA>", dirs.join("."));
        }
        for u in &self.unrecognized {
            text.push_str(u);
            text.push_str("<pgsA>");
        }
        text
    }
}

/// `MiscWorldSaveData` with every DLC off.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MiscWorldSaveData {
    /// The entries as read; the guide state's is rewritten from `player_guide`.
    entries: Vec<String>,
    pub player_guide: PlayerGuideState,
}

impl MiscWorldSaveData {
    const GUIDE_KEY: &'static str = "playerGuideState";

    /// `FromString` onto a new instance.
    #[must_use]
    pub fn from_string(s: &str) -> Self {
        let mut data = Self::default();
        for entry in s.split("<mwA>") {
            if entry.is_empty() {
                continue;
            }
            if let Some(guide) = entry
                .strip_prefix(Self::GUIDE_KEY)
                .and_then(|r| r.strip_prefix("<mwB>"))
            {
                data.player_guide.from_string(guide);
            }
            data.entries.push(entry.to_owned());
        }
        data
    }

    /// `SSaiConversationsHad`.
    #[must_use]
    pub fn ss_ai_conversations_had(&self) -> i32 {
        self.entries
            .iter()
            .find_map(|e| e.strip_prefix("SSaiConversationsHad<mwB>"))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    }

    /// `ToString` (which changes the guide state, as the game's does). A missing guide
    /// entry goes after `SSaiConversationsHad`, `SSaiThrowOuts` and `SLaiState`.
    pub fn save_string(&mut self) -> String {
        let guide = format!(
            "{}<mwB>{}",
            Self::GUIDE_KEY,
            self.player_guide.save_string()
        );
        let mut entries = self.entries.clone();
        match entries
            .iter()
            .position(|e| e.split("<mwB>").next() == Some(Self::GUIDE_KEY))
        {
            Some(i) => entries[i] = guide,
            None => {
                let at = entries
                    .iter()
                    .take_while(|e| {
                        matches!(
                            e.split("<mwB>").next(),
                            Some("SSaiConversationsHad" | "SSaiThrowOuts" | "SLaiState")
                        )
                    })
                    .count();
                entries.insert(at, guide);
            }
        }
        let mut text = String::new();
        for e in entries {
            text.push_str(&e);
            text.push_str("<mwA>");
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_data_writes_the_guide_and_raises_its_like() {
        let mut m = MiscWorldSaveData::default();
        assert_eq!(
            m.save_string(),
            "playerGuideState<mwB>integersArray<pgsB>0.0.0.0.0.0.0<pgsA>itemTypes<pgsB><pgsA>\
             creatureTypes<pgsB><pgsA>likesPlayer<pgsB>0.4<pgsA>directionHandHolding<pgsB>0.825<pgsA><mwA>"
        );
        let s = "SSaiConversationsHad<mwB>1<mwA>playerGuideState<mwB>integersArray<pgsB>0.0.1.1.0.0.2<pgsA>\
                 likesPlayer<pgsB>1<pgsA>directionHandHolding<pgsB>1<pgsA><mwA>CyclesSinceSSai<mwB>4<mwA>";
        let mut m = MiscWorldSaveData::from_string(s);
        assert_eq!(m.ss_ai_conversations_had(), 1);
        assert_eq!(
            m.save_string(),
            "SSaiConversationsHad<mwB>1<mwA>playerGuideState<mwB>integersArray<pgsB>0.0.1.1.0.0.2<pgsA>\
             itemTypes<pgsB><pgsA>creatureTypes<pgsB><pgsA>likesPlayer<pgsB>1<pgsA>\
             directionHandHolding<pgsB>1<pgsA><mwA>CyclesSinceSSai<mwB>4<mwA>"
        );
    }
}
