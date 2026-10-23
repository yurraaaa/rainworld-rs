//! `CoralNeuron`: a chain of segments strung between two points (a placed object's
//! ends, snapped to the wall behind them, or a coral stem's segment), climbable by the
//! player, with two mycelia a segment pair, bumps that ping now and then and, in a
//! superstructure projector room, a `ProjectedCircle` at a free end. The constructor is
//! seeded by `(int)length` (the shared stream untouched); the sprites are
//! `rw-graphics`' `coral_neuron_draw.rs`. Behaviour:
//! `rainworld-re/notes/superstructure-coral.md`.
//!
//! Mirrors: CoralBrain/CoralNeuron.cs (constructor, Update, ConnectToWalls, Connect, SegmentOfMycelium, MycLengthContour, IClimbableVine, IOwnMycelia, IOwnProjectedCircles)

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::mycelium::{MyceliumEnv, MyceliumId, MyceliumOwner, MyceliumPool, OwnerKey, SystemId};
use crate::projected_circle::{CircleOwner, ProjectedCircle};
use crate::room_ext::RoomExt;
use crate::world::AiMap;
use crate::{FloatExt, Vec2};

/// `Custom.fourDirections`.
const FOUR_DIRECTIONS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

/// `Rad(int)`, `Mass(int)` of the climbable vine.
const VINE_RAD: f32 = 2.0;
const VINE_MASS: f32 = 0.5;

/// What the strands ask of the neuron (`IOwnMycelia`): the segments, the roots, and how
/// many strand pairs hang from them.
#[derive(Debug, Clone, PartialEq)]
pub struct NeuronGeometry {
    /// Per segment: position, last position, velocity.
    pub segments: Vec<[Vec2; 3]>,
    pub root_dir_a: Option<Vec2>,
    pub root_dir_b: Option<Vec2>,
    /// `mycelia.GetLength(0)`: strand pairs.
    pub pairs: usize,
}

impl NeuronGeometry {
    /// `SegmentOfMycelium(myc)`: the segment strand `myc` hangs from.
    #[must_use]
    pub fn segment_of_mycelium(&self, myc: usize) -> usize {
        let (len, pairs) = (self.segments.len(), self.pairs);
        if len - 2 <= pairs {
            return myc / 2 + 1;
        }
        let num = myc / 2;
        match (self.root_dir_a.is_some(), self.root_dir_b.is_some()) {
            (true, true) => len / 2 - pairs / 2 + num,
            (true, false) => len - 1 - pairs + num,
            (false, true) => num + 1,
            (false, false) => {
                if num < pairs / 2 {
                    num + 1
                } else {
                    len - 1 - pairs + num
                }
            }
        }
    }

    /// `MycLengthContour(i)`: 0 to 1 along the pairs, longest where the strands are
    /// farthest from the ends that are rooted.
    #[must_use]
    pub fn myc_length_contour(&self, i: usize) -> f32 {
        let (i, n) = (i as f32, self.pairs as f32);
        let half = (self.pairs / 2) as f32;
        match (self.root_dir_a.is_some(), self.root_dir_b.is_some()) {
            (true, true) => i
                .inverse_lerp(0.0, 4.0)
                .min(i.inverse_lerp(n - 1.0, n - 5.0)),
            (true, false) => i.inverse_lerp(0.0, half),
            (false, true) => i.inverse_lerp(n - 1.0, half),
            (false, false) => i
                .inverse_lerp(10.0, 0.0)
                .max(i.inverse_lerp(n - 11.0, n - 1.0)),
        }
    }

    /// `ConnectionPos(index, 1)`.
    #[must_use]
    pub fn connection_pos_at(&self, index: usize, time_stacker: f32) -> Vec2 {
        let s = &self.segments[self.segment_of_mycelium(index)];
        s[1].lerp(s[0], time_stacker.clamp(0.0, 1.0))
    }

    /// `CircleCenter(index, timeStacker)`: the first segment for circle 0, else the last.
    #[must_use]
    pub fn circle_center(&self, index: usize, time_stacker: f32) -> Vec2 {
        let s = if index == 0 {
            &self.segments[0]
        } else {
            &self.segments[self.segments.len() - 1]
        };
        s[1].lerp(s[0], time_stacker.clamp(0.0, 1.0))
    }
}

impl MyceliumOwner for NeuronGeometry {
    fn connection_pos(&self, index: usize) -> Vec2 {
        self.connection_pos_at(index, 1.0)
    }

    fn reset_dir(&self, index: usize, _rng: &mut UnityRandom) -> Vec2 {
        let s = self.segment_of_mycelium(index);
        let dir = self.segments[s][0].direction_to(self.segments[s + 1][0]);
        dir.perpendicular() * if index.is_multiple_of(2) { -1.0 } else { 1.0 }
    }
}

