//! `ClimbableVinesSystem` and `IClimbableVine`: a room's list of climbable polylines and the
//! geometry the player climbs them with. `ClimbableVinesSystem.Update` does nothing, so the
//! system has no tick. Behaviour: `rainworld-re/notes/climbable-vines.md`; divergences D39.
//!
//! The system keeps the game's list of vines, as [`VineRef`]s in registration order. A vine's
//! points, radii and masses are read, and its pushes and `BeingClimbedOn` made, on the live
//! owner through a [`VineHost`] / [`VineHostMut`] the geometry methods take, at the moment the
//! game reads or calls them. The room's host is [`RoomVines`]: the [`PlacedVines`] (vines
//! registered at run time: tests) and the coral system's neurons; each owner the port ports
//! (coral stems, corruption tubes, pole mimics) adds a [`VineRef`] variant and a branch of
//! the host.
//!
//! Mirrors: ClimbableVinesSystem.cs, IClimbableVine.cs

use rw_random::UnityRandom;

use crate::CoralNeuron;
use crate::float_ext::FloatExt;
use crate::{BodyChunk, Sounds, Vec2};

/// A reference to a vine object, as the game's `IClimbableVine` reference: world-unique,
/// compared by identity (`vines[i] == vPos.vine`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VineRef {
    /// Vine `id` registered at run time in room `room` ([`PlacedVines`]).
    Placed { room: i32, id: u64 },
    /// The `CoralNeuron` at place `order` in the update list of room `room`.
    CoralNeuron { room: i32, order: u64 },
}

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

/// Reading `IClimbableVine` on the live owners. Indices are the game's ints; a vine has at
/// least two points while it is queried (the game's `Pos(-1)` would throw).
pub trait VineHost {
    fn kind(&self, vine: VineRef) -> VineKind;

    fn total_positions(&self, vine: VineRef) -> i32;

    fn pos(&self, vine: VineRef, index: i32) -> Vec2;

    fn rad(&self, vine: VineRef, index: i32) -> f32;

    fn mass(&self, vine: VineRef, index: i32) -> f32;

    fn currently_climbable(&self, vine: VineRef) -> bool;
}

/// Calling `IClimbableVine` on the live owners.
pub trait VineHostMut: VineHost {
    fn push(&mut self, vine: VineRef, index: i32, movement: Vec2);

    /// `BeingClimbedOn(crit)`. The draws and sounds an owner makes (the pole mimic's
    /// `Player_Grab_Pole_Mimic`) are made here, in the player's turn.
    fn being_climbed_on(
        &mut self,
        vine: VineRef,
        climber: &VineClimber,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    );

    /// Whether `being_climbed_on` draws, so the player plays its queued sounds first.
    fn draws_when_climbed(&self, _vine: VineRef) -> bool {
        false
    }
}

/// A vine registered at run time in [`PlacedVines`].
pub trait ClimbableVine: std::fmt::Debug + Send + Sync {
    fn id(&self) -> u64;

    fn kind(&self) -> VineKind;

    fn total_positions(&self) -> i32;

    fn pos(&self, index: i32) -> Vec2;

    fn rad(&self, index: i32) -> f32;

    fn mass(&self, index: i32) -> f32;

    fn push(&mut self, index: i32, movement: Vec2);

    fn being_climbed_on(
        &mut self,
        climber: &VineClimber,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    );

    fn currently_climbable(&self) -> bool;

    /// Whether `being_climbed_on` draws.
    fn draws_when_climbed(&self) -> bool {
        false
    }
}

/// The vines registered at run time in a room, the host of their [`VineRef::Placed`]s.
#[derive(Debug, Default)]
pub struct PlacedVines {
    vines: Vec<Box<dyn ClimbableVine>>,
}

impl PlacedVines {
    pub fn add(&mut self, vine: Box<dyn ClimbableVine>) {
        self.vines.push(vine);
    }

    #[must_use]
    pub fn get(&self, id: u64) -> Option<&dyn ClimbableVine> {
        self.vines.iter().find(|v| v.id() == id).map(|v| v.as_ref())
    }

    fn vine(&self, vine: VineRef) -> &dyn ClimbableVine {
        let VineRef::Placed { id, .. } = vine else {
            unreachable!("not a placed vine");
        };
        self.get(id).expect("a placed vine is in its room's host")
    }

    fn vine_mut(&mut self, vine: VineRef) -> &mut dyn ClimbableVine {
        let VineRef::Placed { id, .. } = vine else {
            unreachable!("not a placed vine");
        };
        self.vines
            .iter_mut()
            .find(|v| v.id() == id)
            .expect("a placed vine is in its room's host")
            .as_mut()
    }
}

