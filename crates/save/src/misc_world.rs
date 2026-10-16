//! `MiscWorldSaveData` (`MISCWORLDSAVEDATA<svB>...`, entries by `<mwA>`) and its
//! `PlayerGuideState` (`playerGuideState<mwB>...`, entries by `<pgsA>`).
//!
//! Entries are kept as read, in place; only the guide state is parsed, since writing
//! it changes it (`ToString` raises `likesPlayer` and `handHolding`).

use rw_assets::{AbstractObjectType, ActiveMods, CreatureType};

use crate::cs_float::CsFloat;

/// `PlayerGuideState`: what the guide overseer knows of the player. The type lists hold
/// the `ExtEnum` values as written (a value no mod registers is kept as it is).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerGuideState {
    /// Visited Moon, super jumps shown, pickups shown, scav trade done, angry with the
    /// player, displayed anger, guide symbol.
    pub integers: [i32; 7],
    /// `itemTypes`: the food the player has learnt about.
    pub item_types: Vec<String>,
    /// `creatureTypes`: the creatures the player has been warned of.
    pub creature_types: Vec<String>,
    pub likes_player: f32,
    pub hand_holding: f32,
    pub images_shown: Vec<String>,
    /// `forcedDirectionsGiven`: room index and count.
    pub forced_directions_given: Vec<(i32, i32)>,
    unrecognized: Vec<String>,
    /// Not saved, kept while the state stays in memory: whether `ToString` raises the
    /// like.
    pub increase_like_on_save: bool,
    pub has_been_to_a_swarm_room_this_cycle: bool,
    pub want_food_hand_holding_this_cycle: f32,
    pub want_shelter_hand_holding_this_cycle: f32,
    pub want_direction_hand_holding_this_cycle: f32,
}

impl Default for PlayerGuideState {
    fn default() -> Self {
        Self {
            integers: [0; 7],
            item_types: Vec::new(),
            creature_types: Vec::new(),
            likes_player: 0.3,
            hand_holding: 0.8,
            images_shown: Vec::new(),
            forced_directions_given: Vec::new(),
            unrecognized: Vec::new(),
            increase_like_on_save: true,
            has_been_to_a_swarm_room_this_cycle: false,
            want_food_hand_holding_this_cycle: 0.65,
            want_shelter_hand_holding_this_cycle: 1.0,
            want_direction_hand_holding_this_cycle: 0.6,
        }
    }
}

impl PlayerGuideState {
    #[must_use]
    pub fn angry_with_player(&self) -> bool {
        self.integers[4] > 0
    }

    /// `FromString` onto this state. `imagesShown` and `forcedDirsGiven` add to the
    /// lists; an unparsable number reads as 0, a bad forced direction is skipped (the
    /// game throws).
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
                "itemTypes" => Self::read_types(value, &mut self.item_types, &Self::legacy_items()),
                "creatureTypes" => {
                    Self::read_types(value, &mut self.creature_types, &Self::legacy_creatures());
                }
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
                            self.forced_directions_given.push((k, v));
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

    /// A type list: names by `,`, or the old digit form (`ParseItemTypes`,
    /// `ParseCreatureTypes`: a non-`0` digit at index i adds the table's i-th type).
    fn read_types(value: &str, list: &mut Vec<String>, legacy: &[&'static str]) {
        list.clear();
        if !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()) {
            for (i, c) in value.chars().enumerate() {
                if c != '0'
                    && let Some(name) = legacy.get(i)
                {
                    list.push((*name).to_owned());
                }
            }
            return;
        }
        list.extend(
            value
                .split(',')
                .filter(|t| !t.is_empty())
                .map(str::to_owned),
        );
    }

