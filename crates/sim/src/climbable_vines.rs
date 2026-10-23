//! `ClimbableVinesSystem` and `IClimbableVine`: a room's list of climbable polylines and the
//! geometry the player climbs them with. `ClimbableVinesSystem.Update` does nothing, so the
//! system has no tick. Behaviour: `rainworld-re/notes/climbable-vines.md`; divergences D39.
//!
//! A vine is a polyline of points with a grab radius and a mass each. The system holds the
//! vines it queries as [`ClimbableVine`] trait objects: the owner (a coral neuron, a stem, a
//! corruption tube, a pole mimic) hands in a snapshot of its points, the player's pushes are
//! applied to the snapshot as the game applies them to the owner and are logged in
//! [`VineEvent`]s for the owner to replay on itself.
//!
//! Mirrors: ClimbableVinesSystem.cs, IClimbableVine.cs

use std::fmt::Debug;

use crate::Vec2;
use crate::chunk::BodyChunk;
use crate::float_ext::FloatExt;

/// The identity of a vine object (`vines[i] == vPos.vine`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VineId(pub u64);

/// Which class a vine is (`GetVineObject(vPos) is CoralNeuron`, ...): picks the grab sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VineKind {
    CoralNeuron,
    CoralStem,
    ClimbableCorruptionTube,
    PoleMimic,
    /// More Slugcats' placed `Vine` (`MoreSlugcats.ClimbableVine`).
    ClimbableVine,
    /// Watcher's `SkyWhale.SkyWhaleWhisker`.
    SkyWhaleWhisker,
}

impl VineKind {
    /// The game's class name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::CoralNeuron => "CoralNeuron",
            Self::CoralStem => "CoralStem",
            Self::ClimbableCorruptionTube => "ClimbableCorruptionTube",
            Self::PoleMimic => "PoleMimic",
            Self::ClimbableVine => "ClimbableVine",
            Self::SkyWhaleWhisker => "SkyWhaleWhisker",
        }
    }
}

/// What `BeingClimbedOn(Creature crit)` reads of the climber.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VineClimber {
    /// `crit.mainBodyChunk.pos`.
    pub main_chunk_pos: Vec2,
}

/// `IClimbableVine`. Indices are the game's ints; a vine has at least two points while
/// it is queried (the game's `Pos(-1)` would throw).
pub trait ClimbableVine: Debug + Send + Sync {
    fn id(&self) -> VineId;

    fn kind(&self) -> VineKind;

    fn total_positions(&self) -> i32;

    fn pos(&self, index: i32) -> Vec2;

    fn rad(&self, index: i32) -> f32;

    fn mass(&self, index: i32) -> f32;

    fn push(&mut self, index: i32, movement: Vec2);

    fn being_climbed_on(&mut self, climber: &VineClimber);

    fn currently_climbable(&self) -> bool;

    fn clone_box(&self) -> Box<dyn ClimbableVine>;
}

impl Clone for Box<dyn ClimbableVine> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl PartialEq for dyn ClimbableVine {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
            && self.kind() == other.kind()
            && self.currently_climbable() == other.currently_climbable()
            && self.total_positions() == other.total_positions()
            && (0..self.total_positions()).all(|i| {
                self.pos(i) == other.pos(i)
                    && self.rad(i) == other.rad(i)
                    && self.mass(i) == other.mass(i)
            })
    }
}

/// `ClimbableVinesSystem.VinePosition`: a place along a vine, `float_pos` the fraction (0 to
/// 1) of its length from the first point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VinePosition {
    pub vine: VineId,
    pub float_pos: f32,
}

impl VinePosition {
    #[must_use]
    pub fn new(vine: VineId, float_pos: f32) -> Self {
        Self { vine, float_pos }
    }
}

/// A call the player made on a vine, for its owner to replay in order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VineEvent {
    Push {
        vine: VineId,
        index: i32,
        movement: Vec2,
    },
    BeingClimbedOn {
        vine: VineId,
        climber: VineClimber,
    },
}

/// The segment a [`VinePosition`] lies on: its points `num` and `num2`, and how far between
/// them (`t`).
struct VineSpan {
    num: i32,
    num2: i32,
    t: f32,
}

/// `room.climbableVines`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClimbableVinesSystem {
    vines: Vec<Box<dyn ClimbableVine>>,
    events: Vec<VineEvent>,
}

impl ClimbableVinesSystem {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `vines.Add(this)`.
    pub fn register(&mut self, vine: Box<dyn ClimbableVine>) {
        self.vines.push(vine);
    }

