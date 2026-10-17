//! The stand-in for `LizardAI` until lizard step 7: what its `Update` does around
//! `DetermineBehavior` and the behaviours (the panic and room clocks, the abstract
//! behaviour roll, the social memory's temps, `walkPastPointOfNoReturn`, the jaw,
//! `runSpeed` from the stuck tracker, the casual aggression) and the base
//! `ArtificialIntelligence`'s update and `NewRoom` with its path finder, tracker,
//! relationship tracker, stuck tracker and discomfort tracker. The update itself is
//! `RainWorldGame::lizard_ai_update` (`game_lizard_act.rs`).

use crate::creature::{DiscomfortTracker, RelationshipTracker, RepKey, StuckTracker, Tracker};

/// `LizardAI` as far as lizard step 6a goes.
#[derive(Debug, Clone, PartialEq)]
pub struct LizardAi {
    /// The `LizardPather` (a number in the world's path finders).
    pub path_finder: Option<usize>,
    pub run_speed: f32,
    /// A dev tool's fixed `runSpeed` (MCP `set_destination`), not the game's.
    pub dev_run_speed: Option<f32>,
    pub stranded: bool,
    pub time_in_room: i32,
    /// `ArtificialIntelligence.lastRoom`.
    pub last_room: i32,
    pub panic: i32,
    pub excitement: f32,
    pub stuck: StuckTracker,
    pub discomfort: DiscomfortTracker,
    pub tracker: Tracker,
    pub relationships: RelationshipTracker,
    /// `focusCreature`: set by the behaviours (lizard step 7), so none yet.
    pub focus_creature: Option<RepKey>,
    pub casual_aggression_target: Option<RepKey>,
    /// `usedToVultureMask` (no vulture masks are ported, so it stays 0).
    pub used_to_vulture_mask: i32,
}

impl LizardAi {
    /// The representations the AI and its graphics refer to (kept when the tracker
    /// drops them from its list).
    #[must_use]
    pub fn held_reps(&self, look: Option<RepKey>) -> Vec<RepKey> {
        let mut held: Vec<RepKey> = self.focus_creature.into_iter().collect();
        held.extend(self.casual_aggression_target);
        held.extend(look);
        held
    }
}

impl Default for LizardAi {
    fn default() -> Self {
        Self {
            path_finder: None,
            run_speed: 0.0,
            dev_run_speed: None,
            stranded: false,
            time_in_room: 0,
            last_room: -1,
            panic: 0,
            excitement: 0.5,
            stuck: StuckTracker::lizard(),
            discomfort: DiscomfortTracker::default(),
            tracker: Tracker::lizard(),
            relationships: RelationshipTracker::default(),
            focus_creature: None,
            casual_aggression_target: None,
            used_to_vulture_mask: 0,
        }
    }
}
