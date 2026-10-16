//! The stand-in for `LizardAI` until lizard step 7: what its `Update` does before and
//! around `DetermineBehavior` that needs no tracker (the panic and room clocks, the
//! abstract behaviour roll, the social memory's temps, `walkPastPointOfNoReturn`, the
//! jaw, `runSpeed` from the stuck tracker) and the base `ArtificialIntelligence`'s
//! update and `NewRoom` with its path finder, stuck tracker and discomfort tracker.
//! The update itself is `RainWorldGame::lizard_ai_update` (`game_lizard_act.rs`).

use crate::creature::{DiscomfortTracker, StuckTracker};

/// `LizardAI` as far as lizard step 4 goes.
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
        }
    }
}
