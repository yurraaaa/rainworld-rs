//! The stand-in for `LizardAI` until lizard step 7: what its `Update` does around
//! `DetermineBehavior` and the behaviours (the panic and room clocks, the abstract
//! behaviour roll, the social memory's temps, `walkPastPointOfNoReturn`, the jaw,
//! `runSpeed` from the stuck tracker, the casual aggression) and the base
//! `ArtificialIntelligence`'s update and `NewRoom` with its path finder, tracker,
//! noise tracker, relationship tracker, stuck tracker and discomfort tracker. The update
//! itself is `RainWorldGame::lizard_ai_update` (`game_lizard_act.rs`).
//!
//! Mirrors: LizardAI.cs (Update), ArtificialIntelligence.cs (Update, NewRoom)

use rw_assets::CreatureType;

use crate::FloatExt;
use crate::creature::{
    DiscomfortTracker, NoiseTracker, RelationshipTracker, RepKey, StuckTracker, Tracker,
};

/// `LizardAI` as far as lizard step 6b goes.
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
    pub noise: NoiseTracker,
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
        held.extend(self.noise.held_reps());
        held
    }

    /// `LizardAI.Update`'s `noiseTracker.hearingSkill`: 2 for black (and the Watcher's
    /// indigo) lizards, else 1.3 standing still down to 0.6 at a run speed of 0.6. The
    /// rot module's (More Slugcats, Watcher) is not ported.
    pub fn set_hearing_skill(&mut self, kind: CreatureType) {
        self.noise.hearing_skill =
            if matches!(kind, CreatureType::BlackLizard | CreatureType::IndigoLizard) {
                2.0
            } else {
                self.run_speed.lerp_map(0.0, 0.6, 1.3, 0.6)
            };
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
            noise: NoiseTracker::default(),
            relationships: RelationshipTracker::default(),
            focus_creature: None,
            casual_aggression_target: None,
            used_to_vulture_mask: 0,
        }
    }
}
