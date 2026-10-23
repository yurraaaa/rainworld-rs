//! `CoralStem` and `StemSegment` against `CoralStem.cs` and `StemSegment.cs`: the seeded
//! constructor (the path search's draws, a hand-worked one-segment stem, each segment's
//! draws in the game's order), the neuron a segment makes, the constraint to the segment
//! before it, the root, the frozen gate and the climbable vine members.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use super::{CoralStem, NewStem, SegmentHang, StemCx, StemSegment, StemTick};
use crate::coral_neuron::{CoralNeuron, NeuronCx};
use crate::creature::IdIssuer;
use crate::mycelium::{MyceliumEnv, MyceliumPool, SystemId, SystemState};
use crate::tests::room;
use crate::world::AiMap;
use crate::{CameraView, FloatExt, PlacedVines, RoomVines, Vec2, VineHostMut, VineRef};

/// A 60 x 40 tile room with a solid border: tile (30, 20) is far from every wall.
fn big_room() -> rw_assets::Room {
    let mut rows = vec!["#".repeat(60)];
    rows.extend((0..38).map(|_| format!("#{}#", ".".repeat(58))));
    rows.push("#".repeat(60));
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

fn camera() -> CameraView {
    CameraView {
        room: 0,
        current: 0,
        next: 0,
    }
}

/// The world a stem is built in.
struct Setup {
    room: rw_assets::Room,
    map: AiMap,
    pool: MyceliumPool,
    ids: IdIssuer,
    seq: u64,
}

impl Setup {
    fn new() -> Self {
        let room = big_room();
        let map = AiMap::new(&room, 0, false, false);
        let mut pool = MyceliumPool::default();
        pool.create_registry();
        Self {
            room,
            map,
            pool,
            ids: IdIssuer {
                next_issued_id: 1000,
            },
            seq: 10,
        }
    }

    fn cx(&mut self, projector: bool) -> StemCx<'_> {
        StemCx {
            neuron: NeuronCx {
                room: &self.room,
                system: SystemId(1),
                pool: &mut self.pool,
                projector,
                circles: Vec::new(),
                new_vine_system: false,
                seq: &mut self.seq,
            },
            ai_map: &self.map,
            ids: &mut self.ids,
            index: 0,
        }
    }

    fn stem(&mut self, root: Vec2, rad: f32, with_neurons: bool, seed: i32) -> NewStem {
        let mut rng = UnityRandom::new(seed);
        CoralStem::build(root, rad, with_neurons, &mut self.cx(false), &mut rng)
    }

    fn env(&self, frozen: bool) -> MyceliumEnv<'_> {
        MyceliumEnv {
            room: &self.room,
            room_index: 0,
            ai_map: Some(&self.map),
            camera: camera(),
            cameras_changed_tick: 1,
            system: Some(SystemState {
                id: SystemId(1),
                frozen,
            }),
        }
    }

    fn tick(&self) -> StemTick<'_> {
        StemTick {
            room: &self.room,
            ai_map: &self.map,
            water: None,
            wind: Vec2::new(0.4, -0.3),
            room_gravity: 1.0,
        }
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

/// A stream advanced by `n` draws, to compare states with.
fn after(seed: i32, n: usize) -> [u32; 4] {
    let mut r = UnityRandom::new(seed);
    for _ in 0..n {
        r.value();
    }
    r.state()
}

/// The constructor is seeded by `(int)rootPos.x + (int)rootPos.y`; the stream outside is
/// untouched.
#[test]
fn the_constructor_leaves_the_shared_stream_alone() {
    let mut s = Setup::new();
    let mut rng = UnityRandom::new(5);
    let before = rng.state();
    CoralStem::build(
        Vec2::new(610.0, 410.0),
        200.0,
        true,
        &mut s.cx(false),
        &mut rng,
    );
    assert_eq!(rng.state(), before);
}

