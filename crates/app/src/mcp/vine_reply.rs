//! What `get_state` reports of climbable vines: each realized room's `climbableVines` with
//! its vines, and the player's place on one.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{ClimbableVine, ClimbableVinesSystem, VinePosition};
use serde::Serialize;

use super::placed_cosmetics_reply::RoomName;

/// One registered vine.
#[derive(Debug, Clone, Serialize)]
pub struct VineState {
    pub id: u64,
    /// The implementing class (`CoralNeuron`, `CoralStem`, ...).
    pub kind: &'static str,
    pub climbable: bool,
    /// `TotalLength`.
    pub total_length: f32,
    pub points: Vec<[f32; 2]>,
}

/// A realized room's `ClimbableVinesSystem`.
#[derive(Debug, Clone, Serialize)]
pub struct ClimbableVinesState {
    pub room: String,
    pub vine_count: usize,
    pub vines: Vec<VineState>,
    /// Calls on vines the owners have not taken yet.
    pub pending_events: usize,
}

/// The player's `vinePos`.
#[derive(Debug, Clone, Serialize)]
pub struct VinePosState {
    pub vine: u64,
    pub float_pos: f32,
}

impl VinePosState {
    #[must_use]
    pub fn of(pos: VinePosition) -> Self {
        Self {
            vine: pos.vine.0,
            float_pos: pos.float_pos,
        }
    }
}

impl ClimbableVinesState {
    /// Every realized room with a system.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((i, world.realized(i)?.climbable_vines.as_ref()?)))
            .map(|(i, system)| Self {
                room: RoomName::of(world, i),
                vine_count: system.vines().len(),
                vines: system
                    .vines()
                    .iter()
                    .map(|v| Self::vine(system, v.as_ref()))
                    .collect(),
                pending_events: system.events().len(),
            })
            .collect()
    }

    fn vine(system: &ClimbableVinesSystem, v: &dyn ClimbableVine) -> VineState {
        VineState {
            id: v.id().0,
            kind: v.kind().name(),
            climbable: v.currently_climbable(),
            total_length: system.total_length(v),
            points: (0..v.total_positions())
                .map(|i| v.pos(i).to_array())
                .collect(),
        }
    }
}
