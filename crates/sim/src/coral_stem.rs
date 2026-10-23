//! `CoralStem` and its `StemSegment`s: a chain of two-chunk physical objects grown from a
//! root along the tiles farthest from the walls, climbable by the player (point `i` is
//! chunk `i % 2` of segment `i / 2`), each with strands, now and then a `CoralNeuron`, and,
//! in a superstructure projector room, a `ProjectedCircle` at the free end. The
//! constructor is seeded by `(int)rootPos.x + (int)rootPos.y` (the shared stream
//! untouched). The segments are real physical objects of collision layer 0 and their
//! abstract objects are in no room's entity list (D41). The sprites are `rw-graphics`'
//! `coral_stem_draw.rs`. Behaviour: `rainworld-re/notes/superstructure-coral.md`.
//!
//! Mirrors: CoralBrain/CoralStem.cs, CoralBrain/StemSegment.cs (constructor, Update), PhysicalObject.cs (Update), IClimbableVine.cs, IOwnProjectedCircles.cs

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::coral_neuron::{CoralNeuron, NeuronCx};
use crate::creature::{EntityId, IdIssuer};
use crate::mycelium::{MyceliumEnv, MyceliumId, MyceliumOwner, MyceliumPool, OwnerKey};
use crate::object::{BodyChunkConnection, ConnectionKind};
use crate::projected_circle::{CircleOwner, ProjectedCircle};
use crate::room_ext::RoomExt;
use crate::world::AiMap;
use crate::{
    BodyChunk, FloatExt, ImpactCall, ImpactCalls, PhysicalObject, PhysicsParams, Vec2, Water,
};

/// `Custom.fourDirections`.
const FOUR_DIRECTIONS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

/// A segment of a stem of room `room`'s coral system: the stem's place in the system's
/// list and the segment's in the stem's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SegmentRef {
    pub room: i32,
    pub stem: u32,
    pub segment: u32,
}

/// What a segment's strands ask of it (`IOwnMycelia`): they hang from chunk 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentHang {
    pub last_pos: Vec2,
    pub pos: Vec2,
}

impl MyceliumOwner for SegmentHang {
    /// `ConnectionPos(index, 1)`: `Vector2.Lerp(lastPos, pos, 1)`.
    fn connection_pos(&self, _index: usize) -> Vec2 {
        self.last_pos.lerp(self.pos, 1.0)
    }

    fn reset_dir(&self, _index: usize, _rng: &mut UnityRandom) -> Vec2 {
        Vec2::ZERO
    }
}

/// The `StemSegment` in a room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct StemSegment {
    /// Its place in the room's update list (and in `physicalObjects[0]`).
    pub update_order: u64,
    /// Its `abstractPhysicalObject`'s ID (`GetNewID()`).
    pub id: EntityId,
    pub body: PhysicalObject,
    pub size: f32,
    pub con_rad: f32,
    /// The mesh's segment count.
    pub mesh_segs: usize,
    pub dots: usize,
    pub first: bool,
    pub direction: Vec2,
    pub last_direction: Vec2,
    /// The place in the update list of the `CoralNeuron` it made.
    pub neuron: Option<u64>,
    pub owner: OwnerKey,
    pub mycelia: Vec<MyceliumId>,
}

/// The `CoralStem` of a room's system.
#[derive(Debug, Clone, PartialEq)]
pub struct CoralStem {
    pub system: crate::SystemId,
    pub root_pos: Vec2,
    pub root_direction: Vec2,
    pub root_tile: (i32, i32),
    pub segments: Vec<StemSegment>,
    pub size: f32,
    pub with_neurons: bool,
}

/// What the constructor reads of the room and makes in it besides the stem.
pub struct StemCx<'a> {
    pub neuron: NeuronCx<'a>,
    pub ai_map: &'a AiMap,
    pub ids: &'a mut IdIssuer,
    /// The stem's place in the system's list (its circle's owner).
    pub index: u32,
}

/// A stem built by the constructor, with what it added to the room in the order it added
/// it: the climbable vines system (when the room had none), the neurons its segments made
/// (each with its circles) and its own circle.
pub struct NewStem {
    pub vine_system_order: Option<u64>,
    pub stem: CoralStem,
    pub neurons: Vec<CoralNeuron>,
    pub circles: Vec<ProjectedCircle>,
}

/// The ticks' room: what a segment's turn reads of it.
pub struct StemTick<'a> {
    pub room: &'a Room,
    pub ai_map: &'a AiMap,
    pub water: Option<&'a Water>,
    pub wind: Vec2,
    pub room_gravity: f32,
}