    /// `ParseItemTypes`' table: the base types up to `OverseerCarcass` without
    /// `GraffitiBomb`.
    fn legacy_items() -> Vec<&'static str> {
        AbstractObjectType::registered(ActiveMods::NONE)
            .into_iter()
            .filter(|&t| t != AbstractObjectType::GraffitiBomb)
            .take(31)
            .map(AbstractObjectType::value)
            .collect()
    }

    /// `ParseCreatureTypes`' table: the base types, `StandardGroundCreature` to `Hazer`.
    fn legacy_creatures() -> Vec<&'static str> {
        CreatureType::registered(ActiveMods::NONE)
            .into_iter()
            .take(47)
            .map(CreatureType::value)
            .collect()
    }

    /// `ToString`, which first raises the like by 0.1 (or zeroes it when angry) and the
    /// hand holding by 0.025.
    pub fn save_string(&mut self) -> String {
        let ints: Vec<String> = self.integers.iter().map(i32::to_string).collect();
        let mut text = format!(
            "integersArray<pgsB>{}<pgsA>itemTypes<pgsB>{}<pgsA>creatureTypes<pgsB>{}<pgsA>",
            ints.join("."),
            self.item_types.join(","),
            self.creature_types.join(",")
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
        if !self.forced_directions_given.is_empty() {
            // The game assigns instead of appending here: what came before is lost.
            let dirs: Vec<String> = self
                .forced_directions_given
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

    /// The value of entry `key`.
    fn entry(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find_map(|e| e.strip_prefix(key).and_then(|r| r.strip_prefix("<mwB>")))
    }

    /// `SSaiConversationsHad`.
    #[must_use]
    pub fn ss_ai_conversations_had(&self) -> i32 {
        self.entry("SSaiConversationsHad")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    }

    /// `EverMetMoon`: `SLaiState`'s `playerEncounters` (its first integer) above 0.
    #[must_use]
    pub fn ever_met_moon(&self) -> bool {
        let Some(state) = self.entry("SLaiState") else {
            return false;
        };
        state
            .split("<slosA>")
            .find_map(|e| e.strip_prefix("integersArray<slosB>"))
            .and_then(|ints| ints.split('.').next())
            .and_then(|n| n.trim().parse::<i32>().ok())
            .is_some_and(|n| n > 0)
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
        assert!(!m.ever_met_moon());
        assert_eq!(
            m.save_string(),
            "SSaiConversationsHad<mwB>1<mwA>playerGuideState<mwB>integersArray<pgsB>0.0.1.1.0.0.2<pgsA>\
             itemTypes<pgsB><pgsA>creatureTypes<pgsB><pgsA>likesPlayer<pgsB>1<pgsA>\
             directionHandHolding<pgsB>1<pgsA><mwA>CyclesSinceSSai<mwB>4<mwA>"
        );
    }

    /// Every field read back in place: the type lists by name, the images and forced
    /// directions in order, an unknown entry at the end. A saved forced direction makes
    /// `ToString` start over from it (the game's `text =`).
    #[test]
    fn guide_state_reads_every_field() {
        let s = "integersArray<pgsB>1.4.1.0.1.1.2<pgsA>itemTypes<pgsB>DangleFruit,SeedCob<pgsA>\
                 creatureTypes<pgsB>PinkLizard,Spitter<pgsA>likesPlayer<pgsB>-0.25<pgsA>\
                 directionHandHolding<pgsB>0.5<pgsA>imagesShown<pgsB>SU_A22.SU_A37<pgsA>\
                 forcedDirsGiven<pgsB>12,3.40,1<pgsA>modKey<pgsB>x<pgsA>";
        let mut g = PlayerGuideState::default();
        g.from_string(s);
        assert_eq!(g.integers, [1, 4, 1, 0, 1, 1, 2]);
        assert_eq!(g.item_types, ["DangleFruit", "SeedCob"]);
        assert_eq!(g.creature_types, ["PinkLizard", "Spitter"]);
        assert_eq!((g.likes_player, g.hand_holding), (-0.25, 0.5));
        assert_eq!(g.images_shown, ["SU_A22", "SU_A37"]);
        assert_eq!(g.forced_directions_given, [(12, 3), (40, 1)]);
        assert!(g.angry_with_player());
        assert_eq!(
            g.save_string(),
            "forcedDirsGiven<pgsB>12,3.40,1<pgsA>modKey<pgsB>x<pgsA>"
        );
        // Angry: the like is zeroed, the hand holding still raised.
        assert_eq!((g.likes_player, g.hand_holding), (0.0, 0.525));
        g.forced_directions_given.clear();
        g.integers[4] = 0;
        g.increase_like_on_save = false;
        assert_eq!(
            g.save_string(),
            "integersArray<pgsB>1.4.1.0.0.1.2<pgsA>itemTypes<pgsB>DangleFruit,SeedCob<pgsA>\
             creatureTypes<pgsB>PinkLizard,Spitter<pgsA>likesPlayer<pgsB>0<pgsA>\
             directionHandHolding<pgsB>0.55<pgsA>imagesShown<pgsB>SU_A22.SU_A37<pgsA>modKey<pgsB>x<pgsA>"
        );
    }

    /// The old digit form: `ParseItemTypes` index 6 is `DangleFruit` (no `GraffitiBomb`
    /// in the table), 12 `SeedCob`; `ParseCreatureTypes` index 3 `PinkLizard`, 46 `Hazer`.
    #[test]
    fn legacy_digit_type_lists() {
        let mut g = PlayerGuideState::default();
        g.from_string("itemTypes<pgsB>0000001000001<pgsA>");
        assert_eq!(g.item_types, ["DangleFruit", "SeedCob"]);
        let mut digits = "0".repeat(47);
        digits.replace_range(3..4, "1");
        digits.replace_range(46..47, "2");
        g.from_string(&format!("creatureTypes<pgsB>{digits}<pgsA>"));
        assert_eq!(g.creature_types, ["PinkLizard", "Hazer"]);
        g.from_string("creatureTypes<pgsB><pgsA>");
        assert!(g.creature_types.is_empty());
    }

    #[test]
    fn ever_met_moon_reads_the_oracle_state() {
        let m = MiscWorldSaveData::from_string(
            "SLaiState<mwB>integersArray<slosB>2.0.0<slosA>likesPlayer<slosB>0.3<slosA><mwA>",
        );
        assert!(m.ever_met_moon());
        let m = MiscWorldSaveData::from_string("SLaiState<mwB>integersArray<slosB>0.1<slosA><mwA>");
        assert!(!m.ever_met_moon());
    }
}