impl VineHost for PlacedVines {
    fn kind(&self, vine: VineRef) -> VineKind {
        self.vine(vine).kind()
    }

    fn total_positions(&self, vine: VineRef) -> i32 {
        self.vine(vine).total_positions()
    }

    fn pos(&self, vine: VineRef, index: i32) -> Vec2 {
        self.vine(vine).pos(index)
    }

    fn rad(&self, vine: VineRef, index: i32) -> f32 {
        self.vine(vine).rad(index)
    }

    fn mass(&self, vine: VineRef, index: i32) -> f32 {
        self.vine(vine).mass(index)
    }

    fn currently_climbable(&self, vine: VineRef) -> bool {
        self.vine(vine).currently_climbable()
    }
}

impl VineHostMut for PlacedVines {
    fn push(&mut self, vine: VineRef, index: i32, movement: Vec2) {
        self.vine_mut(vine).push(index, movement);
    }

    fn being_climbed_on(
        &mut self,
        vine: VineRef,
        climber: &VineClimber,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) {
        self.vine_mut(vine).being_climbed_on(climber, rng, sounds);
    }

    fn draws_when_climbed(&self, vine: VineRef) -> bool {
        self.vine(vine).draws_when_climbed()
    }
}

/// The room's live vine owners, read.
#[derive(Clone, Copy)]
pub struct RoomVinesView<'a> {
    pub placed: &'a PlacedVines,
    pub neurons: &'a [CoralNeuron],
}

impl RoomVinesView<'_> {
    fn neuron(&self, vine: VineRef) -> &CoralNeuron {
        let VineRef::CoralNeuron { order, .. } = vine else {
            unreachable!("not a coral neuron");
        };
        self.neurons
            .iter()
            .find(|n| n.update_order == order)
            .expect("a coral neuron is in its room's system")
    }
}

impl VineHost for RoomVinesView<'_> {
    fn kind(&self, vine: VineRef) -> VineKind {
        match vine {
            VineRef::Placed { .. } => self.placed.kind(vine),
            VineRef::CoralNeuron { .. } => VineKind::CoralNeuron,
        }
    }

    fn total_positions(&self, vine: VineRef) -> i32 {
        match vine {
            VineRef::Placed { .. } => self.placed.total_positions(vine),
            VineRef::CoralNeuron { .. } => self.neuron(vine).geometry.segments.len() as i32,
        }
    }

    fn pos(&self, vine: VineRef, index: i32) -> Vec2 {
        match vine {
            VineRef::Placed { .. } => self.placed.pos(vine, index),
            VineRef::CoralNeuron { .. } => self.neuron(vine).vine_pos(index as usize),
        }
    }

    fn rad(&self, vine: VineRef, index: i32) -> f32 {
        match vine {
            VineRef::Placed { .. } => self.placed.rad(vine, index),
            VineRef::CoralNeuron { .. } => CoralNeuron::vine_rad(index as usize),
        }
    }

    fn mass(&self, vine: VineRef, index: i32) -> f32 {
        match vine {
            VineRef::Placed { .. } => self.placed.mass(vine, index),
            VineRef::CoralNeuron { .. } => CoralNeuron::vine_mass(index as usize),
        }
    }

    fn currently_climbable(&self, vine: VineRef) -> bool {
        match vine {
            VineRef::Placed { .. } => self.placed.currently_climbable(vine),
            VineRef::CoralNeuron { .. } => true,
        }
    }
}

/// The room's live vine owners, called: the host of every [`VineRef`] the room's
/// [`ClimbableVinesSystem`] lists, built from split borrows of the room.
pub struct RoomVines<'a> {
    pub placed: &'a mut PlacedVines,
    pub neurons: &'a mut [CoralNeuron],
}

impl<'a> RoomVines<'a> {
    /// A host of run-time vines only.
    pub fn placed_only(placed: &'a mut PlacedVines) -> Self {
        Self {
            placed,
            neurons: &mut [],
        }
    }

    #[must_use]
    pub fn view(&self) -> RoomVinesView<'_> {
        RoomVinesView {
            placed: self.placed,
            neurons: self.neurons,
        }
    }
}

impl VineHost for RoomVines<'_> {
    fn kind(&self, vine: VineRef) -> VineKind {
        self.view().kind(vine)
    }

    fn total_positions(&self, vine: VineRef) -> i32 {
        self.view().total_positions(vine)
    }

    fn pos(&self, vine: VineRef, index: i32) -> Vec2 {
        self.view().pos(vine, index)
    }

    fn rad(&self, vine: VineRef, index: i32) -> f32 {
        self.view().rad(vine, index)
    }

    fn mass(&self, vine: VineRef, index: i32) -> f32 {
        self.view().mass(vine, index)
    }

    fn currently_climbable(&self, vine: VineRef) -> bool {
        self.view().currently_climbable(vine)
    }
}