/// `rad` 0 is size 0: a path of two tiles from the root tile (open air all round: the
/// first step goes to the first of the four directions, the left one, as the scores tie
/// and `>` keeps the first; the second leaves the tile just visited behind) and, since the
/// whole 40 px is covered at the first segment, one segment of size 0 with 20 strands.
/// `travelled` runs `Lerp(6, 12, 0.95)` = 11.7 to the first chunk (58.5 % along the first
/// 20 px) and 64.9 more to the second, past the end, on the last tile's middle.
#[test]
fn a_size_zero_stem_is_one_segment() {
    let mut s = Setup::new();
    let built = s.stem(Vec2::new(610.0, 410.0), 0.0, false, 1);
    let stem = &built.stem;
    assert_eq!(stem.root_tile, (30, 20));
    assert_eq!(stem.root_direction, Vec2::new(0.0, 1.0));
    assert_eq!(stem.size, 0.0);
    assert_eq!(stem.segments.len(), 1);
    let g = &stem.segments[0];
    assert!(g.first);
    assert_eq!(g.size, 0.0);
    assert_eq!(g.mycelia.len(), 20);
    assert_eq!(g.id.number, 1001);
    let (c0, c1) = (&g.body.chunks[0], &g.body.chunks[1]);
    assert!(near(c0.pos.x, 610.0 + (590.0 - 610.0) * 0.585) && near(c0.pos.y, 410.0));
    assert!(near(c1.pos.x, 590.0) && near(c1.pos.y, 410.0));
    assert_eq!((c0.rad, c0.mass), (4.0, 0.015));
    assert_eq!(g.body.connections[0].distance, 6.0);
    assert_eq!((g.con_rad, g.mesh_segs, g.dots), (6.0, 5, 1));
    assert_eq!(g.update_order, 11);
    assert!(built.neurons.is_empty() && built.circles.is_empty());
}

/// The path search draws `value` once for every open, not narrow neighbour at every step
/// (the lean is `* InverseLerp(10, 30, j)` and is added whatever it is): in open air, 4 a
/// step for `(int)Ceil(Lerp(2, 60, size))` steps.
#[test]
fn the_path_search_draws_for_each_candidate() {
    let s = Setup::new();
    let mut rng = UnityRandom::new(3);
    // Size 0.1: j < 7.8, eight steps.
    let path = CoralStem::path(
        &s.room,
        &s.map,
        (30, 20),
        Vec2::new(0.0, 1.0),
        0.1,
        &mut rng,
    );
    assert_eq!(path.len(), 8);
    assert_eq!(path[0], (30, 20));
    assert_eq!(rng.state(), after(3, 32));
}

/// A segment's draws in the game's order: `RNV` (1), then for a stem without neurons the
/// strand count (`value < 0.5` makes two, a size 0 segment makes 20 without a draw), each
/// strand `Lerp(30, 40, value)` and its `Reset` (`2 + 2 * 2` draws: the length gives two
/// points; the owner's `ResetDir` draws none).
#[test]
fn a_segment_draws_in_the_games_order() {
    for seed in 0..12 {
        let mut s = Setup::new();
        let mut rng = UnityRandom::new(seed);
        StemSegment::build(
            0.3,
            Vec2::new(100.0, 100.0),
            Vec2::new(110.0, 100.0),
            false,
            &mut s.cx(false),
            &mut rng,
        );
        let mut reference = UnityRandom::new(seed);
        reference.value();
        let two = reference.value() < 0.5;
        let n = 2 + if two { 2 * 7 } else { 0 };
        assert_eq!(rng.state(), after(seed, n), "seed {seed}");
    }
    let mut s = Setup::new();
    let mut rng = UnityRandom::new(4);
    StemSegment::build(
        0.0,
        Vec2::new(100.0, 100.0),
        Vec2::new(110.0, 100.0),
        true,
        &mut s.cx(false),
        &mut rng,
    );
    assert_eq!(rng.state(), after(4, 1 + 20 * 7));
}

/// A segment of a stem with neurons (size above 0) draws `value > size`, then, if that
/// holds, `value < 0.33`, then the neuron's length `Lerp(50, 200, value)`; the neuron's
/// own seeded draws leave the stream as they found it. Its segments all start on chunk 1.
#[test]
fn a_segment_may_make_a_neuron() {
    let (mut made, mut not) = (0, 0);
    for seed in 0..80 {
        let mut s = Setup::new();
        let mut rng = UnityRandom::new(seed);
        let (g, neuron) = StemSegment::build(
            0.3,
            Vec2::new(100.0, 100.0),
            Vec2::new(110.0, 100.0),
            true,
            &mut s.cx(false),
            &mut rng,
        );
        let mut reference = UnityRandom::new(seed);
        reference.value();
        let big = reference.value() > 0.3;
        let makes = big && reference.value() < 0.33;
        assert_eq!(neuron.is_some(), makes, "seed {seed}");
        assert_eq!(g.neuron.is_some(), makes);
        let length = 50.0f32.lerp(200.0, {
            let mut r3 = UnityRandom::new(seed);
            for _ in 0..(2 + usize::from(big)) {
                r3.value();
            }
            r3.value()
        });
        if makes {
            made += 1;
            let (n, _) = neuron.unwrap();
            assert_eq!(n.geometry.segments.len(), (length / 20.0) as usize);
            assert!(
                n.geometry
                    .segments
                    .iter()
                    .all(|p| p[0] == g.body.chunks[1].pos && p[1] == g.body.chunks[1].pos)
            );
            assert_eq!(n.pos_a, None);
            assert_eq!(n.pos_b, Some(g.body.chunks[1].pos));
            // The neuron's draws are its own: here the segment draws on after the length,
            // the strand-count `value` next.
            let mut r4 = UnityRandom::new(seed);
            for _ in 0..(3 + usize::from(big)) {
                r4.value();
            }
            let two = r4.value() < 0.5;
            let total = 4 + usize::from(big) + if two { 2 * 7 } else { 0 };
            assert_eq!(rng.state(), after(seed, total), "seed {seed}");
        } else {
            not += 1;
        }
    }
    assert!(made > 0 && not > 0);
}