/// What the constructor reads of the room and makes in it besides the neuron.
pub struct NeuronCx<'a> {
    pub room: &'a Room,
    pub system: SystemId,
    pub pool: &'a mut MyceliumPool,
    /// The room's `SuperStructureProjector` effect amount is above 0.
    pub projector: bool,
    /// The `ProjectedCircle`s already in the room's update list, in its order.
    pub circles: Vec<u64>,
    /// The room's update-list counter (`AddObject`).
    pub seq: &'a mut u64,
}

impl NeuronCx<'_> {
    fn next_seq(&mut self) -> u64 {
        *self.seq += 1;
        *self.seq
    }
}

/// A neuron built by the constructor, with the circles it added to the room before it was
/// added itself.
pub struct NewNeuron {
    pub neuron: CoralNeuron,
    pub circles: Vec<ProjectedCircle>,
}

/// The `CoralNeuron` in a room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct CoralNeuron {
    /// Its place in the room's update list.
    pub update_order: u64,
    pub system: SystemId,
    pub owner: OwnerKey,
    pub geometry: NeuronGeometry,
    pub con_rad: f32,
    pub bumps: Vec<Vec2>,
    /// `bumpPings[i, 0]` (now) and `[i, 1]` (last tick).
    pub bump_pings: Vec<[f32; 2]>,
    pub pos_a: Option<Vec2>,
    pub pos_b: Option<Vec2>,
    /// `mycelia[l, m]` at `l * 2 + m`.
    pub mycelia: Vec<MyceliumId>,
    /// `slatedForDeletetion`.
    pub slated: bool,
}

impl CoralNeuron {
    /// The constructor (`length`, the ends `pos_a`, `pos_b`), then the caller's
    /// `AddObject`: seeded by `(int)length`, the shared stream restored. `None` where the
    /// game's `new Mycelium[length - 2, 2]` would throw (fewer than two segments).
    pub fn build(
        length: f32,
        pos_a: Option<Vec2>,
        pos_b: Option<Vec2>,
        cx: &mut NeuronCx<'_>,
        rng: &mut UnityRandom,
    ) -> Option<NewNeuron> {
        let count = (length / 20.0).clamp(1.0, 200.0).to_int_cs() as usize;
        if count < 2 {
            return None;
        }
        let saved = rng.state();
        rng.init_state(length.to_int_cs());
        let (pos_a, root_dir_a) = Self::snap_to_wall(cx.room, pos_a);
        let (pos_b, root_dir_b) = Self::snap_to_wall(cx.room, pos_b);
        let mut segments = vec![[Vec2::ZERO; 3]; count];
        for (k, s) in segments.iter_mut().enumerate() {
            let t = k as f32 / (count - 1) as f32;
            if let (Some(a), Some(b)) = (pos_a, pos_b) {
                let jitter = Vec2::random_unit(rng) * rng.value();
                s[0] = a.lerp(b, t.clamp(0.0, 1.0)) + jitter;
            } else if let Some(b) = pos_b {
                s[0] = b;
            }
            s[1] = s[0];
            s[2] = Vec2::random_unit(rng) * rng.value();
        }
        let pairs = (count - 2).min(20);
        let geometry = NeuronGeometry {
            segments,
            root_dir_a,
            root_dir_b,
            pairs,
        };
        let owner = cx.pool.new_owner();
        let mut mycelia = Vec::with_capacity(pairs * 2);
        for l in 0..pairs {
            for m in 0..2 {
                let random = 30.0f32.lerp(300.0, rng.value());
                let contour = 30.0f32.lerp(300.0, geometry.myc_length_contour(l));
                let index = l * 2 + m;
                let init = geometry.segments[geometry.segment_of_mycelium(l * 2 + 1)][0];
                let id = cx.pool.add(
                    Some(cx.system),
                    (owner, &geometry),
                    index,
                    random.min(contour),
                    init,
                    rng,
                );
                if let Some(strand) = cx.pool.get_mut(id) {
                    strand.use_static_culling = false;
                }
                mycelia.push(id);
            }
        }
        let con_rad = length / count as f32 * 1.5;
        let bump_count = (count as f32 * 1.2).to_int_cs() as usize;
        let mut bumps = Vec::with_capacity(bump_count);
        for _ in 0..bump_count {
            let x = (-1.0f32).lerp(1.0, rng.value());
            bumps.push(Vec2::new(x, rng.value()));
        }
        let mut circles: Vec<ProjectedCircle> = Vec::new();
        if cx.projector {
            let size = length.inverse_lerp(40.0, 700.0);
            if root_dir_a.is_none() && (root_dir_b.is_some() || length > 100.0) {
                Self::add_circle(cx, &mut circles, 0, size, rng);
            }
            if root_dir_b.is_none() {
                Self::add_circle(cx, &mut circles, 1, size, rng);
            }
        }
        rng.set_state(saved);
        let update_order = cx.next_seq();
        for c in &mut circles {
            c.owner = CircleOwner::Neuron {
                order: update_order,
            };
        }
        Some(NewNeuron {
            neuron: Self {
                update_order,
                system: cx.system,
                owner,
                geometry,
                con_rad,
                bump_pings: vec![[0.0; 2]; bumps.len()],
                bumps,
                pos_a,
                pos_b,
                mycelia,
                slated: false,
            },
            circles,
        })
    }