impl VineHostMut for RoomVines<'_> {
    fn push(&mut self, vine: VineRef, index: i32, movement: Vec2) {
        match vine {
            VineRef::Placed { .. } => self.placed.push(vine, index, movement),
            VineRef::CoralNeuron { order, .. } => {
                if let Some(n) = self.neurons.iter_mut().find(|n| n.update_order == order) {
                    n.push(index as usize, movement);
                }
            }
        }
    }

    fn being_climbed_on(
        &mut self,
        vine: VineRef,
        climber: &VineClimber,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) {
        match vine {
            VineRef::Placed { .. } => self.placed.being_climbed_on(vine, climber, rng, sounds),
            // `CoralNeuron.BeingClimbedOn` does nothing.
            VineRef::CoralNeuron { .. } => {}
        }
    }

    fn draws_when_climbed(&self, vine: VineRef) -> bool {
        match vine {
            VineRef::Placed { .. } => self.placed.draws_when_climbed(vine),
            VineRef::CoralNeuron { .. } => false,
        }
    }
}

/// `room.climbableVines` and the room's live vine owners, as the player's update reaches
/// them (null `climbableVines` is no access).
pub struct VineAccess<'a> {
    pub system: &'a ClimbableVinesSystem,
    pub host: RoomVines<'a>,
}

/// `ClimbableVinesSystem.VinePosition`: a place along a vine, `float_pos` the fraction (0 to
/// 1) of its length from the first point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VinePosition {
    pub vine: VineRef,
    pub float_pos: f32,
}

impl VinePosition {
    #[must_use]
    pub fn new(vine: VineRef, float_pos: f32) -> Self {
        Self { vine, float_pos }
    }
}

/// The segment a [`VinePosition`] lies on: its points `num` and `num2`, and how far between
/// them (`t`).
struct VineSpan {
    num: i32,
    num2: i32,
    t: f32,
}

/// `room.climbableVines`: the game's `vines` list. Its geometry methods read and push the
/// vines on the `host` that holds their owners.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClimbableVinesSystem {
    /// Its place in the room's update list (0 for a system made without one: tests).
    pub update_order: u64,
    vines: Vec<VineRef>,
}

impl ClimbableVinesSystem {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `vines.Add(this)`.
    pub fn register(&mut self, vine: VineRef) {
        self.vines.push(vine);
    }

    /// `vines.Remove`.
    pub fn unregister(&mut self, vine: VineRef) {
        self.vines.retain(|v| *v != vine);
    }

    /// `vines.Contains`.
    #[must_use]
    pub fn contains(&self, vine: VineRef) -> bool {
        self.vines.contains(&vine)
    }

    /// `vines`, in registration order.
    #[must_use]
    pub fn vines(&self) -> &[VineRef] {
        &self.vines
    }

    fn span<H: VineHost + ?Sized>(&self, host: &H, vpos: &VinePosition) -> VineSpan {
        let num = self.prev_seg_at_float(host, vpos.vine, vpos.float_pos);
        let num2 = (num + 1).max(0).min(host.total_positions(vpos.vine) - 1);
        let t = vpos.float_pos.inverse_lerp(
            self.float_at_segment(host, vpos.vine, num),
            self.float_at_segment(host, vpos.vine, num2),
        );
        VineSpan { num, num2, t }
    }

    /// `VineOverlap`: the nearest place on any climbable vine within `rad` of `pos`.
    #[must_use]
    pub fn vine_overlap<H: VineHost + ?Sized>(
        &self,
        host: &H,
        pos: Vec2,
        rad: f32,
    ) -> Option<VinePosition> {
        let mut best = f32::MAX;
        let mut result = None;
        for &vine in &self.vines {
            if !host.currently_climbable(vine) {
                continue;
            }
            for j in 0..host.total_positions(vine) - 1 {
                if !Self::overlapping_segment(
                    host.pos(vine, j),
                    host.rad(vine, j),
                    host.pos(vine, j + 1),
                    host.rad(vine, j + 1),
                    pos,
                    rad,
                ) {
                    continue;
                }
                let b =
                    Self::closest_point_on_segment(host.pos(vine, j), host.pos(vine, j + 1), pos);
                let distance = pos.distance(b);
                if distance < best {
                    best = distance;
                    result = Some(self.place_on_segment(host, vine, j, b));
                }
            }
        }
        result
    }