impl StemSegment {
    fn params() -> PhysicsParams {
        PhysicsParams {
            g: 0.9,
            room_gravity: 1.0,
            air_friction: 0.999,
            bounce: 0.5,
            surface_friction: 0.5,
            impact_threshold: 1.0,
            buoyancy: 0.95,
            water_friction: 0.96,
            water_retardation_immunity: 0.0,
        }
    }

    /// The constructor (`size` 0 to 1, the chunks at `pos_a` and `pos_b`), then the
    /// caller's `AddObject`: a neuron the segment makes is added first.
    fn build(
        size: f32,
        pos_a: Vec2,
        pos_b: Vec2,
        with_neurons: bool,
        cx: &mut StemCx<'_>,
        rng: &mut UnityRandom,
    ) -> (Self, Option<(CoralNeuron, Vec<ProjectedCircle>)>) {
        let id = cx.ids.new_id(-1);
        let rad = 4.0f32.lerp(9.0, size);
        let mass = 0.015f32.lerp(2.8, size);
        let mut chunks = vec![
            BodyChunk::new(Vec2::ZERO, rad, mass),
            BodyChunk::new(Vec2::ZERO, rad, mass),
        ];
        let con_rad = 6.0f32.lerp(12.0, size);
        chunks[0].hard_set_position(pos_a);
        chunks[1].hard_set_position(pos_b);
        chunks[0].vel = Vec2::random_unit(rng);
        let distance = 6.0f32.lerp(68.0, size);
        let mut body = PhysicalObject::new(chunks, Self::params());
        body.connections.push(BodyChunkConnection::new(
            0,
            1,
            distance,
            ConnectionKind::Normal,
            1.0,
            0.5,
        ));
        let mesh_segs = distance.lerp_map(6.0, 68.0, 5.0, 16.0).to_int_cs() as usize;
        let dots = 1.0f32.lerp(5.0, size).to_int_cs() as usize;
        let mut neuron = None;
        if with_neurons && size > 0.0 && rng.value() > size && rng.value() < 0.33 {
            let length = 50.0f32.lerp(200.0, rng.value());
            if let Some(mut built) =
                CoralNeuron::build(length, None, Some(pos_b), &mut cx.neuron, rng)
            {
                built.neuron.set_all_points(pos_b);
                neuron = Some((built.neuron, built.circles));
            }
        }
        let count = if size == 0.0 {
            20
        } else if rng.value() < 0.5 {
            2
        } else {
            0
        };
        let owner = cx.neuron.pool.new_owner();
        let hang = SegmentHang {
            last_pos: pos_b,
            pos: pos_b,
        };
        let mut mycelia = Vec::with_capacity(count);
        for k in 0..count {
            let length = 30.0f32.lerp(40.0, rng.value()).max(distance);
            let id = cx.neuron.pool.add(
                Some(cx.neuron.system),
                (owner, &hang),
                k,
                length,
                pos_b,
                rng,
            );
            if let Some(strand) = cx.neuron.pool.get_mut(id) {
                strand.use_static_culling = false;
            }
            mycelia.push(id);
        }
        let segment = Self {
            update_order: 0,
            id,
            body,
            size,
            con_rad,
            mesh_segs,
            dots,
            first: false,
            direction: Vec2::ZERO,
            last_direction: Vec2::ZERO,
            neuron: neuron.as_ref().map(|(n, _)| n.update_order),
            owner,
            mycelia,
        };
        (segment, neuron)
    }

    /// What its strands ask of it.
    #[must_use]
    pub fn hang(&self) -> SegmentHang {
        let c = &self.body.chunks[1];
        SegmentHang {
            last_pos: c.last_pos,
            pos: c.pos,
        }
    }
}