/// `Pos`, `Rad`, `Mass` and `Push` of the vine read and write the chunks: point `i` is
/// chunk `i % 2` of segment `i / 2`.
#[test]
fn the_vine_members_are_the_chunks() {
    let mut s = Setup::new();
    let mut built = s.stem(Vec2::new(610.0, 410.0), 300.0, false, 2).stem;
    assert!(built.segments.len() > 3);
    assert_eq!(built.total_positions(), built.segments.len() * 2);
    let g = &built.segments[3];
    assert_eq!(built.vine_pos(7), g.body.chunks[1].pos);
    assert_eq!(built.vine_pos(6), g.body.chunks[0].pos);
    assert_eq!(built.vine_rad(7), g.body.chunks[1].rad);
    assert_eq!(built.vine_mass(6), g.body.chunks[0].mass);
    let (pos, vel) = (g.body.chunks[1].pos, g.body.chunks[1].vel);
    let y0 = built.segments[3].body.chunks[0].pos.y;
    built.push(7, Vec2::new(2.0, -1.0));
    let c = &built.segments[3].body.chunks[1];
    assert_eq!(c.pos, pos + Vec2::new(2.0, -1.0));
    assert_eq!(c.vel, vel + Vec2::new(2.0, -1.0));
    // Through the room's host, as the player's push lands.
    let mut placed = PlacedVines::default();
    let mut stems = [built];
    let mut host = RoomVines::placed_only(&mut placed);
    host.stems = &mut stems;
    host.push(
        VineRef::CoralStem { room: 0, stem: 0 },
        6,
        Vec2::new(0.0, 3.0),
    );
    assert_eq!(stems[0].segments[3].body.chunks[0].pos.y, y0 + 3.0);
}

/// The root segment: chunk 0 sits `rad` inside the wall behind the root (collision with
/// the terrain off, still), chunk 1 is pushed out along the root direction by 2.
#[test]
fn the_first_segment_is_pinned_to_the_root() {
    let mut s = Setup::new();
    let mut rng = UnityRandom::new(9);
    let mut stem = s.stem(Vec2::new(610.0, 410.0), 100.0, false, 3).stem;
    let mut pool = std::mem::take(&mut s.pool);
    let tick = s.tick();
    let env = s.env(false);
    let before1 = stem.segments[0].body.chunks[1].vel;
    stem.segment_after_physics(0, None, &mut pool, &env, &tick, &mut rng);
    let c = &stem.segments[0].body.chunks;
    assert!(!c[0].collide_with_terrain);
    assert_eq!(
        c[0].pos,
        Vec2::new(610.0, 410.0) - Vec2::new(0.0, 1.0) * c[0].rad
    );
    assert_eq!(c[0].vel, Vec2::ZERO);
    assert!(near(c[1].vel.x, before1.x) && near(c[1].vel.y, before1.y + 2.0));
}

