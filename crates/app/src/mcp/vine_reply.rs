//! What `get_state` reports of climbable vines: each realized room's `climbableVines` with
//! its vines, and the player's place on one.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{ClimbableVinesSystem, VineHost, VinePosition, VineRef};
use serde::Serialize;

use super::placed_cosmetics_reply::RoomName;

/// One registered vine.
#[derive(Debug, Clone, Serialize)]
pub struct VineState {
    /// The vine's reference (`Placed { room, id }`).
    pub vine: String,
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
}

/// The player's `vinePos`.
#[derive(Debug, Clone, Serialize)]
pub struct VinePosState {
    pub vine: String,
    pub float_pos: f32,
}

impl VinePosState {
    #[must_use]
    pub fn of(pos: VinePosition) -> Self {
        Self {
            vine: format!("{:?}", pos.vine),
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
            .filter_map(|i| Some((i, world.realized(i)?.vine_view()?)))
            .map(|(i, (system, host))| Self {
                room: RoomName::of(world, i),
                vine_count: system.vines().len(),
                vines: system
                    .vines()
                    .iter()
                    .map(|&v| Self::vine(system, &host, v))
                    .collect(),
            })
            .collect()
    }

    fn vine(system: &ClimbableVinesSystem, host: &dyn VineHost, v: VineRef) -> VineState {
        VineState {
            vine: format!("{v:?}"),
            kind: host.kind(v).name(),
            climbable: host.currently_climbable(v),
            total_length: system.total_length(host, v),
            points: (0..host.total_positions(v))
                .map(|i| host.pos(v, i).to_array())
                .collect(),
        }
    }
}
