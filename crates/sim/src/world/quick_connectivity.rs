//! `QuickConnectivity`: `Check` (a bounded search back from the goal for whether the
//! start connects to it) and `DefineNodeOfLocalCoordinate`. Decompile
//! `QuickConnectivity.cs`. Behaviour: `rainworld-re/notes/ai-map.md`.

use rustc_hash::FxHashSet;
use rw_random::UnityRandom;

use super::ai_map::FOUR_DIRECTIONS_AND_ZERO;
use super::{AiMap, RealizedRoom, World};
use crate::creature::{CreatureTemplate, WorldCoordinate};

/// `QuickConnectivity`.
pub struct QuickConnectivity;

impl QuickConnectivity {
    /// `Check`: the generations (tiles taken off the queue) a search from `goal` over
    /// incoming connections no worse than the start tile's legality needs to meet
    /// `start`; -1 past `max_generations` or when the queue runs dry. The queue is kept
    /// sorted by Manhattan distance to the start, a tile nearer than the one being
    /// expanded going before the first that is not.
    #[must_use]
    pub fn check(
        map: &AiMap,
        template: &CreatureTemplate,
        start: (i32, i32),
        goal: (i32, i32),
        max_generations: i32,
    ) -> i32 {
        if start == goal {
            return 0;
        }
        let (w, h) = (map.width(), map.height());
        let start = (start.0.clamp(0, w - 1), start.1.clamp(0, h - 1));
        let goal = (goal.0.clamp(0, w - 1), goal.1.clamp(0, h - 1));
        let legality = map.tile_cost_for_creature(start, template).legality;
        let mut checked: FxHashSet<i32> = FxHashSet::default();
        let mut queue = vec![goal];
        for i in 0..max_generations {
            if queue.is_empty() {
                return -1;
            }
            let t = queue.remove(0);
            let here = (t.0 - start.0).abs() + (t.1 - start.1).abs();
            for c in &map.tile(t.0, t.1).incoming_paths {
                let s = c.start_tile();
                let key = s.0 * h + s.1;
                if checked.contains(&key)
                    || template.connection_resistance(c.kind).legality > legality
                    || map.tile_cost_for_creature(s, template).legality > legality
                {
                    continue;
                }
                if s == start {
                    return i;
                }
                checked.insert(key);
                let last_nearer = queue
                    .last()
                    .is_none_or(|l| (l.0 - start.0).abs() + (l.1 - start.1).abs() < here);
                if last_nearer {
                    queue.push(s);
                    continue;
                }
                let at = queue
                    .iter()
                    .position(|q| (q.0 - start.0).abs() + (q.1 - start.1).abs() >= here)
                    .unwrap_or(queue.len());
                queue.insert(at, s);
            }
        }
        -1
    }
}

impl World {
    /// `QuickConnectivity.DefineNodeOfLocalCoordinate`: a defined node as is; else a
    /// random relevant node (one draw, a second over all nodes when there is none). In a
    /// room ready for AI the node is then chosen: from a tile (or a neighbour) some exit
    /// is reachable from, the nearest node by the template's exit distance; else the
    /// node with the fewest `Check` generations there and back (200 each way when it
    /// does not connect, under 400 in all), ties to the nearer node tile.
    pub(crate) fn define_node(
        &self,
        coord: WorldCoordinate,
        t: &CreatureTemplate,
        rng: &mut UnityRandom,
    ) -> WorldCoordinate {
        if coord.node_defined() {
            return coord;
        }
        let Some(room) = self.abstract_room(coord.room) else {
            return coord;
        };
        let specific = rng.range_i32(0, t.nodes_relevant(room));
        let mut node = t.common_node_index(room, specific);
        if node < 0 {
            let n = i32::try_from(room.nodes.len()).unwrap_or(i32::MAX);
            node = rng.range_i32(0, n);
        }
        let Some(realized) = self.realized(coord.room).filter(|r| r.ready_for_ai()) else {
            return WorldCoordinate { node, ..coord };
        };
        let Some(map) = realized.ai_map.as_ref() else {
            return WorldCoordinate { node, ..coord };
        };
        let near = FOUR_DIRECTIONS_AND_ZERO
            .iter()
            .map(|&(dx, dy)| (coord.tile.0 + dx, coord.tile.1 + dy))
            .find(|&p| map.any_exit_reachable_from_tile(p, t));
        if let Some(pos) = near {
            let mut best = i32::MAX;
            for j in 0..t.nodes_relevant(room) {
                let d = map.exit_distance_for_creature(pos, j, t);
                if d > 0 && d < best {
                    best = d;
                    node = t.common_node_index(room, j);
                }
            }
            return WorldCoordinate { node, ..coord };
        }
        self.define_node_by_connectivity(coord, realized, map, t, node, rng)
    }

    /// `DefineNodeOfLocalCoordinate`'s search over every node's tile (each drawn as
    /// `LocalCoordinateOfNode` draws it).
    fn define_node_by_connectivity(
        &self,
        coord: WorldCoordinate,
        realized: &RealizedRoom,
        map: &AiMap,
        t: &CreatureTemplate,
        fallback: i32,
        rng: &mut UnityRandom,
    ) -> WorldCoordinate {
        let mut distance = (realized.room.width() + realized.room.height()) as f32;
        let mut generations = 400;
        let mut node = fallback;
        let nodes = self.abstract_room(coord.room).map_or(0, |r| r.nodes.len());
        for k in 0..nodes {
            let k = i32::try_from(k).unwrap_or(-1);
            let tile = self.local_coordinate_of_node(coord.room, k, rng);
            let (dx, dy) = (
                (coord.tile.0 - tile.0) as f32,
                (coord.tile.1 - tile.1) as f32,
            );
            let d = (dx * dx + dy * dy).sqrt();
            let mut there = QuickConnectivity::check(map, t, tile, coord.tile, generations / 2);
            if there == -1 {
                there = 200;
            }
            let mut back = QuickConnectivity::check(map, t, coord.tile, tile, generations / 2);
            if back == -1 {
                back = 200;
            }
            let sum = there + back;
            if sum < generations || (sum == generations && d < distance) {
                distance = d;
                generations = sum;
                node = k;
            }
        }
        WorldCoordinate { node, ..coord }
    }
}