impl CoralStem {
    /// The constructor (a placed object's `rootPos` and `rad`) in the room of `cx`, whose
    /// segments are added as it makes them; the caller registers the stem as a vine
    /// first, then the neurons it made. Seeded by `(int)rootPos.x + (int)rootPos.y`.
    pub fn build(
        root_pos: Vec2,
        rad: f32,
        with_neurons: bool,
        cx: &mut StemCx<'_>,
        rng: &mut UnityRandom,
    ) -> NewStem {
        let size = (rad / 600.0).clamp(0.0, 1.0);
        let saved = rng.state();
        rng.init_state(root_pos.x.to_int_cs().wrapping_add(root_pos.y.to_int_cs()));
        let vine_system_order = cx.neuron.new_vine_system.then(|| cx.neuron.next_seq());
        cx.neuron.new_vine_system = false;
        let room = cx.neuron.room;
        let root_tile = root_pos.tile();
        let mut root_direction = Vec2::new(0.0, 1.0);
        for (dx, dy) in FOUR_DIRECTIONS {
            if room.is_solid_tile(root_tile.0 + dx, root_tile.1 + dy) {
                root_direction = Vec2::new(-dx as f32, -dy as f32);
            }
        }
        let path = Self::path(room, cx.ai_map, root_tile, root_direction, size, rng);
        let mut stem = Self {
            system: cx.neuron.system,
            root_pos,
            root_direction,
            root_tile,
            segments: Vec::new(),
            size,
            with_neurons,
        };
        let mut neurons = Vec::new();
        let mut circles = Vec::new();
        let total = path.len() as f32 * 20.0;
        let mut travelled = 0.0f32;
        while travelled < total {
            let mut num5 = 1.0 - travelled.inverse_lerp(0.0, total);
            num5 *= size.lerp(1.0, 0.05);
            num5 = 1.0 - num5;
            travelled += 6.0f32.lerp(12.0, num5);
            let pos_a = Self::along(&path, travelled);
            travelled += 6.0f32.lerp(68.0, num5);
            let pos_b = Self::along(&path, travelled);
            if travelled > total {
                num5 = 1.0;
            }
            let (mut segment, neuron) =
                StemSegment::build(1.0 - num5, pos_a, pos_b, with_neurons, cx, rng);
            if let Some((n, c)) = neuron {
                neurons.push(n);
                circles.extend(c);
            }
            segment.update_order = cx.neuron.next_seq();
            if stem.segments.is_empty() {
                segment.first = true;
            }
            stem.segments.push(segment);
        }
        if cx.neuron.projector {
            let order = cx.neuron.next_seq();
            let circle = ProjectedCircle::new(
                order,
                CircleOwner::Stem { stem: cx.index },
                0,
                0.2f32.lerp(0.8, size),
                room,
                &cx.neuron.circles,
                rng,
            );
            cx.neuron.circles.push(order);
            circles.push(circle);
        }
        rng.set_state(saved);
        NewStem {
            vine_system_order,
            stem,
            neurons,
            circles,
        }
    }

    /// The tiles the stem grows through: from the root tile, each step to the open
    /// neighbour with most room around it (cubed proximity), a random lean growing from
    /// step 10 to 30, and a push away from the last nine tiles.
    fn path(
        room: &Room,
        map: &AiMap,
        root: (i32, i32),
        root_direction: Vec2,
        size: f32,
        rng: &mut UnityRandom,
    ) -> Vec<(i32, i32)> {
        let mut at = root;
        let mut list: Vec<(i32, i32)> = Vec::new();
        let steps = 2.0f32.lerp(60.0, size);
        let mut j = 0;
        while (j as f32) < steps {
            let mut best = f32::MIN;
            let mut step = (root_direction.x.to_int_cs(), root_direction.y.to_int_cs());
            for (dx, dy) in FOUR_DIRECTIONS {
                let next = (at.0 + dx, at.1 + dy);
                if room.is_solid_tile(next.0, next.1) || map.narrow_space(next.0, next.1) {
                    continue;
                }
                let mut score = 0.0f32;
                for (lx, ly) in FOUR_DIRECTIONS {
                    let proximity = map.terrain_proximity(next.0 + lx, next.1 + ly).min(5);
                    score += (proximity as f32).mathf_pow(3.0);
                }
                score += rng.value() * 10.0 * (j as f32).inverse_lerp(10.0, 30.0);
                let mut back = list.len() as i32 - 1;
                while back >= 0 && back > list.len() as i32 - 10 {
                    let tile = list[back as usize];
                    let (dx, dy) = ((tile.0 - next.0) as f32, (tile.1 - next.1) as f32);
                    score += (dx * dx + dy * dy).sqrt() * 0.3;
                    back -= 1;
                }
                if score > best {
                    step = (dx, dy);
                    best = score;
                }
            }
            if best == f32::MIN {
                break;
            }
            list.push(at);
            at = (at.0 + step.0, at.1 + step.1);
            j += 1;
        }
        list
    }

    /// The point `travelled` px along the path of tiles (20 px a tile).
    fn along(path: &[(i32, i32)], travelled: f32) -> Vec2 {
        let n = (travelled / 20.0).floor() as i32;
        let a = Vec2::middle_of_tile(path[path.len().saturating_sub(1).min(n.max(0) as usize)]);
        let b =
            Vec2::middle_of_tile(path[path.len().saturating_sub(1).min((n + 1).max(0) as usize)]);
        a.lerp(
            b,
            travelled
                .inverse_lerp(n as f32 * 20.0, (n + 1) as f32 * 20.0)
                .clamp(0.0, 1.0),
        )
    }

