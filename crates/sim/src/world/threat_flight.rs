//! `ThreatTracker`'s flight: `FleeTo` (random tiles around the creature scored by the
//! threat along the way there, the saved destination changed only for a clearly
//! better one, or a room exit away from the threats), `EvaluateFlightDestThreat`,
//! `ThreatOfPath`, and `GenerateRandomPath` and `FindMostAttractiveExit`, which no
//! lizard calls (the first nothing in the game calls; the second the mission tracker,
//! firecracker plants and the More Slugcats yeeks). Decompile: `ThreatTracker.cs`.
//! Behaviour: `rainworld-re/notes/lizard-ai.md` section 9.
//!
//! Mirrors: ThreatTracker.cs (FleeTo, GenerateRandomPath, EvaluateFlightDestThreat, ThreatOfPath, FindMostAttractiveExit)

use rw_assets::{CreatureType, NodeType};
use rw_random::UnityRandom;

use super::ai_map::FOUR_DIRECTIONS;
use super::{QuickConnectivity, World};
use crate::FloatExt;
use crate::RoomExt;
use crate::creature::{CreatureId, CreatureTemplate, WorldCoordinate};
use crate::vec2::TileExt;

impl World {
    /// `FleeTo(occupyTile, reevaluations, maximumDistance, considerLeavingRoom,
    /// considerGoingHome)` of AI `ai`'s threat tracker; `most_guess` is the most
    /// threatening creature's `BestGuessForPosition()` (none without one). The
    /// reevaluations are shared out by the path finders' budget; a tile the path finder
    /// cannot use is moved to the first usable neighbour; up to `reevaluations * 5`
    /// random tiles (four draws each) within `maximumDistance` are tried, of which
    /// `reevaluations` accessible ones more than 3 tiles from the test destination are
    /// scored, the least threatening kept as the test destination; it becomes the saved
    /// one (for 20 ticks) when clearly less threatening. Leaving the room, the nearest
    /// exit (or, going home, border exit or region transport) within
    /// `maximumDistance` that is not the entrance it came by in its first 100 ticks, does
    /// not lead to the threat and has no threat point nearer it wins: through a room
    /// exit to the far node of the next room, else to the off-screen den.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn threat_flee_to(
        &mut self,
        ai: CreatureId,
        mut occupy: WorldCoordinate,
        reevaluations: i32,
        max_distance: i32,
        consider_leaving_room: bool,
        consider_going_home: bool,
        most_guess: Option<WorldCoordinate>,
        rng: &mut UnityRandom,
    ) -> WorldCoordinate {
        let reevaluations = self
            .path_finders
            .divider
            .request_pathfinder_updates(reevaluations);
        if !self.flee_viable(ai, occupy) {
            for d in FOUR_DIRECTIONS {
                let n = occupy.add_tile(d);
                if self.flee_viable(ai, n) {
                    occupy = n;
                    break;
                }
            }
        }
        let c = self.creature(ai);
        let (room, template) = (c.pos.room, c.template);
        let (w, h) = self
            .realized(room)
            .map_or((1, 1), |r| (r.room.width(), r.room.height()));
        let mut path = Vec::new();
        let mut scored = 0;
        let test = self.lizard_ref(ai).ai.threat.test_flee_dest;
        let mut best = self.evaluate_flight_dest_threat(ai, occupy, test, max_distance, &mut path);
        for _ in 0..reevaluations * 5 {
            let dx = rng.range_i32(0, max_distance) * if rng.value() < 0.5 { -1 } else { 1 };
            let dy = rng.range_i32(0, max_distance) * if rng.value() < 0.5 { -1 } else { 1 };
            let coord = WorldCoordinate::new(
                occupy.room,
                (occupy.tile.0 + dx).clamp(0, w - 1),
                (occupy.tile.1 + dy).clamp(0, h - 1),
                -1,
            );
            let test = self.lizard_ref(ai).ai.threat.test_flee_dest;
            if test.tile.float_dist(coord.tile) > 3.0 && self.threat_map_accessible(ai, coord) {
                let v =
                    self.evaluate_flight_dest_threat(ai, occupy, coord, max_distance, &mut path);
                if v < best {
                    self.lizard_at(ai).ai.threat.test_flee_dest = coord;
                    best = v;
                }
                scored += 1;
            }
            if scored >= reevaluations {
                break;
            }
        }
        let th = &self.lizard_ref(ai).ai.threat;
        let (saved, test, current) = (th.saved_flee_dest, th.test_flee_dest, th.current_threat);
        if th.anti_flicker_counter < 1
            && saved != test
            && best
                < self.evaluate_flight_dest_threat(ai, occupy, saved, max_distance, &mut path)
                    - 0.5 * current.inverse_lerp(1.0, 0.5)
        {
            let th = &mut self.lizard_at(ai).ai.threat;
            th.saved_flee_dest = test;
            th.anti_flicker_counter = 20;
        }
        self.lizard_at(ai).ai.threat.flee_threat = best;
        if consider_leaving_room
            && let Some(dest) = self.flee_exit(
                ai,
                occupy,
                max_distance,
                consider_going_home,
                most_guess,
                &template,
            )
        {
            return dest;
        }
        self.lizard_ref(ai).ai.threat.saved_flee_dest
    }

    /// `FleeTo`'s way out of the room, when it finds one.
    fn flee_exit(
        &self,
        ai: CreatureId,
        occupy: WorldCoordinate,
        max_distance: i32,
        consider_going_home: bool,
        most_guess: Option<WorldCoordinate>,
        template: &CreatureTemplate,
    ) -> Option<WorldCoordinate> {
        let c = self.creature(ai);
        let here = c.pos.room;
        let data = self.abstract_room(here)?;
        let map = self.realized(here).and_then(|r| r.ai_map.as_deref())?;
        let fly = CreatureTemplate::of(CreatureType::Fly, self.mods());
        let l = self.lizard_ref(ai);
        let pf = self.path_finder(ai);
        let mut best_node = -1;
        let mut best_dist = i32::MAX;
        for k in 0..template.nodes_relevant(data) {
            let mut d =
                map.exit_distance_for_creature_and_check_neighbours(occupy.tile, k, template);
            let common = template.common_node_index(data, k);
            if pf.is_some_and(|p| l.ai.time_in_room < 100 && p.forbidden_entrance.node == common) {
                d = -1;
            }
            let Some(node) = usize::try_from(common).ok().and_then(|n| data.nodes.get(n)) else {
                continue;
            };
            let connection = usize::try_from(common)
                .ok()
                .and_then(|n| data.connections.get(n))
                .copied()
                .unwrap_or(-1);
            let open_exit = node.kind == NodeType::Exit && connection > -1;
            if open_exit && most_guess.is_some_and(|g| g.room == connection) {
                d = -1;
            }
            if d > 0 {
                let mine =
                    map.exit_distance_for_creature_and_check_neighbours(occupy.tile, k, &fly);
                if mine > 5 {
                    for p in l.ai.threat.points.iter().filter(|p| p.pos.room == here) {
                        let mut theirs = map
                            .exit_distance_for_creature_and_check_neighbours(p.pos.tile, k, &fly);
                        if theirs > 0 {
                            theirs += 10;
                            if theirs < mine {
                                d = -1;
                                break;
                            }
                        }
                    }
                }
            }
            let wanted = open_exit
                || (node.kind.is_border_exit() && consider_going_home)
                || (node.kind == NodeType::RegionTransportation && consider_going_home);
            if d > 0 && d < best_dist && wanted {
                best_node = common;
                best_dist = d;
            }
        }
        if best_node < 0 || best_dist >= max_distance {
            return None;
        }
        let node = usize::try_from(best_node).ok()?;
        match data.nodes[node].kind {
            NodeType::Exit => {
                let next = *data.connections.get(node)?;
                let other = self.abstract_room(next)?;
                let entrance = other.exit_index(here);
                let mut far = entrance;
                let mut longest = i32::MIN;
                for m in 0..i32::try_from(other.nodes.len()).unwrap_or(0) {
                    let len = self.connection_length(next, entrance, m, template);
                    if self.connection_and_back_possible(next, entrance, m, template)
                        && len > longest
                    {
                        far = m;
                        longest = len;
                    }
                }
                Some(WorldCoordinate::new(next, -1, -1, far))
            }
            NodeType::RegionTransportation
            | NodeType::SeaExit
            | NodeType::SideExit
            | NodeType::SkyExit => {
                Some(WorldCoordinate::new(self.offscreen_den_index(), -1, -1, 0))
            }
            _ => None,
        }
    }

    /// `pathFinder.CoordinateViable` of AI `ai` (false without a path finder).
    fn flee_viable(&self, ai: CreatureId, coord: WorldCoordinate) -> bool {
        self.path_finder(ai)
            .is_some_and(|p| p.coordinate_viable(coord))
    }

    /// The threat tracker's `aiMap.WorldCoordinateAccessibleToCreature`.
    fn threat_map_accessible(&self, ai: CreatureId, coord: WorldCoordinate) -> bool {
        let c = self.creature(ai);
        let room = self.lizard_ref(ai).ai.threat.map_room;
        self.realized(room)
            .and_then(|r| r.ai_map.as_deref())
            .is_some_and(|m| m.world_coordinate_accessible_to_creature(coord, &c.template))
    }

    /// `EvaluateFlightDestThreat`: the most for another room, `2 * maximumDistance` tiles
    /// or more away, or a tile the path finder cannot use; else the threat of the
    /// straight line there when every tile of it is accessible, or of the quick path
    /// there (none: the most).
    fn evaluate_flight_dest_threat(
        &self,
        ai: CreatureId,
        occupy: WorldCoordinate,
        coord: WorldCoordinate,
        max_distance: i32,
        path: &mut Vec<(i32, i32)>,
    ) -> f32 {
        if coord.room != occupy.room
            || coord.manhattan_distance(occupy) >= max_distance * 2
            || !self.flee_viable(ai, coord)
        {
            return f32::MAX;
        }
        let c = self.creature(ai);
        let Some(r) = self.realized(c.pos.room) else {
            return f32::MAX;
        };
        let n = r.room.ray_trace_tiles_list(occupy.tile, coord.tile, path);
        let tracker_map = self
            .realized(self.lizard_ref(ai).ai.threat.map_room)
            .and_then(|r| r.ai_map.as_deref());
        let straight = path[..n]
            .iter()
            .all(|&t| tracker_map.is_some_and(|m| m.tile_accessible_to_creature(t, &c.template)));
        if straight {
            return self.threat_of_path(ai, &path[..n]);
        }
        let Some(map) = r.ai_map.as_deref() else {
            return f32::MAX;
        };
        let n = QuickConnectivity::quick_path(
            map,
            &c.template,
            occupy.tile,
            coord.tile,
            max_distance * 2,
            500,
            true,
            path,
        );
        self.threat_of_path(ai, &path[..n])
    }

    /// `ThreatOfPath`: the tiles' threat plus ten times the severity of each threat point
    /// on them, 100 more for a one-tile path ending out of threat (under 0.5), 1000 more
    /// when it ends in a more threatened area than it starts, over the length, plus the
    /// end's area threat. An empty path: the most.
    fn threat_of_path(&self, ai: CreatureId, path: &[(i32, i32)]) -> f32 {
        let (Some(&first), Some(&last)) = (path.first(), path.last()) else {
            return f32::MAX;
        };
        let room = self.creature(ai).pos.room;
        let (first, last) = (
            WorldCoordinate::new(room, first.0, first.1, -1),
            WorldCoordinate::new(room, last.0, last.1, -1),
        );
        let points = &self.lizard_ref(ai).ai.threat.points;
        let mut num = 0.0;
        for &t in path {
            num += self.threat_of_tile(ai, WorldCoordinate::new(room, t.0, t.1, -1));
            for p in points {
                if p.pos.tile == t {
                    num += 10.0 * p.severity;
                }
            }
        }
        if path.len() < 2 && self.threat_of_tile(ai, last) < 0.5 {
            num += 100.0;
        }
        if self.threat_of_area(ai, last) > self.threat_of_area(ai, first) {
            num += 1000.0;
        }
        num /= path.len() as f32;
        num + self.threat_of_area(ai, last)
    }

    /// `GenerateRandomPath(occupyTile, length)`: none from a tile the path finder cannot
    /// use; else up to `length` steps, each to the allowed outgoing connection with the
    /// highest draw (one draw per allowed connection; a tile already on the path scores
    /// 0), stopping when none scores. The path holds the tiles stepped from.
    pub fn threat_generate_random_path(
        &self,
        ai: CreatureId,
        occupy: WorldCoordinate,
        length: i32,
        rng: &mut UnityRandom,
    ) -> Option<Vec<(i32, i32)>> {
        if !self.flee_viable(ai, occupy) {
            return None;
        }
        let c = self.creature(ai);
        let map = self
            .realized(c.pos.room)
            .and_then(|r| r.ai_map.as_deref())?;
        let t = &c.template;
        let mut list: Vec<(i32, i32)> = Vec::new();
        let mut cur = occupy.tile;
        for _ in 0..length {
            let mut next = cur;
            let mut num = 0.0;
            for m in &map.tile(cur.0, cur.1).outgoing_paths {
                if !t.connection_resistance(m.kind).allowed()
                    || !t
                        .accessibility_resistance(map.tile_at(m.dest).acc)
                        .allowed()
                {
                    continue;
                }
                let mut v = rng.value();
                if v <= num {
                    continue;
                }
                for &k in &list {
                    if v <= 0.0 {
                        break;
                    }
                    if k == m.dest_tile() {
                        v = 0.0;
                    }
                }
                if v > num {
                    next = m.dest_tile();
                    num = v;
                }
            }
            if num <= 0.0 {
                break;
            }
            list.push(cur);
            cur = next;
        }
        Some(list)
    }

    /// `FindMostAttractiveExit`: of the room's open exits the creature maps, the one
    /// whose exit distance most exceeds the threat points' (none nearer it than the
    /// creature), over the threat of the area at the node (`LocalCoordinateOfNode`:
    /// a draw for a border exit, hive or garbage hole); -1 when none scores 0 or more.
    pub fn threat_find_most_attractive_exit(&self, ai: CreatureId, rng: &mut UnityRandom) -> i32 {
        let map_room = self.lizard_ref(ai).ai.threat.map_room;
        let Some(map) = self.realized(map_room).and_then(|r| r.ai_map.as_deref()) else {
            return -1;
        };
        let c = self.creature(ai);
        let Some(data) = self.abstract_room(c.pos.room) else {
            return -1;
        };
        let t = &c.template;
        let points = &self.lizard_ref(ai).ai.threat.points;
        let mut best = f32::MIN;
        let mut result = -1;
        for (i, node) in data.nodes.iter().enumerate() {
            let common = i32::try_from(i).unwrap_or(-1);
            let connection = data.connections.get(i).copied().unwrap_or(-1);
            if !t.maps_node(node.kind) || (node.kind == NodeType::Exit && connection <= -1) {
                continue;
            }
            let mine =
                map.exit_distance_for_creature(c.pos.tile, t.specific_node_index(data, common), t);
            let mut num = 0.0;
            for p in points.iter().filter(|p| p.pos.room == c.pos.room) {
                let theirs = match &p.crit {
                    None => map.exit_distance_for_creature(
                        p.pos.tile,
                        t.specific_node_index(data, common),
                        t,
                    ),
                    Some(crit) => map.exit_distance_for_creature(
                        p.pos.tile,
                        crit.specific_node_index(data, common),
                        crit,
                    ),
                };
                if theirs < mine {
                    num = -1.0;
                    break;
                }
                num += (theirs - mine) as f32;
            }
            let (x, y) = self.local_coordinate_of_node(map_room, common, rng);
            num /= self.threat_of_area(ai, WorldCoordinate::new(map_room, x, y, -1));
            if node.kind != NodeType::Exit {
                num = f32::MIN;
            }
            if num > best {
                best = num;
                result = common;
            }
        }
        if best >= 0.0 { result } else { -1 }
    }
}
