//! `RoomRealizer`: realizes the room the camera's creature is in and the room it will
//! probably go to next, and abstracts rooms to stay within a performance budget.
//!
//! Base game (the More Slugcats overseer branch is not ported). The next-room guess
//! reads the Fly's baked exit distances ([`super::RealizedRoom::exit_distance`]).

use rw_random::UnityRandom;

use super::World;
use crate::creature::IdIssuer;

/// `performanceBudget`.
const PERFORMANCE_BUDGET: f32 = 1500.0;

/// What the realizer reads of `followCreature`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowCreature {
    /// `pos.room` (global index).
    pub room: i32,
    /// `pos.Tile`.
    pub tile: (i32, i32),
    /// `realizedCreature.room != null`: the body is in a room, not in a shortcut.
    pub in_room: bool,
    pub alive: bool,
}

/// `RealizedRoomTracker`.
#[derive(Debug, Clone, PartialEq)]
struct Tracker {
    room: i32,
    time_since_visited: i32,
    age: i32,
    room_changes_since_visited: i32,
    has_been_visited: bool,
}

/// `RoomAndInt`; `room` is `None` for a missing room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RoomAndInt {
    room: Option<i32>,
    counter: i32,
}

/// `RoomRealizer`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoomRealizer {
    trackers: Vec<Tracker>,
    /// The room realized last, until it has loaded.
    currently_loading: Option<i32>,
    last_frame_follow_room: i32,
    probable_next: Option<i32>,
    improbable_next: Option<i32>,
    recently_abstracted: Vec<RoomAndInt>,
    neighbour_candidates: Vec<RoomAndInt>,
}

impl Tracker {
    fn new(room: i32, has_been_visited: bool) -> Self {
        Self {
            room,
            time_since_visited: 0,
            age: 0,
            room_changes_since_visited: 0,
            has_been_visited,
        }
    }

    fn deletion_factor(&self) -> f32 {
        self.time_since_visited as f32 / 2400.0
            + self.room_changes_since_visited as f32
            + if self.has_been_visited { 0.0 } else { 24000.0 }
    }
}

/// One realizer tick's view: the world, the followed creature and the camera's room.
struct Ctx<'a> {
    world: &'a mut World,
    follow: FollowCreature,
    camera_room: i32,
    /// For the items a room makes when realized.
    ids: &'a mut IdIssuer,
    rng: &'a mut UnityRandom,
}

impl Ctx<'_> {
    /// `RoomPerformanceEstimation`.
    fn estimate(&self, room: i32) -> f32 {
        let Some(data) = self.world.abstract_room(room) else {
            return 0.0;
        };
        let area = (data.size.0 * data.size.1) as f32;
        let mut estimate = (2080.0 + (area - 2080.0) * 0.25) / 40.0;
        if data.nodes.iter().any(|n| n.submerged) {
            estimate += (50.0 + (data.size.0 as f32 - 50.0) * 0.5) * 0.2;
        }
        if data.single_realized_room {
            estimate += PERFORMANCE_BUDGET * 0.55;
        }
        for &id in self.world.room_creatures(room) {
            let c = self.world.creature(id);
            if c.state.alive {
                estimate += c.template.performance_estimation();
            }
        }
        estimate
    }

    fn connections(&self, room: i32) -> Vec<i32> {
        self.world
            .abstract_room(room)
            .map(|r| r.connections.clone())
            .unwrap_or_default()
    }

    /// `GetAbstractRoom`, as an index that exists.
    fn existing(&self, room: i32) -> Option<i32> {
        self.world.abstract_room(room).map(|r| r.index)
    }

    fn realized(&self, room: i32) -> bool {
        self.world.realized(room).is_some()
    }
}