    /// `TotalPositions`.
    #[must_use]
    pub fn total_positions(&self) -> usize {
        self.segments.len() * 2
    }

    /// `Pos(index)`.
    #[must_use]
    pub fn vine_pos(&self, index: usize) -> Vec2 {
        self.segments[index / 2].body.chunks[index % 2].pos
    }

    /// `Rad(index)`.
    #[must_use]
    pub fn vine_rad(&self, index: usize) -> f32 {
        self.segments[index / 2].body.chunks[index % 2].rad
    }

    /// `Mass(index)`.
    #[must_use]
    pub fn vine_mass(&self, index: usize) -> f32 {
        self.segments[index / 2].body.chunks[index % 2].mass
    }

    /// `Push(index, movement)`.
    pub fn push(&mut self, index: usize, movement: Vec2) {
        let c = &mut self.segments[index / 2].body.chunks[index % 2];
        c.pos += movement;
        c.vel += movement;
    }

    /// `CircleCenter(index, timeStacker)`: the last segment's chunk 1. `None` for a stem
    /// without segments (the game's would throw).
    #[must_use]
    pub fn circle_center(&self, time_stacker: f32) -> Option<Vec2> {
        let c = &self.segments.last()?.body.chunks[1];
        Some(c.last_pos.lerp(c.pos, time_stacker.clamp(0.0, 1.0)))
    }

    /// `PhysicalObject.Update`'s part before the segment's own: the chunks move and
    /// collide, then the rod. Returns the terrain impacts' room calls in order. Nothing
    /// happens while the system is frozen (`frozen`).
    pub fn segment_physics(
        &mut self,
        k: usize,
        frozen: bool,
        tick: &StemTick<'_>,
    ) -> Vec<ImpactCall> {
        if frozen {
            return Vec::new();
        }
        let body = &mut self.segments[k].body;
        body.params.room_gravity = tick.room_gravity;
        let mut log = ImpactCalls::default();
        body.update_chunks(tick.room, tick.water, &mut log);
        body.update_connections();
        log.0
    }

    /// The rest of `StemSegment.Update` after `base.Update`: the strands, the pull on its
    /// neuron, the constraint to the segment before, the root, the push off the walls and
    /// the wind, then the direction. `neuron` is the segment's own neuron.
    pub fn segment_after_physics(
        &mut self,
        k: usize,
        neuron: Option<&mut CoralNeuron>,
        pool: &mut MyceliumPool,
        env: &MyceliumEnv<'_>,
        tick: &StemTick<'_>,
        rng: &mut UnityRandom,
    ) {
        self.update_strands(k, pool, env, rng);
        if let Some(neuron) = neuron {
            self.pull_neuron(k, neuron);
        }
        if k > 0 {
            self.hold_to_previous(k);
        }
        if self.segments[k].first {
            let (root_pos, root_dir) = (self.root_pos, self.root_direction);
            let s = &mut self.segments[k];
            s.body.chunks[0].collide_with_terrain = false;
            s.body.chunks[0].pos = root_pos - root_dir * s.body.chunks[0].rad;
            s.body.chunks[0].vel *= 0.0;
            s.body.chunks[1].vel += root_dir * 2.0;
        }
        if k > 0 {
            for j in 0..2 {
                let at = self.segments[k].body.chunks[j].pos.tile();
                let push = Self::wall_push(tick.room, tick.ai_map, at);
                self.segments[k].body.chunks[j].vel += push.normalized() * 0.1 + tick.wind * 0.005;
            }
        }
        let s = &mut self.segments[k];
        s.last_direction = s.direction;
        s.direction = s.body.chunks[0].pos.direction_to(s.body.chunks[1].pos);
    }

    fn update_strands(
        &mut self,
        k: usize,
        pool: &mut MyceliumPool,
        env: &MyceliumEnv<'_>,
        rng: &mut UnityRandom,
    ) {
        let count = self.segments[k].mycelia.len();
        for i in 0..count {
            let s = &self.segments[k];
            let (c0, c1) = (s.body.chunks[0].pos, s.body.chunks[1].pos);
            let dir =
                Vec2::from_degrees(c0.to_aim(c1) - 30.0 + i as f32 / (count - 1) as f32 * 60.0);
            let id = s.mycelia[i];
            if let Some(m) = pool.get_mut(id) {
                m.points[1][0] = m.points[1][0].lerp(c1 + dir * m.con_rad, 0.01);
                m.points[1][2] += dir * 5.0;
            }
            let hang = self.segments[k].hang();
            pool.update(id, &hang, env, rng);
        }
    }