/// Segment `k` holds chunk 0 `conRad` from the previous segment's chunk 1 (the lighter end
/// moves more) and reads the positions as they are now, a push of the player's included.
#[test]
fn a_segment_sees_the_previous_one_as_pushed_this_tick() {
    let mut s = Setup::new();
    let mut rng = UnityRandom::new(9);
    let mut stem = s.stem(Vec2::new(610.0, 410.0), 300.0, false, 2).stem;
    // The player's push on segment 2's chunk 1 (point 5).
    stem.push(5, Vec2::new(6.0, 0.0));
    let (con_rad, c0, p1) = (
        stem.segments[3].con_rad,
        stem.segments[3].body.chunks[0].clone(),
        stem.segments[2].body.chunks[1].clone(),
    );
    let dir = c0.pos.direction_to(p1.pos);
    let dist = c0.pos.distance(p1.pos);
    let share = p1.mass / (p1.mass + c0.mass);
    let expect_c0 = c0.pos + dir * ((dist - con_rad) * share);
    let expect_p1 = p1.pos - dir * ((dist - con_rad) * (1.0 - share));
    let mut pool = std::mem::take(&mut s.pool);
    let tick = s.tick();
    let env = s.env(false);
    stem.hold_to_previous(3);
    assert!(near(stem.segments[3].body.chunks[0].pos.x, expect_c0.x));
    assert!(near(stem.segments[3].body.chunks[0].pos.y, expect_c0.y));
    assert!(near(stem.segments[2].body.chunks[1].pos.x, expect_p1.x));
    assert!(near(stem.segments[2].body.chunks[1].pos.y, expect_p1.y));
    // A whole turn runs the same constraint and moves the direction on.
    stem.segment_after_physics(3, None, &mut pool, &env, &tick, &mut rng);
    let g = &stem.segments[3];
    assert_eq!(
        g.direction,
        g.body.chunks[0].pos.direction_to(g.body.chunks[1].pos)
    );
}

/// A frozen system stops its segments before they move or draw anything.
#[test]
fn a_frozen_system_stops_the_physics() {
    let mut s = Setup::new();
    let mut stem = s.stem(Vec2::new(610.0, 410.0), 100.0, false, 3).stem;
    let before = stem.clone();
    let tick = s.tick();
    assert!(stem.segment_physics(1, true, &tick).is_empty());
    assert_eq!(stem, before);
    stem.segment_physics(1, false, &tick);
    assert_ne!(stem, before);
}

/// The strands hang from chunk 1 (`Lerp(lastPos, pos, 1)`), reach nowhere in particular
/// (`ResetDir` is zero, no draw) and are moved by the segment before they update.
#[test]
fn the_strands_hang_from_chunk_one() {
    let hang = SegmentHang {
        last_pos: Vec2::new(1.0, 2.0),
        pos: Vec2::new(5.0, 6.0),
    };
    use crate::mycelium::MyceliumOwner;
    assert_eq!(hang.connection_pos(3), Vec2::new(5.0, 6.0));
    let mut rng = UnityRandom::new(1);
    let before = rng.state();
    assert_eq!(hang.reset_dir(0, &mut rng), Vec2::ZERO);
    assert_eq!(rng.state(), before);
}

/// In a projector room the stem adds its circle after its last segment, owned by the stem;
/// without the effect none.
#[test]
fn a_projector_room_gives_the_stem_a_circle() {
    let mut s = Setup::new();
    let mut rng = UnityRandom::new(1);
    let built = CoralStem::build(
        Vec2::new(610.0, 410.0),
        100.0,
        false,
        &mut s.cx(true),
        &mut rng,
    );
    assert_eq!(built.circles.len(), 1);
    let circle = &built.circles[0];
    assert_eq!(circle.owner, crate::CircleOwner::Stem { stem: 0 });
    assert_eq!(circle.index, 0);
    assert_eq!(
        circle.base_rad,
        60.0f32.lerp(180.0, 0.2f32.lerp(0.8, 100.0 / 600.0))
    );
    let last = built.stem.segments.last().unwrap();
    assert!(last.update_order < circle.update_order);
    assert_eq!(built.stem.circle_center(1.0), Some(last.body.chunks[1].pos));
}