impl RoomRealizer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// One tick (`RoomRealizer.Update`), before the shortcuts and rooms update.
    pub fn update(
        &mut self,
        world: &mut World,
        follow: FollowCreature,
        camera_room: i32,
        ids: &mut IdIssuer,
        rng: &mut UnityRandom,
    ) {
        let mut cx = Ctx {
            world,
            follow,
            camera_room,
            ids,
            rng,
        };
        self.update_trackers(&cx);
        for r in &mut self.recently_abstracted {
            r.counter += 1;
        }
        self.recently_abstracted.retain(|r| r.counter <= 80);
        if self.last_frame_follow_room != follow.room && !self.recently_abstracted(follow.room) {
            self.remove_not_visited_rooms(&mut cx);
            self.realize_and_track(&mut cx, follow.room, true);
            for t in &mut self.trackers {
                if t.room == follow.room {
                    t.room_changes_since_visited = 0;
                } else {
                    t.room_changes_since_visited += 1;
                }
            }
            self.neighbour_candidates.clear();
        }
        self.last_frame_follow_room = follow.room;
        if let Some(loading) = self.currently_loading {
            if cx.world.realized(loading).is_none_or(|r| r.fully_loaded()) {
                self.currently_loading = None;
            }
            return;
        }
        let performance = self.current_performance(&cx);
        if performance > PERFORMANCE_BUDGET / 4.0 {
            self.delete_distant_rooms(&mut cx);
        }
        if performance > PERFORMANCE_BUDGET {
            let candidates: Vec<i32> = self.trackers.iter().map(|t| t.room).collect();
            self.shave_down(&mut cx, performance, PERFORMANCE_BUDGET, candidates);
            return;
        }
        let ready = follow.in_room
            && cx
                .world
                .realized(follow.room)
                .is_some_and(|r| r.ready_for_ai());
        if ready {
            self.predict_next_room(&cx);
            if let Some(next) = self.probable_next
                && !cx.realized(next)
                && !self.recently_abstracted(next)
            {
                let cost = cx.estimate(next);
                if performance + cost < PERFORMANCE_BUDGET
                    || self.release_performance(&mut cx, performance, PERFORMANCE_BUDGET - cost)
                {
                    self.realize_and_track(&mut cx, next, false);
                }
            }
        }
        self.maybe_realize_neighbour(&mut cx, performance);
    }

    /// The tracker loop at the top of `Update`.
    fn update_trackers(&mut self, cx: &Ctx<'_>) {
        for t in &mut self.trackers {
            if t.room == cx.follow.room {
                t.time_since_visited = 0;
                t.has_been_visited = true;
            } else {
                t.time_since_visited += 1;
            }
            if cx.world.realized(t.room).is_some_and(|r| r.fully_loaded()) {
                t.age += 1;
            }
        }
        self.trackers.retain(|t| cx.realized(t.room));
    }

    /// The next-room guess: the exit nearest the creature (distance through the room
    /// plus straight-line distance to the exit's entrance) and the farthest one.
    fn predict_next_room(&mut self, cx: &Ctx<'_>) {
        let room = cx.follow.room;
        let connections = cx.connections(room);
        let Some(realized) = cx.world.realized(room) else {
            return;
        };
        let (mut nearest, mut nearest_d) = (None, i32::MAX);
        let (mut farthest, mut farthest_d) = (None, 0);
        for (j, _) in connections.iter().enumerate() {
            let Some(shortcut) = realized.room.shortcut_map().leading_to_node(j) else {
                continue;
            };
            let (tx, ty) = cx.follow.tile;
            let (sx, sy) = shortcut.start;
            let straight = (((tx - sx) * (tx - sx) + (ty - sy) * (ty - sy)) as f32).sqrt();
            let d = realized.exit_distance(cx.follow.tile, j) + straight as i32;
            if d < nearest_d {
                (nearest, nearest_d) = (Some(j), d);
            }
            if d > farthest_d {
                (farthest, farthest_d) = (Some(j), d);
            }
        }
        self.probable_next = nearest
            .map(|j| connections[j])
            .filter(|&c| c > -1)
            .and_then(|c| cx.existing(c));
        self.improbable_next = farthest
            .filter(|&j| Some(j) != nearest)
            .map(|j| connections[j])
            .filter(|&c| c > -1)
            .and_then(|c| cx.existing(c));
    }

    /// The random neighbour at the end of `Update`: one `Random.Range` per tick while
    /// nothing loads and the budget is under half, realizing a neighbour drawn more than
    /// 40 ticks in a row.
    fn maybe_realize_neighbour(&mut self, cx: &mut Ctx<'_>, performance: f32) {
        let connections = cx.connections(cx.follow.room);
        if self.currently_loading.is_some()
            || performance >= PERFORMANCE_BUDGET / 2.0
            || connections.is_empty()
        {
            return;
        }
        let len = i32::try_from(connections.len()).unwrap_or(i32::MAX);
        let drawn = connections[cx.rng.range_i32(0, len) as usize];
        let room = cx.existing(drawn);
        let slot = match self
            .neighbour_candidates
            .iter()
            .position(|c| c.room == room)
        {
            Some(i) => i,
            None => {
                self.neighbour_candidates
                    .push(RoomAndInt { room, counter: 0 });
                self.neighbour_candidates.len() - 1
            }
        };
        match room {
            Some(r)
                if !cx.realized(r)
                    && !self.recently_abstracted(r)
                    && performance + cx.estimate(r) < PERFORMANCE_BUDGET / 2.0 =>
            {
                self.neighbour_candidates[slot].counter += 1;
                if self.neighbour_candidates[slot].counter > 40 {
                    self.realize_and_track(cx, r, false);
                    log::info!("activated random neighbour {r}");
                }
            }
            _ => self.neighbour_candidates[slot].counter = 0,
        }
    }

    /// `CheckForAndDeleteDistantRooms`: one random tracked room, abstracted unless it
    /// or a neighbour (two steps for a cheap room) is the creature's or the probable
    /// next room.
    fn delete_distant_rooms(&mut self, cx: &mut Ctx<'_>) {
        let follow_loaded = cx
            .world
            .realized(cx.follow.room)
            .is_some_and(|r| r.fully_loaded());
        if !follow_loaded || self.trackers.is_empty() {
            return;
        }
        let len = i32::try_from(self.trackers.len()).unwrap_or(i32::MAX);
        let pick = cx.rng.range_i32(0, len) as usize;
        let room = self.trackers[pick].room;
        if room == cx.follow.room
            || Some(room) == self.probable_next
            || !self.can_abstractize(cx, pick)
        {
            return;
        }
        let follow = Some(cx.follow.room);
        for c in cx.connections(room) {
            if c <= -1 {
                continue;
            }
            let neighbour = cx.existing(c);
            if neighbour == follow || neighbour == self.probable_next {
                return;
            }
            if cx.estimate(room) >= PERFORMANCE_BUDGET / 4.0 {
                continue;
            }
            for c2 in neighbour.map(|n| cx.connections(n)).unwrap_or_default() {
                let second = cx.existing(c2);
                if second == follow || second == self.probable_next {
                    return;
                }
            }
        }
        log::info!("kill distant room {room}");
        self.kill_room(cx, room);
        self.trackers.retain(|t| t.room != room);
    }

    /// `CanAbstractizeRoom` for tracker `i`.
    fn can_abstractize(&self, cx: &Ctx<'_>, i: usize) -> bool {
        let t = &self.trackers[i];
        let min_age = if Some(t.room) == self.probable_next {
            400
        } else {
            40
        };
        t.room != cx.follow.room
            && cx.world.realized(t.room).is_some_and(|r| r.fully_loaded())
            && (!t.has_been_visited || t.time_since_visited > 200)
            && t.age > min_age
            && t.room != cx.camera_room
    }

    /// `RealizeAndTrackRoom`.
    fn realize_and_track(&mut self, cx: &mut Ctx<'_>, room: i32, entering: bool) {
        if let Some(t) = self.trackers.iter_mut().find(|t| t.room == room) {
            t.has_been_visited |= entering;
            return;
        }
        if let Err(e) = cx.world.activate_room(room, cx.ids, cx.rng) {
            log::warn!("cannot realize room {room}: {e}");
        }
        self.currently_loading = cx.realized(room).then_some(room);
        self.trackers.push(Tracker::new(room, entering));
    }

    /// `ShaveDownPerformanceTo` over `candidates` (tracked rooms).
    fn shave_down(
        &mut self,
        cx: &mut Ctx<'_>,
        mut current: f32,
        goal: f32,
        mut candidates: Vec<i32>,
    ) {
        while current > goal && !candidates.is_empty() {
            match self.put_out_a_room(cx, &mut candidates) {
                Some(room) => current -= cx.estimate(room),
                None => break,
            }
        }
    }

    /// `PutOutARoom`: abstracts the candidate with the highest deletion score.
    fn put_out_a_room(&mut self, cx: &mut Ctx<'_>, candidates: &mut Vec<i32>) -> Option<i32> {
        let follow_connections = cx.connections(cx.follow.room);
        let mut best: Option<(f32, i32)> = None;
        for &room in candidates.iter() {
            let Some(i) = self.trackers.iter().position(|t| t.room == room) else {
                continue;
            };
            if !self.can_abstractize(cx, i) {
                continue;
            }
            let mut score = self.trackers[i].deletion_factor();
            if !follow_connections.contains(&room) {
                score += 1000.0;
            }
            if Some(room) == self.probable_next {
                score -= 1000.0;
            }
            if Some(room) == self.improbable_next {
                score += 1.0;
            }
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, room));
            }
        }
        let (_, room) = best?;
        log::info!("kill room {room}");
        self.kill_room(cx, room);
        self.trackers.retain(|t| t.room != room);
        candidates.retain(|&c| c != room);
        Some(room)
    }

    /// `IfPossibleReleasePerformance`.
    fn release_performance(&mut self, cx: &mut Ctx<'_>, current: f32, goal: f32) -> bool {
        let candidates: Vec<i32> = (0..self.trackers.len())
            .filter(|&i| {
                self.can_abstractize(cx, i) && Some(self.trackers[i].room) != self.probable_next
            })
            .map(|i| self.trackers[i].room)
            .collect();
        let freed: f32 = candidates.iter().map(|&r| cx.estimate(r)).sum();
        if current - freed <= goal {
            self.shave_down(cx, current, goal, candidates);
            true
        } else {
            false
        }
    }

    /// `RemoveNotVisitedRooms` (Remix off).
    fn remove_not_visited_rooms(&mut self, cx: &mut Ctx<'_>) {
        let connections = cx.connections(cx.follow.room);
        let doomed: Vec<i32> = self
            .trackers
            .iter()
            .filter(|t| !t.has_been_visited && !connections.contains(&t.room))
            .map(|t| t.room)
            .collect();
        for room in doomed {
            self.kill_room(cx, room);
            self.trackers.retain(|t| t.room != room);
        }
    }

    /// `KillRoom`.
    fn kill_room(&mut self, cx: &mut Ctx<'_>, room: i32) {
        cx.world.abstractize(room, cx.rng);
        match self
            .recently_abstracted
            .iter_mut()
            .find(|r| r.room == Some(room))
        {
            Some(r) => r.counter = 0,
            None => self.recently_abstracted.push(RoomAndInt {
                room: Some(room),
                counter: 0,
            }),
        }
    }

    fn recently_abstracted(&self, room: i32) -> bool {
        self.recently_abstracted
            .iter()
            .any(|r| r.room == Some(room))
    }

    /// `CurrentPerformanceEstimation`.
    fn current_performance(&self, cx: &Ctx<'_>) -> f32 {
        self.trackers.iter().map(|t| cx.estimate(t.room)).sum()
    }

    /// Global indices of the tracked rooms, in tracking order.
    #[must_use]
    pub fn tracked_rooms(&self) -> Vec<i32> {
        self.trackers.iter().map(|t| t.room).collect()
    }

    /// The room guessed to be entered next.
    #[must_use]
    pub fn probable_next_room(&self) -> Option<i32> {
        self.probable_next
    }
}