    /// `vines.Remove`.
    pub fn unregister(&mut self, id: VineId) {
        self.vines.retain(|v| v.id() != id);
    }

    /// Replaces the snapshot of the registered vine with the same id (the owner's refresh);
    /// a vine not registered is added.
    pub fn refresh(&mut self, vine: Box<dyn ClimbableVine>) {
        match self.vines.iter_mut().find(|v| v.id() == vine.id()) {
            Some(slot) => *slot = vine,
            None => self.vines.push(vine),
        }
    }

    /// `vines.Contains`.
    #[must_use]
    pub fn contains(&self, id: VineId) -> bool {
        self.vines.iter().any(|v| v.id() == id)
    }

    #[must_use]
    pub fn vines(&self) -> &[Box<dyn ClimbableVine>] {
        &self.vines
    }

    /// `GetVineObject`; `None` when the vine is not registered.
    #[must_use]
    pub fn get_vine_object(&self, vpos: &VinePosition) -> Option<&dyn ClimbableVine> {
        self.vines
            .iter()
            .find(|v| v.id() == vpos.vine)
            .map(|v| v.as_ref())
    }

    /// The calls made on vines since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<VineEvent> {
        std::mem::take(&mut self.events)
    }

    #[must_use]
    pub fn events(&self) -> &[VineEvent] {
        &self.events
    }

    fn vine_of(&self, id: VineId) -> &dyn ClimbableVine {
        self.vines
            .iter()
            .find(|v| v.id() == id)
            .map(|v| v.as_ref())
            .expect("a vine position names a registered vine")
    }

    fn span(&self, vpos: &VinePosition) -> VineSpan {
        let vine = self.vine_of(vpos.vine);
        let num = self.prev_seg_at_float(vine, vpos.float_pos);
        let num2 = (num + 1).max(0).min(vine.total_positions() - 1);
        let t = vpos.float_pos.inverse_lerp(
            self.float_at_segment(vine, num),
            self.float_at_segment(vine, num2),
        );
        VineSpan { num, num2, t }
    }

    /// `VineOverlap`: the nearest place on any climbable vine within `rad` of `pos`.
    #[must_use]
    pub fn vine_overlap(&self, pos: Vec2, rad: f32) -> Option<VinePosition> {
        let mut best = f32::MAX;
        let mut result = None;
        for vine in &self.vines {
            if !vine.currently_climbable() {
                continue;
            }
            for j in 0..vine.total_positions() - 1 {
                if !Self::overlapping_segment(
                    vine.pos(j),
                    vine.rad(j),
                    vine.pos(j + 1),
                    vine.rad(j + 1),
                    pos,
                    rad,
                ) {
                    continue;
                }
                let b = Self::closest_point_on_segment(vine.pos(j), vine.pos(j + 1), pos);
                let distance = pos.distance(b);
                if distance < best {
                    best = distance;
                    result = Some(self.place_on_segment(vine.as_ref(), j, b));
                }
            }
        }
        result
    }

    /// The place `point` (on segment `j`) holds along `vine`.
    fn place_on_segment(&self, vine: &dyn ClimbableVine, j: i32, point: Vec2) -> VinePosition {
        let t = vine
            .pos(j)
            .distance(point)
            .inverse_lerp(0.0, vine.pos(j).distance(vine.pos(j + 1)));
        VinePosition::new(
            vine.id(),
            self.float_at_segment(vine, j)
                .lerp(self.float_at_segment(vine, j + 1), t),
        )
    }

    /// `OnVinePos`.
    #[must_use]
    pub fn on_vine_pos(&self, vpos: &VinePosition) -> Vec2 {
        let vine = self.vine_of(vpos.vine);
        let s = self.span(vpos);
        vine.pos(s.num).lerp(vine.pos(s.num2), s.t)
    }

    /// `VineRad`.
    #[must_use]
    pub fn vine_rad(&self, vpos: &VinePosition) -> f32 {
        let vine = self.vine_of(vpos.vine);
        let s = self.span(vpos);
        vine.rad(s.num).lerp(vine.rad(s.num2), s.t)
    }

    /// `VineDir`: the unit direction of the vine along its length at `vpos`.
    #[must_use]
    pub fn vine_dir(&self, vpos: &VinePosition) -> Vec2 {
        let vine = self.vine_of(vpos.vine);
        let s = self.span(vpos);
        if s.num == s.num2 {
            let n = vine.total_positions();
            return vine.pos(n - 2).direction_to(vine.pos(n - 1));
        }
        self.dir_of_segment(vine, s.num)
            .lerp(self.dir_of_segment(vine, s.num2), s.t)
            .normalized()
    }

    fn dir_of_segment(&self, vine: &dyn ClimbableVine, seg: i32) -> Vec2 {
        let n = vine.total_positions();
        if seg == n - 1 {
            return vine.pos(n - 2).direction_to(vine.pos(n - 1));
        }
        vine.pos(seg).direction_to(vine.pos(seg + 1))
    }

    /// `ClimbOnVineSpeed`: +1 toward the far end, -1 toward the start, by how well the
    /// climber's goal lines up with the vine (nothing below a dot of 0.3).
    #[must_use]
    pub fn climb_on_vine_speed(&self, vpos: &VinePosition, goal_pos: Vec2) -> f32 {
        if vpos.float_pos == 0.0 {
            return 1.0;
        }
        if vpos.float_pos == 1.0 {
            return -1.0;
        }
        let vine = self.vine_of(vpos.vine);
        let s = self.span(vpos);
        let on = vine.pos(s.num).lerp(vine.pos(s.num2), s.t);
        let f = (vine.pos(s.num) - vine.pos(s.num2))
            .normalized()
            .dot((on - goal_pos).normalized());
        f.abs().inverse_lerp(0.3, 0.5) * f.unity_sign()
    }

    /// `VineSwitch`: when the climber's goal is across the vine (the dot at most 0.5), the
    /// place on another climbable segment (or a part of this vine over 100 px away) nearest
    /// the goal, those along the goal's direction favoured.
    #[must_use]
    pub fn vine_switch(
        &self,
        vpos: &VinePosition,
        goal_pos: Vec2,
        rad: f32,
    ) -> Option<VinePosition> {
        let here = self.vine_of(vpos.vine);
        let s = self.span(vpos);
        let on = here.pos(s.num).lerp(here.pos(s.num2), s.t);
        let toward_goal = (on - goal_pos).normalized();
        if (here.pos(s.num) - here.pos(s.num2))
            .normalized()
            .dot(toward_goal)
            .abs()
            > 0.5
        {
            return None;
        }
        let mut best = f32::MAX;
        let mut result = None;
        for vine in &self.vines {
            if !vine.currently_climbable() {
                continue;
            }
            for j in 0..vine.total_positions() - 1 {
                if !Self::overlapping_segment(
                    vine.pos(j),
                    vine.rad(j),
                    vine.pos(j + 1),
                    vine.rad(j + 1),
                    on,
                    rad,
                ) {
                    continue;
                }
                let closest = Self::closest_point_on_segment(vine.pos(j), vine.pos(j + 1), on);
                let mut score = closest.distance(goal_pos);
                score *= 1.0
                    - 0.25
                        * (vine.pos(j) - vine.pos(j + 1))
                            .normalized()
                            .dot(toward_goal)
                            .abs();
                if vine.id() == vpos.vine {
                    let there = self.place_on_segment(vine.as_ref(), j, closest).float_pos
                        * self.total_length(vine.as_ref());
                    if (vpos.float_pos * self.total_length(here) - there).abs() < 100.0 {
                        score = f32::MAX;
                    }
                }
                if score < best {
                    best = score;
                    result = Some(self.place_on_segment(vine.as_ref(), j, closest));
                }
            }
        }
        result
    }

    /// `TotalLength`.
    #[must_use]
    pub fn total_length(&self, vine: &dyn ClimbableVine) -> f32 {
        let mut length = 0.0;
        for i in 0..vine.total_positions() - 1 {
            length += vine.pos(i).distance(vine.pos(i + 1));
        }
        length
    }

    /// `TotalLength(vPos.vine)`.
    #[must_use]
    pub fn total_length_of(&self, id: VineId) -> f32 {
        self.total_length(self.vine_of(id))
    }

    /// `FloatAtSegment`: the fraction of the vine's length before point `segment`.
    #[must_use]
    pub fn float_at_segment(&self, vine: &dyn ClimbableVine, segment: i32) -> f32 {
        let mut total = 0.0;
        let mut before = 0.0;
        for i in 0..vine.total_positions() - 1 {
            if i < segment {
                before += vine.pos(i).distance(vine.pos(i + 1));
            }
            total += vine.pos(i).distance(vine.pos(i + 1));
        }
        before / total
    }

    /// `PrevSegAtFloat`: the segment holding fraction `f_pos` (the last point when none
    /// does, as for NaN or a fraction past the end).
    #[must_use]
    pub fn prev_seg_at_float(&self, vine: &dyn ClimbableVine, f_pos: f32) -> i32 {
        let f_pos = f_pos * self.total_length(vine);
        let mut length = 0.0;
        for i in 0..vine.total_positions() - 1 {
            length += vine.pos(i).distance(vine.pos(i + 1));
            if length > f_pos {
                return i;
            }
        }
        vine.total_positions() - 1
    }

    fn overlapping_segment(
        a: Vec2,
        a_rad: f32,
        b: Vec2,
        b_rad: f32,
        test_pos: Vec2,
        t_rad: f32,
    ) -> bool {
        let perp = (a - b).normalized().perpendicular();
        if test_pos.distance_to_line(a + perp, a - perp) >= 0.0 {
            return test_pos.distance(a) < a_rad + t_rad;
        }
        if test_pos.distance_to_line(b + perp, b - perp) <= 0.0 {
            return test_pos.distance(b) < b_rad + t_rad;
        }
        let closest = test_pos.closest_point_on_line(a, b);
        closest.distance(test_pos)
            < a.distance(closest)
                .lerp_map(0.0, a.distance(b), a_rad, b_rad)
                + t_rad
    }

    fn closest_point_on_segment(a: Vec2, b: Vec2, test_pos: Vec2) -> Vec2 {
        let perp = (a - b).normalized().perpendicular();
        if test_pos.distance_to_line(a + perp, a - perp) >= 0.0 {
            return a;
        }
        if test_pos.distance_to_line(b + perp, b - perp) <= 0.0 {
            return b;
        }
        test_pos.closest_point_on_line(a, b)
    }

    /// `ConnectChunkToVine`: a chunk further than `con_rad` from its place on the vine is
    /// pulled in, the vine pulled back by the share of the masses.
    pub fn connect_chunk_to_vine(
        &mut self,
        chunk: &mut BodyChunk,
        vpos: &VinePosition,
        con_rad: f32,
    ) {
        let vine = self.vine_of(vpos.vine);
        let s = self.span(vpos);
        let on = vine.pos(s.num).lerp(vine.pos(s.num2), s.t);
        let share = chunk.mass / (chunk.mass + vine.mass(s.num).lerp(vine.mass(s.num2), s.t));
        let distance = chunk.pos.distance(on);
        let dir = chunk.pos.direction_to(on);
        if distance > con_rad {
            let pull = dir * ((distance - con_rad) * (1.0 - share));
            let movement = -dir * ((distance - con_rad) * share);
            chunk.pos += pull;
            chunk.vel += pull;
            self.push(vpos.vine, s.num, movement);
            self.push(vpos.vine, s.num2, movement);
        }
    }

    /// `PushAtVine`: `push` shared between the two points by the place's fraction, divided by
    /// the (at least 1) mass there.
    pub fn push_at_vine(&mut self, vpos: &VinePosition, push: Vec2) {
        let vine = self.vine_of(vpos.vine);
        let s = self.span(vpos);
        let mass = 1.0_f32.max(vine.mass(s.num).lerp(vine.mass(s.num2), s.t));
        self.push(vpos.vine, s.num, push / mass * (1.0 - s.t));
        self.push(vpos.vine, s.num2, push / mass * s.t);
    }

    /// `VineBeingClimbedOn`.
    pub fn vine_being_climbed_on(&mut self, vpos: &VinePosition, climber: VineClimber) {
        if let Some(vine) = self.vines.iter_mut().find(|v| v.id() == vpos.vine) {
            vine.being_climbed_on(&climber);
        }
        self.events.push(VineEvent::BeingClimbedOn {
            vine: vpos.vine,
            climber,
        });
    }

    /// `VineCurrentlyClimbable`.
    #[must_use]
    pub fn vine_currently_climbable(&self, vpos: &VinePosition) -> bool {
        self.vine_of(vpos.vine).currently_climbable()
    }

    fn push(&mut self, id: VineId, index: i32, movement: Vec2) {
        if let Some(vine) = self.vines.iter_mut().find(|v| v.id() == id) {
            vine.push(index, movement);
        }
        self.events.push(VineEvent::Push {
            vine: id,
            index,
            movement,
        });
    }
}

#[cfg(test)]
#[path = "climbable_vines_tests.rs"]
pub(crate) mod tests;