/// A fast landing is `PhysicalObject.TerrainImpact`'s: a first contact faster than 4 whose
/// speed times loudness and `Lerp(mass, 1, 0.5)` passes 0.5 makes a noise 0.9 radii off the
/// chunk toward the surface, `Lerp(350, Lerp(100, 1500, InverseLerp(0.5, 20, that)), 0.5)`
/// strong; speed times mass over 7 moves the screen.
#[test]
fn a_fast_landing_makes_the_objects_noise() {
    let mut s = Setup::new();
    let mut stem = s.stem(Vec2::new(610.0, 410.0), 0.0, false, 1).stem;
    let tick = s.tick();
    // Chunk 0 (rad 4, mass 0.015) over the floor (top at y 20), falling at 12.
    stem.segments[0].body.connections[0].active = false;
    let c = &mut stem.segments[0].body.chunks[0];
    c.pos = Vec2::new(300.0, 30.0);
    c.last_pos = c.pos;
    c.vel = Vec2::new(0.0, -12.0);
    let calls = stem.segment_physics(0, false, &tick);
    // `vel.y -= 0.9`, then `* 0.999` in the air.
    let speed = (12.0f32 + 0.9) * 0.999;
    let loud = speed * 1.0 * 0.015f32.lerp(1.0, 0.5);
    let strength = 350.0f32.lerp(100.0f32.lerp(1500.0, loud.inverse_lerp(0.5, 20.0)), 0.5);
    let landed = stem.segments[0].body.chunks[0].pos;
    assert!(near(landed.y, 24.0), "{landed:?}");
    let noises: Vec<_> = calls
        .iter()
        .filter_map(|c| match c {
            crate::ImpactCall::Noise(pos, strength) => Some((*pos, *strength)),
            crate::ImpactCall::ScreenMovement(_) => None,
        })
        .collect();
    assert_eq!(noises.len(), 1);
    assert!(near(noises[0].0.x, landed.x) && near(noises[0].0.y, landed.y - 4.0 * 0.9));
    assert!(near(noises[0].1, strength));
    // 0.015 * 12.9 is far under 7: no screen movement.
    assert!(
        calls
            .iter()
            .all(|c| !matches!(c, crate::ImpactCall::ScreenMovement(_)))
    );
}

/// The neuron block: chunk 0 takes `(dist - 1) * (1 - share)` toward the neuron's segment 0 and
/// the segment `share` toward chunk 1 (`share = mass / (0.5 + mass)`); then the previous
/// segment's chunk 0 and the neuron's segment 1 are pushed along `DirVec(prev chunk 0, seg 1)`
/// scaled by `InverseLerp(1, -1, Dot(DirVec(chunk 0, chunk 1), DirVec(chunk 1, seg 1)))`, read
/// with chunk 0 **after** its move.
#[test]
fn the_neuron_block_reads_chunk_zero_after_its_move() {
    let mut s = Setup::new();
    let mut stem = s.stem(Vec2::new(610.0, 410.0), 300.0, false, 2).stem;
    let mut rng = UnityRandom::new(1);
    let mut neuron = {
        let mut cx = s.cx(false);
        CoralNeuron::build(
            60.0,
            None,
            Some(Vec2::new(500.0, 500.0)),
            &mut cx.neuron,
            &mut rng,
        )
        .expect("a neuron")
        .neuron
    };
    let at = stem.segments[3].body.chunks[1].pos;
    neuron.geometry.segments[0][0] = at + Vec2::new(30.0, 4.0);
    neuron.geometry.segments[1][0] = at + Vec2::new(-20.0, 25.0);
    let (c0, mass) = (
        stem.segments[3].body.chunks[0].pos,
        stem.segments[3].body.chunks[1].mass,
    );
    let (seg0, seg1) = (
        neuron.geometry.segments[0][0],
        neuron.geometry.segments[1][0],
    );
    let (prev0, prev_vel) = (
        stem.segments[2].body.chunks[0].pos,
        stem.segments[2].body.chunks[0].vel,
    );
    let seg1_vel = neuron.geometry.segments[1][2];
    let share = mass / (0.5 + mass);
    let dir = at.direction_to(seg0);
    let dist = at.distance(seg0);
    let moved = c0 + dir * ((dist - 1.0) * (1.0 - share));
    let new_seg0 = seg0 - dir * ((dist - 1.0) * share);
    let scale = moved
        .direction_to(at)
        .dot(at.direction_to(seg1))
        .inverse_lerp(1.0, -1.0);
    let push = prev0.direction_to(seg1) * scale;
    stem.pull_neuron(3, &mut neuron);
    let c = &stem.segments[3].body.chunks[0];
    assert!(near(c.pos.x, moved.x) && near(c.pos.y, moved.y));
    let n0 = neuron.geometry.segments[0][0];
    assert!(near(n0.x, new_seg0.x) && near(n0.y, new_seg0.y));
    let prev = stem.segments[2].body.chunks[0].vel - prev_vel;
    let want = push * -(0.2 * 1.8);
    assert!(
        near(prev.x, want.x) && near(prev.y, want.y),
        "{prev:?} {want:?}"
    );
    let on_neuron = neuron.geometry.segments[1][2] - seg1_vel;
    let want = push * (0.8 * 1.8);
    assert!(near(on_neuron.x, want.x) && near(on_neuron.y, want.y));
}