    /// The place `point` (on segment `j`) holds along `vine`.
    fn place_on_segment<H: VineHost + ?Sized>(
        &self,
        host: &H,
        vine: VineRef,
        j: i32,
        point: Vec2,
    ) -> VinePosition {
        let t = host
            .pos(vine, j)
            .distance(point)
            .inverse_lerp(0.0, host.pos(vine, j).distance(host.pos(vine, j + 1)));
        VinePosition::new(
            vine,
            self.float_at_segment(host, vine, j)
                .lerp(self.float_at_segment(host, vine, j + 1), t),
        )
    }

    /// `OnVinePos`.
    #[must_use]
    pub fn on_vine_pos<H: VineHost + ?Sized>(&self, host: &H, vpos: &VinePosition) -> Vec2 {
        let s = self.span(host, vpos);
        host.pos(vpos.vine, s.num)
            .lerp(host.pos(vpos.vine, s.num2), s.t)
    }

    /// `VineRad`.
    #[must_use]
    pub fn vine_rad<H: VineHost + ?Sized>(&self, host: &H, vpos: &VinePosition) -> f32 {
        let s = self.span(host, vpos);
        host.rad(vpos.vine, s.num)
            .lerp(host.rad(vpos.vine, s.num2), s.t)
    }

    /// `VineDir`: the unit direction of the vine along its length at `vpos`.
    #[must_use]
    pub fn vine_dir<H: VineHost + ?Sized>(&self, host: &H, vpos: &VinePosition) -> Vec2 {
        let s = self.span(host, vpos);
        if s.num == s.num2 {
            let n = host.total_positions(vpos.vine);
            return host
                .pos(vpos.vine, n - 2)
                .direction_to(host.pos(vpos.vine, n - 1));
        }
        self.dir_of_segment(host, vpos.vine, s.num)
            .lerp(self.dir_of_segment(host, vpos.vine, s.num2), s.t)
            .normalized()
    }

    fn dir_of_segment<H: VineHost + ?Sized>(&self, host: &H, vine: VineRef, seg: i32) -> Vec2 {
        let n = host.total_positions(vine);
        if seg == n - 1 {
            return host.pos(vine, n - 2).direction_to(host.pos(vine, n - 1));
        }
        host.pos(vine, seg).direction_to(host.pos(vine, seg + 1))
    }

    /// `ClimbOnVineSpeed`: +1 toward the far end, -1 toward the start, by how well the
    /// climber's goal lines up with the vine (nothing below a dot of 0.3).
    #[must_use]
    pub fn climb_on_vine_speed<H: VineHost + ?Sized>(
        &self,
        host: &H,
        vpos: &VinePosition,
        goal_pos: Vec2,
    ) -> f32 {
        if vpos.float_pos == 0.0 {
            return 1.0;
        }
        if vpos.float_pos == 1.0 {
            return -1.0;
        }
        let s = self.span(host, vpos);
        let (a, b) = (host.pos(vpos.vine, s.num), host.pos(vpos.vine, s.num2));
        let on = a.lerp(b, s.t);
        let f = (a - b).normalized().dot((on - goal_pos).normalized());
        f.abs().inverse_lerp(0.3, 0.5) * f.unity_sign()
    }

    /// `VineSwitch`: when the climber's goal is across the vine (the dot at most 0.5), the
    /// place on another climbable segment (or a part of this vine over 100 px away) nearest
    /// the goal, those along the goal's direction favoured.
    #[must_use]
    pub fn vine_switch<H: VineHost + ?Sized>(
        &self,
        host: &H,
        vpos: &VinePosition,
        goal_pos: Vec2,
        rad: f32,
    ) -> Option<VinePosition> {
        let s = self.span(host, vpos);
        let (a, b) = (host.pos(vpos.vine, s.num), host.pos(vpos.vine, s.num2));
        let on = a.lerp(b, s.t);
        let toward_goal = (on - goal_pos).normalized();
        if (a - b).normalized().dot(toward_goal).abs() > 0.5 {
            return None;
        }
        let mut best = f32::MAX;
        let mut result = None;
        for &vine in &self.vines {
            if !host.currently_climbable(vine) {
                continue;
            }
            for j in 0..host.total_positions(vine) - 1 {
                let (pj, pk) = (host.pos(vine, j), host.pos(vine, j + 1));
                if !Self::overlapping_segment(
                    pj,
                    host.rad(vine, j),
                    pk,
                    host.rad(vine, j + 1),
                    on,
                    rad,
                ) {
                    continue;
                }
                let closest = Self::closest_point_on_segment(pj, pk, on);
                let mut score = closest.distance(goal_pos);
                score *= 1.0 - 0.25 * (pj - pk).normalized().dot(toward_goal).abs();
                if vine == vpos.vine {
                    let there = self.place_on_segment(host, vine, j, closest).float_pos
                        * self.total_length(host, vine);
                    if (vpos.float_pos * self.total_length(host, vpos.vine) - there).abs() < 100.0 {
                        score = f32::MAX;
                    }
                }
                if score < best {
                    best = score;
                    result = Some(self.place_on_segment(host, vine, j, closest));
                }
            }
        }
        result
    }

