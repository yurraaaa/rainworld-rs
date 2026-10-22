//! What `get_state` reports of the realized rooms' cosmetic slime molds and dead token
//! stalks.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{CosmeticSlimeMold, TokenStalk};
use serde::Serialize;

/// One `CosmeticSlimeMold`.
#[derive(Debug, Clone, Serialize)]
pub struct SlimeMoldState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub dots: usize,
    /// The first dot's position, radius and brightness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<([f32; 2], f32, f32)>,
}

/// One dead `TokenStalk`.
#[derive(Debug, Clone, Serialize)]
pub struct TokenStalkState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub hover_pos: [f32; 2],
    pub base_pos: [f32; 2],
    pub flip: f32,
    pub arm_pos: [f32; 2],
    pub arm_get_to_pos: [f32; 2],
    pub head: [f32; 2],
    pub head_dir: [f32; 2],
    pub head_dist: f32,
    pub segments: usize,
    pub lamp_power: f32,
    pub sin_counter: f32,
    /// The two curves' `[progress, from, to]` (head angle, head distance).
    pub curves: [[f32; 3]; 2],
}

impl SlimeMoldState {
    /// Every realized room's patches.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| r.slime_molds.iter().map(move |m| Self::of(room.clone(), m)))
            .collect()
    }

    fn of(room: String, m: &CosmeticSlimeMold) -> Self {
        Self {
            room,
            id: m.update_order,
            dots: m.positions.len(),
            first: m
                .positions
                .first()
                .map(|p| (p.to_array(), m.rads[0], m.brightnesses[0])),
        }
    }
}

impl TokenStalkState {
    /// Every realized room's dead stalks.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                r.token_stalks
                    .iter()
                    .map(move |s| Self::of(room.clone(), s))
            })
            .collect()
    }

    fn of(room: String, s: &TokenStalk) -> Self {
        Self {
            room,
            id: s.update_order,
            hover_pos: s.hover_pos.to_array(),
            base_pos: s.base_pos.to_array(),
            flip: s.flip,
            arm_pos: s.arm_pos.to_array(),
            arm_get_to_pos: s.arm_get_to_pos.to_array(),
            head: s.head.to_array(),
            head_dir: s.head_dir.to_array(),
            head_dist: s.head_dist,
            segments: s.coord.len(),
            lamp_power: s.lamp_power,
            sin_counter: s.sin_counter,
            curves: s.curve_lerps.map(|l| [l.progress, l.from, l.to]),
        }
    }
}

/// A room's name for the replies.
struct RoomName;

impl RoomName {
    fn of(world: &World, index: i32) -> String {
        world
            .abstract_room(index)
            .map_or_else(|| format!("#{index}"), |r| r.name.clone())
    }
}
