//! What `get_state` reports of the realized rooms' `ShortcutHelper`s.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{ObjectPusher, ShortcutHelper, ShortcutPusher};
use serde::Serialize;

/// One `ShortcutPusher`.
#[derive(Debug, Clone, Serialize)]
pub struct ShortcutPusherState {
    pub tile: [i32; 2],
    pub dir: [i32; 2],
    pub push_pos: [f32; 2],
    pub wrong_hole: bool,
    pub floor: bool,
    pub swell: f32,
    /// Indices of the `validNeighbors` in this room's pusher list.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub valid_neighbors: Vec<usize>,
}

/// One `ObjectPusher`.
#[derive(Debug, Clone, Serialize)]
pub struct PushOutObjectState {
    /// The pushed body (`Player`, `Entity(Creature(..))`, `Entity(Object(..))`).
    pub object: String,
    pub counter: i32,
    pub has_been_out_counter: i32,
    pub wait_counter: i32,
    pub search_towards_side: i32,
    pub slated_for_deletion: bool,
    pub given_up: bool,
}

/// One room's `ShortcutHelper`.
#[derive(Debug, Clone, Serialize)]
pub struct ShortcutHelperState {
    pub room: String,
    pub id: u64,
    pub check_obj: i32,
    pub pushers: Vec<ShortcutPusherState>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub push_out_objects: Vec<PushOutObjectState>,
}

impl ShortcutHelperState {
    /// Every realized room's `ShortcutHelper`.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| {
                let helper = world.realized(i)?.shortcut_helper.as_ref()?;
                let room = world
                    .abstract_room(i)
                    .map_or_else(|| format!("#{i}"), |r| r.name.clone());
                Some(Self::of(room, helper))
            })
            .collect()
    }

    fn of(room: String, h: &ShortcutHelper) -> Self {
        Self {
            room,
            id: h.update_order,
            check_obj: h.check_obj,
            pushers: h.pushers.iter().map(ShortcutPusherState::of).collect(),
            push_out_objects: h
                .push_out_objects
                .iter()
                .map(PushOutObjectState::of)
                .collect(),
        }
    }
}

impl ShortcutPusherState {
    fn of(p: &ShortcutPusher) -> Self {
        Self {
            tile: [p.shortcut_pos.0, p.shortcut_pos.1],
            dir: [p.shortcut_dir.0, p.shortcut_dir.1],
            push_pos: p.push_pos.to_array(),
            wrong_hole: p.wrong_hole,
            floor: p.floor,
            swell: p.swell,
            valid_neighbors: p.valid_neighbors.clone(),
        }
    }
}

impl PushOutObjectState {
    fn of(p: &ObjectPusher) -> Self {
        Self {
            object: format!("{:?}", p.owner),
            counter: p.counter,
            has_been_out_counter: p.has_been_out_counter,
            wait_counter: p.wait_counter,
            search_towards_side: p.search_towards_side,
            slated_for_deletion: p.slated_for_deletion,
            given_up: p.given_up,
        }
    }
}