    /// `room.AddObject(new ProjectedCircle(room, this, index, size))`. Its owner is set
    /// once the neuron has its place in the update list (the circle never reads it before).
    fn add_circle(
        cx: &mut NeuronCx<'_>,
        circles: &mut Vec<ProjectedCircle>,
        index: usize,
        size: f32,
        rng: &mut UnityRandom,
    ) {
        let order = cx.next_seq();
        let circle = ProjectedCircle::new(
            order,
            CircleOwner::Neuron { order: 0 },
            index,
            size,
            cx.room,
            &cx.circles,
            rng,
        );
        cx.circles.push(order);
        circles.push(circle);
    }

    /// The constructor's wall snap of one end: each direction with a Solid neighbour tile
    /// roots the end on it (the last one wins).
    fn snap_to_wall(room: &Room, end: Option<Vec2>) -> (Option<Vec2>, Option<Vec2>) {
        let Some(mut pos) = end else {
            return (None, None);
        };
        let tile = pos.tile();
        let mut root_dir = None;
        for (dx, dy) in FOUR_DIRECTIONS {
            if room.is_solid_tile(tile.0 + dx, tile.1 + dy) {
                let dir = Vec2::new(-dx as f32, -dy as f32);
                let middle = Vec2::middle_of_tile(tile);
                pos = if dx == 0 {
                    Vec2::new(pos.x, middle.y - dir.y * 20.0)
                } else {
                    Vec2::new(middle.x - dir.x * 20.0, pos.y)
                };
                root_dir = Some(dir);
            }
        }
        (Some(pos), root_dir)
    }

    /// `Update`: nothing while the system is frozen; else the segments drift and are
    /// pulled to `conRad` apart (three passes with the wall roots held between them), the
    /// strands update and are bent away from the segment, and the bumps ping.
    pub fn update(
        &mut self,
        wind: Vec2,
        pool: &mut MyceliumPool,
        env: &MyceliumEnv<'_>,
        rng: &mut UnityRandom,
    ) {
        if env.system.is_some_and(|s| s.frozen) {
            return;
        }
        let Some(map) = env.ai_map else {
            return;
        };
        let n = self.geometry.segments.len();
        for i in 2..n {
            let dir = self.geometry.segments[i - 2][0].direction_to(self.geometry.segments[i][0]);
            self.geometry.segments[i - 2][2] -= dir * 0.15;
            self.geometry.segments[i][2] += dir * 0.15;
        }
        for j in 0..n {
            let num = j as f32 / (n - 1) as f32;
            let s = &mut self.geometry.segments[j];
            s[1] = s[0];
            s[0] += s[2];
            s[2] *= 0.999;
            let at = s[0];
            let proximity = map.terrain_proximity(at.tile().0, at.tile().1);
            if proximity < 4 {
                let push = Self::wall_push(env.room, map, at.tile());
                self.geometry.segments[j][2] +=
                    push.normalized() * (proximity as f32).lerp_map(0.0, 3.0, 2.0, 0.2);
            }
            self.geometry.segments[j][2] += wind * 0.005;
            self.pull_toward_ends(j, num);
        }
        self.connect_to_walls();
        for i in (1..n).rev() {
            self.connect(i, i - 1);
        }
        self.connect_to_walls();
        for i in 1..n {
            self.connect(i, i - 1);
        }
        self.connect_to_walls();
        self.update_mycelia(pool, env, rng);
        for ping in &mut self.bump_pings {
            ping[1] = ping[0];
            ping[0] = (ping[0] - 0.1).max(0.0);
            if ping[0] == 0.0 && rng.value() < 1.0 / 60.0 {
                ping[0] = 1.0;
            }
        }
    }

    /// The push off the walls around `tile`: each open, not narrow neighbour weighted by
    /// the terrain proximity around it.
    fn wall_push(room: &Room, map: &AiMap, tile: (i32, i32)) -> Vec2 {
        let mut push = Vec2::ZERO;
        for (kx, ky) in FOUR_DIRECTIONS {
            let side = (tile.0 + kx, tile.1 + ky);
            if !room.is_solid_tile(side.0, side.1) && !map.narrow_space(side.0, side.1) {
                let mut weight = 0.0;
                for (lx, ly) in FOUR_DIRECTIONS {
                    weight += map.terrain_proximity(side.0 + lx, side.1 + ly) as f32;
                }
                push += Vec2::new(kx as f32, ky as f32) * weight;
            }
        }
        push
    }

