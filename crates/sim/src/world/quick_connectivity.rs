//! `QuickConnectivity`: `Check` (a bounded search back from the goal for whether the
//! start connects to it), `QuickPath` (the same search, then the path walked) and
//! `DefineNodeOfLocalCoordinate`. Decompile
//! `QuickConnectivity.cs`. Behaviour: `rainworld-re/notes/ai-map.md`.
//!
//! Mirrors: QuickConnectivity.cs

use rustc_hash::{FxHashMap, FxHashSet};
use rw_random::UnityRandom;

use super::ai_map::FOUR_DIRECTIONS_AND_ZERO;
use super::{AiMap, Legality, PathCost, RealizedRoom, World};
use crate::creature::{CreatureTemplate, WorldCoordinate};
use crate::math::vec2::TileExt;

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

    /// `QuickPath`: a search back from `goal` over incoming connections within
    /// `max_distance` (straight tiles) of `start`, taking (in an open medium) the queued
    /// tile nearest the start, for at most `max_generations`; then the walk from `start`
    /// down the cheapest outgoing costs, written over the start of `path`. Returns the
    /// tiles walked before `goal` (1 when `start` is `goal`), 0 for no path. The
    /// frame's iteration quota never stops it (its flag is never set).
    #[allow(clippy::too_many_arguments)]
    pub fn quick_path(
        map: &AiMap,
        template: &CreatureTemplate,
        start: (i32, i32),
        goal: (i32, i32),
        max_distance: i32,
        max_generations: i32,
        in_open_medium: bool,
        path: &mut Vec<(i32, i32)>,
    ) -> usize {
        if start == goal {
            if path.is_empty() {
                path.push(start);
            } else {
                path[0] = start;
            }
            return 1;
        }
        if start.float_dist(goal) > max_distance as f32
            || !map.tile_accessible_to_creature(start, template)
            || !map.tile_accessible_to_creature(goal, template)
        {
            return 0;
        }
        let (w, h) = (map.width(), map.height());
        let start = (start.0.clamp(0, w - 1), start.1.clamp(0, h - 1));
        let goal = (goal.0.clamp(0, w - 1), goal.1.clamp(0, h - 1));
        let unset = PathCost::new(-1.0, Legality::Unallowed);
        let mut matrix: FxHashMap<i32, PathCost> = FxHashMap::default();
        let mut next = vec![goal];
        let mut dists = Vec::new();
        if in_open_medium {
            dists.push(goal.float_dist(start));
        }
        matrix.insert(goal.0 * h + goal.1, PathCost::new(0.0, Legality::Allowed));
        let mut found = false;
        for _ in 0..max_generations {
            if found {
                break;
            }
            if next.is_empty() {
                return 0;
            }
            let mut index = 0;
            if in_open_medium {
                let mut best = f32::MAX;
                for (j, &d) in dists.iter().enumerate() {
                    if d < best {
                        index = j;
                        best = d;
                    }
                }
            }
            let t = next.remove(index);
            if in_open_medium {
                dists.remove(index);
            }
            let cost = *matrix.entry(t.0 * h + t.1).or_insert(unset);
            for c in &map.tile(t.0, t.1).incoming_paths {
                let step = template.connection_resistance(c.kind)
                    + map.tile_cost_for_creature(c.start_tile(), template);
                if !step.allowed() {
                    continue;
                }
                let s = c.start_tile();
                let key = s.0 * h + s.1;
                if matrix.entry(key).or_insert(unset).resistance >= 0.0 {
                    continue;
                }
                let d = s.float_dist(start);
                if d <= max_distance as f32 {
                    matrix.insert(key, cost + step);
                    next.push(s);
                    if in_open_medium {
                        dists.push(d);
                    }
                    if s == start {
                        found = true;
                        break;
                    }
                }
            }
        }
        if !found {
            return 0;
        }
        let mut at = start;
        let mut n = 0;
        for _ in 0..max_generations {
            let mut best = PathCost::new(0.0, Legality::Unallowed);
            let mut to = at;
            for c in &map.tile(at.0, at.1).outgoing_paths {
                let d = c.dest_tile();
                let v = matrix.get(&(d.0 * h + d.1)).copied().unwrap_or(unset);
                if v.less_than(best) {
                    best = v;
                    to = d;
                }
            }
            if !best.allowed() {
                return 0;
            }
            if path.len() <= n {
                path.push(at);
            } else {
                path[n] = at;
            }
            n += 1;
            at = to;
            if to == goal {
                return n;
            }
        }
        0
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