    /// The neuron block: segment 0 of the neuron is pulled to chunk 1 (and chunk 0 takes
    /// the share), segment 1 pushed by the previous segment's side.
    pub(crate) fn pull_neuron(&mut self, k: usize, neuron: &mut CoralNeuron) {
        let c1 = self.segments[k].body.chunks[1].pos;
        let mass = self.segments[k].body.chunks[1].mass;
        let seg0 = neuron.geometry.segments[0][0];
        let dir = c1.direction_to(seg0);
        let dist = c1.distance(seg0);
        let share = mass / (CoralNeuron::vine_mass(0) + mass);
        let to_chunk = dir * ((dist - 1.0) * (1.0 - share));
        let to_neuron = dir * ((dist - 1.0) * share);
        let s = &mut self.segments[k].body.chunks[0];
        s.vel += to_chunk;
        s.pos += to_chunk;
        neuron.geometry.segments[0][2] -= to_neuron;
        neuron.geometry.segments[0][0] -= to_neuron;
        if k > 0 {
            let c0 = self.segments[k].body.chunks[0].pos;
            let prev0 = self.segments[k - 1].body.chunks[0].pos;
            let seg1 = neuron.geometry.segments[1][0];
            let mut dir = prev0.direction_to(seg1);
            dir *= c0
                .direction_to(c1)
                .dot(c1.direction_to(seg1))
                .inverse_lerp(1.0, -1.0);
            let share = 0.8;
            self.segments[k - 1].body.chunks[0].vel -= dir * ((1.0 - share) * 1.8);
            neuron.geometry.segments[1][2] += dir * (share * 1.8);
        }
    }

    /// The constraint to the previous segment: chunk 0 and the previous chunk 1 `conRad`
    /// apart, and the bend of the three before it.
    fn hold_to_previous(&mut self, k: usize) {
        let con_rad = self.segments[k].con_rad;
        let (c0, c1) = {
            let s = &self.segments[k].body.chunks;
            (s[0].clone(), s[1].clone())
        };
        let prev1 = self.segments[k - 1].body.chunks[1].clone();
        let dir = c0.pos.direction_to(prev1.pos);
        let dist = c0.pos.distance(prev1.pos);
        let share = prev1.mass / (prev1.mass + c0.mass);
        let pull_this = dir * ((dist - con_rad) * share);
        let pull_prev = dir * ((dist - con_rad) * (1.0 - share));
        let s = &mut self.segments[k].body.chunks[0];
        s.pos += pull_this;
        s.vel += pull_this;
        let p = &mut self.segments[k - 1].body.chunks[1];
        p.pos -= pull_prev;
        p.vel -= pull_prev;
        if k > 1 {
            let prev0 = self.segments[k - 1].body.chunks[0].pos;
            let prev1 = self.segments[k - 1].body.chunks[1].pos;
            let pp0 = self.segments[k - 2].body.chunks[0].clone();
            let c0_pos = self.segments[k].body.chunks[0].pos;
            let dir = pp0.pos.direction_to(c1.pos);
            let share = pp0.mass / (pp0.mass + c1.mass);
            let bend = 0.5
                + 0.5
                    * (prev0 - c1.pos)
                        .normalized()
                        .dot((prev1 - c0_pos).normalized())
                        .inverse_lerp(0.0, -1.0);
            self.segments[k].body.chunks[1].vel += dir * (1.6 * bend * share);
            self.segments[k - 2].body.chunks[0].vel -= dir * (1.6 * bend * (1.0 - share));
        }
    }
}

impl CoralStem {
    /// The push off the walls around `tile`: each open, not narrow neighbour weighted by
    /// the terrain proximity (at most 4) around it.
    fn wall_push(room: &Room, map: &AiMap, tile: (i32, i32)) -> Vec2 {
        let mut push = Vec2::ZERO;
        for (kx, ky) in FOUR_DIRECTIONS {
            let side = (tile.0 + kx, tile.1 + ky);
            if !room.is_solid_tile(side.0, side.1) && !map.narrow_space(side.0, side.1) {
                let mut weight = 0.0;
                for (lx, ly) in FOUR_DIRECTIONS {
                    weight += map.terrain_proximity(side.0 + lx, side.1 + ly).min(4) as f32;
                }
                push += Vec2::new(kx as f32, ky as f32) * weight;
            }
        }
        push
    }
}

#[cfg(test)]
#[path = "coral_stem_tests.rs"]
mod tests;