    /// The anchoring of segment `j` (`num` along the chain): its half's root direction,
    /// else a pull toward the free end's place.
    fn pull_toward_ends(&mut self, j: usize, num: f32) {
        let s = &mut self.geometry.segments[j];
        if num < 0.5 {
            if let Some(dir) = self.geometry.root_dir_a {
                s[2] += dir * (num.inverse_lerp(0.25, 0.0) * 0.5);
            } else if let Some(a) = self.pos_a {
                s[2] += (a - s[0]).clamp_magnitude(40.0) / 420.0 * num.inverse_lerp(0.25, 0.0);
            }
        } else if num > 0.5 {
            if let Some(dir) = self.geometry.root_dir_b {
                s[2] += dir * (num.inverse_lerp(0.75, 1.0) * 0.5);
            } else if let Some(b) = self.pos_b {
                s[2] += (b - s[0]).clamp_magnitude(40.0) / 420.0 * num.inverse_lerp(0.75, 1.0);
            }
        }
    }

    fn update_mycelia(
        &mut self,
        pool: &mut MyceliumPool,
        env: &MyceliumEnv<'_>,
        rng: &mut UnityRandom,
    ) {
        for pair in 0..self.geometry.pairs {
            let seg = self.geometry.segment_of_mycelium(pair * 2);
            let dir =
                self.geometry.segments[seg][0].direction_to(self.geometry.segments[seg + 1][0]);
            let perpendicular = dir.perpendicular();
            for side in 0..2 {
                let id = self.mycelia[pair * 2 + side];
                pool.update(id, &self.geometry, env, rng);
                let sign = if side == 0 { -1.0 } else { 1.0 };
                if let Some(m) = pool.get_mut(id) {
                    m.points[1][2] += perpendicular * sign;
                    if m.points.len() > 2 {
                        m.points[2][2] += perpendicular * (sign * 0.5);
                    }
                }
            }
        }
    }

    /// `ConnectToWalls`: a rooted end sits 10 px inside its wall, still.
    fn connect_to_walls(&mut self) {
        let last = self.geometry.segments.len() - 1;
        if let (Some(dir), Some(a)) = (self.geometry.root_dir_a, self.pos_a) {
            self.geometry.segments[0][0] = a - dir * 10.0;
            self.geometry.segments[0][2] *= 0.0;
        }
        if let (Some(dir), Some(b)) = (self.geometry.root_dir_b, self.pos_b) {
            self.geometry.segments[last][0] = b - dir * 10.0;
            self.geometry.segments[last][2] *= 0.0;
        }
    }

    /// `Connect(A, B)`: segments closer than `conRad` are pushed apart, in proportion to
    /// how close.
    fn connect(&mut self, a: usize, b: usize) {
        let s = &mut self.geometry.segments;
        let normalized = (s[a][0] - s[b][0]).normalized();
        let distance = s[a][0].distance(s[b][0]);
        let near = distance.inverse_lerp(0.0, self.con_rad);
        let push = normalized * ((self.con_rad - distance) * 0.5 * near);
        s[a][0] += push;
        s[a][2] += push;
        s[b][0] -= push;
        s[b][2] -= push;
    }

    /// `Push(index, movement)`.
    pub fn push(&mut self, index: usize, movement: Vec2) {
        self.geometry.segments[index][0] += movement;
        self.geometry.segments[index][2] += movement;
    }

    /// `Pos(index)`.
    #[must_use]
    pub fn vine_pos(&self, index: usize) -> Vec2 {
        self.geometry.segments[index][0]
    }

    /// `Rad(index)`.
    #[must_use]
    pub fn vine_rad(_index: usize) -> f32 {
        VINE_RAD
    }

    /// `Mass(index)`.
    #[must_use]
    pub fn vine_mass(_index: usize) -> f32 {
        VINE_MASS
    }

    /// `CanHostCircle`.
    #[must_use]
    pub fn can_host_circle(&self) -> bool {
        !self.slated
    }

    /// A coral stem's neuron starts with every segment on one point (`StemSegment`'s
    /// constructor sets `segments[j, 0]` and `[j, 1]`).
    pub fn set_all_points(&mut self, at: Vec2) {
        for s in &mut self.geometry.segments {
            s[0] = at;
            s[1] = at;
        }
    }
}

#[cfg(test)]
#[path = "coral_neuron_tests.rs"]
mod tests;
