//! A room's `ShortcutHelper`: the pushers at its shortcut entrances (wrong holes push the
//! player back out), the objects it pushes out of dead shortcuts, and the round-robin
//! that finds them. The behaviour is in `shortcut_helper_turn.rs` and
//! `player/shortcut_push.rs`.
//!
//! Mirrors: ShortcutHelper.cs (ShortcutHelper, ShortcutPusher, ObjectPusher fields,
//!   CanPullIntoShortcut)

use rw_assets::{Room, ShortcutType};

use crate::water::ChunkOwner;
use crate::water::world::LayerEntry;
use crate::{FloatExt, Vec2};

/// `ShortcutHelper.ShortcutPusher`: one shortcut entrance.
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutPusher {
    pub wrong_hole: bool,
    pub shortcut_pos: (i32, i32),
    pub shortcut_dir: (i32, i32),
    pub push_pos: Vec2,
    pub swell: f32,
    pub swell_up: bool,
    pub floor: bool,
    /// Indices into the helper's pushers (`validNeighbors`).
    pub valid_neighbors: Vec<usize>,
}

impl ShortcutPusher {
    pub fn new(
        room: &Room,
        wrong_hole: bool,
        shortcut_pos: (i32, i32),
        shortcut_dir: (i32, i32),
    ) -> Self {
        let beyond = (
            shortcut_pos.0 + shortcut_dir.0,
            shortcut_pos.1 + shortcut_dir.1,
        );
        Self {
            wrong_hole,
            shortcut_pos,
            shortcut_dir,
            push_pos: Vec2::middle_of_tile(beyond),
            swell: 0.0,
            swell_up: false,
            floor: shortcut_dir.1 > 0
                && room.tile(beyond.0, beyond.1).terrain == rw_assets::Terrain::Floor,
            valid_neighbors: Vec::new(),
        }
    }

    /// `LookForNeighbors` of pusher `j` against `k` (> `j`): a wrong hole next to a right
    /// one facing the same way lists it as a valid neighbour.
    fn look_for_neighbors(pushers: &mut [Self], j: usize, k: usize) {
        let (me, other) = (&pushers[j], &pushers[k]);
        let (dir, pos) = (me.shortcut_dir, me.shortcut_pos);
        if other.wrong_hole == me.wrong_hole
            || other.shortcut_dir != dir
            || (other.shortcut_pos.0 - pos.0).abs() > 2
            || (other.shortcut_pos.1 - pos.1).abs() > 2
            || (dir.1 != 0 && other.shortcut_pos.1 != pos.1)
            || (dir.0 != 0 && other.shortcut_pos.0 != pos.0)
        {
            return;
        }
        let (wrong, valid) = if me.wrong_hole { (j, k) } else { (k, j) };
        pushers[wrong].valid_neighbors.push(valid);
    }

    /// The end of `ShortcutHelper.Update`'s pusher loop.
    pub(crate) fn settle_swell(&mut self) {
        let target = if self.swell_up { 1.0 } else { 0.0 };
        self.swell = self.swell.lerp_and_tick(target, 0.02, 1.0 / 30.0);
        self.swell_up = false;
    }
}

/// `ShortcutHelper.ObjectPusher`: an object being pushed out of a shortcut entrance.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectPusher {
    pub owner: ChunkOwner,
    pub counter: i32,
    pub has_been_out_counter: i32,
    pub wait_counter: i32,
    pub search_towards_side: i32,
    pub search_towards_side_x_tile: i32,
    pub slated_for_deletion: bool,
    pub given_up: bool,
}

impl ObjectPusher {
    pub fn new(owner: ChunkOwner) -> Self {
        Self {
            owner,
            counter: 0,
            has_been_out_counter: 0,
            wait_counter: 0,
            search_towards_side: 0,
            search_towards_side_x_tile: 0,
            slated_for_deletion: false,
            given_up: false,
        }
    }
}

/// A room's `ShortcutHelper`.
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutHelper {
    pub pushers: Vec<ShortcutPusher>,
    pub push_out_objects: Vec<ObjectPusher>,
    pub check_obj: i32,
    /// Its place in the room's update list.
    pub update_order: u64,
}

impl ShortcutHelper {
    /// The constructor: a pusher per shortcut, then the neighbour links.
    pub fn new(
        room: &Room,
        connections: &[i32],
        single_room_world: bool,
        gate: bool,
        update_order: u64,
    ) -> Self {
        let mut pushers: Vec<ShortcutPusher> = room
            .shortcut_map()
            .shortcuts()
            .iter()
            .map(|s| {
                let wrong =
                    Self::wrong_hole(s.kind, s.dest_node, connections, single_room_world, gate);
                let dir = room.shortcut_hole_direction(s.start.0, s.start.1);
                ShortcutPusher::new(room, wrong, s.start, dir)
            })
            .collect();
        for j in 0..pushers.len() {
            for k in j + 1..pushers.len() {
                ShortcutPusher::look_for_neighbors(&mut pushers, j, k);
            }
        }
        Self {
            pushers,
            push_out_objects: Vec::new(),
            check_obj: 0,
            update_order,
        }
    }

    /// Whether a shortcut of `kind` to node `dest_node` is a wrong hole: anything but a
    /// normal tube or a room exit, and a room exit leading nowhere (`connections` -1) in
    /// a multi-room world outside gates.
    pub fn wrong_hole(
        kind: ShortcutType,
        dest_node: Option<usize>,
        connections: &[i32],
        single_room_world: bool,
        gate: bool,
    ) -> bool {
        if kind != ShortcutType::Normal && kind != ShortcutType::RoomExit {
            return true;
        }
        kind == ShortcutType::RoomExit
            && !single_room_world
            && !gate
            && dest_node.is_some_and(|n| connections.get(n) == Some(&-1))
    }

    /// The round-robin's pick: in each non-empty layer of `physicalObjects` (`objects`
    /// sorted by layer), the object at `check_obj % count`.
    pub(crate) fn checked_objects(objects: &[LayerEntry], check_obj: i32) -> Vec<ChunkOwner> {
        objects
            .chunk_by(|a, b| a.0 == b.0)
            .map(|layer| layer[check_obj.rem_euclid(layer.len() as i32) as usize].2)
            .collect()
    }

    /// `AddPushOutObject`: one pusher per object.
    pub(crate) fn add_push_out_object(&mut self, owner: ChunkOwner) {
        if self.push_out_objects.iter().all(|p| p.owner != owner) {
            self.push_out_objects.push(ObjectPusher::new(owner));
        }
    }
}

#[cfg(test)]
#[path = "tests/shortcut_helper.rs"]
mod tests;