    /// `TotalLength`.
    #[must_use]
    pub fn total_length<H: VineHost + ?Sized>(&self, host: &H, vine: VineRef) -> f32 {
        let mut length = 0.0;
        for i in 0..host.total_positions(vine) - 1 {
            length += host.pos(vine, i).distance(host.pos(vine, i + 1));
        }
        length
    }

    /// `FloatAtSegment`: the fraction of the vine's length before point `segment`.
    #[must_use]
    pub fn float_at_segment<H: VineHost + ?Sized>(
        &self,
        host: &H,
        vine: VineRef,
        segment: i32,
    ) -> f32 {
        let mut total = 0.0;
        let mut before = 0.0;
        for i in 0..host.total_positions(vine) - 1 {
            if i < segment {
                before += host.pos(vine, i).distance(host.pos(vine, i + 1));
            }
            total += host.pos(vine, i).distance(host.pos(vine, i + 1));
        }
        before / total
    }

    /// `PrevSegAtFloat`: the segment holding fraction `f_pos` (the last point when none
    /// does, as for NaN or a fraction past the end).
    #[must_use]
    pub fn prev_seg_at_float<H: VineHost + ?Sized>(
        &self,
        host: &H,
        vine: VineRef,
        f_pos: f32,
    ) -> i32 {
        let f_pos = f_pos * self.total_length(host, vine);
        let mut length = 0.0;
        for i in 0..host.total_positions(vine) - 1 {
            length += host.pos(vine, i).distance(host.pos(vine, i + 1));
            if length > f_pos {
                return i;
            }
        }
        host.total_positions(vine) - 1
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
    pub fn connect_chunk_to_vine<H: VineHostMut + ?Sized>(
        &self,
        host: &mut H,
        chunk: &mut BodyChunk,
        vpos: &VinePosition,
        con_rad: f32,
    ) {
        let s = self.span(&*host, vpos);
        let (a, b) = (host.pos(vpos.vine, s.num), host.pos(vpos.vine, s.num2));
        let on = a.lerp(b, s.t);
        let vine_mass = host
            .mass(vpos.vine, s.num)
            .lerp(host.mass(vpos.vine, s.num2), s.t);
        let share = chunk.mass / (chunk.mass + vine_mass);
        let distance = chunk.pos.distance(on);
        let dir = chunk.pos.direction_to(on);
        if distance > con_rad {
            let pull = dir * ((distance - con_rad) * (1.0 - share));
            let movement = -dir * ((distance - con_rad) * share);
            chunk.pos += pull;
            chunk.vel += pull;
            host.push(vpos.vine, s.num, movement);
            host.push(vpos.vine, s.num2, movement);
        }
    }

    /// `PushAtVine`: `push` shared between the two points by the place's fraction, divided by
    /// the (at least 1) mass there.
    pub fn push_at_vine<H: VineHostMut + ?Sized>(
        &self,
        host: &mut H,
        vpos: &VinePosition,
        push: Vec2,
    ) {
        let s = self.span(&*host, vpos);
        let mass = 1.0_f32.max(
            host.mass(vpos.vine, s.num)
                .lerp(host.mass(vpos.vine, s.num2), s.t),
        );
        host.push(vpos.vine, s.num, push / mass * (1.0 - s.t));
        host.push(vpos.vine, s.num2, push / mass * s.t);
    }

    /// `VineBeingClimbedOn`.
    pub fn vine_being_climbed_on<H: VineHostMut + ?Sized>(
        &self,
        host: &mut H,
        vpos: &VinePosition,
        climber: &VineClimber,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) {
        host.being_climbed_on(vpos.vine, climber, rng, sounds);
    }

    /// `VineCurrentlyClimbable`.
    #[must_use]
    pub fn vine_currently_climbable<H: VineHost + ?Sized>(
        &self,
        host: &H,
        vpos: &VinePosition,
    ) -> bool {
        host.currently_climbable(vpos.vine)
    }
}

#[cfg(test)]
#[path = "climbable_vines_tests.rs"]
pub(crate) mod tests;
