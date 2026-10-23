//! `CoralNeuron` against `CoralNeuron.cs`: the seeded constructor (segments, strands,
//! bumps in the game's draw order, the shared stream untouched), the wall snap of the
//! ends, a hand-worked `Update`, its draws, the frozen gate, the circles it adds and the
//! climbable vine members.

use rw_random::UnityRandom;

use super::{CoralNeuron, NeuronCx};
use crate::superstructure::coral::mycelium::{MyceliumEnv, MyceliumPool, SystemId, SystemState};
use crate::tests::room;
use crate::world::AiMap;
use crate::{
    CameraView, FloatExt, PlacedVines, RoomVines, RoomVinesView, Vec2, VineHost, VineHostMut,
    VineKind, VineRef,
};

/// A 30 x 20 tile room with a solid border: tile (15, 10) is far from every wall.
fn big_room() -> rw_assets::Room {
    let mut rows = vec!["#".repeat(30)];
    rows.extend((0..18).map(|_| format!("#{}#", ".".repeat(28))));
    rows.push("#".repeat(30));
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

/// A 8 x 6 tile room with a solid border.
fn small_room() -> rw_assets::Room {
    room(&[
        "########", "#......#", "#......#", "#......#", "#......#", "########",
    ])
}

fn camera() -> CameraView {
    CameraView {
        room: 0,
        current: 0,
        next: 0,
    }
}

/// A neuron made in `r` with a fresh pool and system; the shared stream is `rng`.
struct Made {
    neuron: CoralNeuron,
    circles: Vec<crate::ProjectedCircle>,
    pool: MyceliumPool,
    seq: u64,
}

impl Made {
    fn new(
        r: &rw_assets::Room,
        length: f32,
        ends: (Option<Vec2>, Option<Vec2>),
        projector: bool,
        rng: &mut UnityRandom,
    ) -> Self {
        let mut pool = MyceliumPool::default();
        let system = pool.create_registry();
        let mut seq = 10;
        let built = {
            let mut cx = NeuronCx {
                room: r,
                system,
                pool: &mut pool,
                projector,
                circles: Vec::new(),
                new_vine_system: false,
                seq: &mut seq,
            };
            CoralNeuron::build(length, ends.0, ends.1, &mut cx, rng).expect("a neuron")
        };
        Self {
            neuron: built.neuron,
            circles: built.circles,
            pool,
            seq,
        }
    }
}

fn env<'a>(r: &'a rw_assets::Room, map: &'a AiMap, frozen: bool) -> MyceliumEnv<'a> {
    MyceliumEnv {
        room: r,
        room_index: 0,
        ai_map: Some(map),
        camera: camera(),
        cameras_changed_tick: 1,
        system: Some(SystemState {
            id: SystemId(1),
            frozen,
        }),
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

/// The constructor in open air, from seed `(int)length`: per segment `RNV() * value` for
/// the position (two ends) and again for the velocity, then per strand pair side
/// `Lerp(30, 300, value)` capped by the contour and the strand's `Reset` (`2 + 2n`
/// draws), then the bumps' `x` and `y`; the stream outside is restored.
#[test]
fn the_constructor_follows_its_seed() {
    let r = big_room();
    let (a, b) = (Vec2::new(300.0, 200.0), Vec2::new(400.0, 200.0));
    let mut rng = UnityRandom::new(77);
    let before = rng.state();
    let made = Made::new(&r, 100.0, (Some(a), Some(b)), false, &mut rng);
    assert_eq!(rng.state(), before);
    let n = &made.neuron;
    assert_eq!(n.geometry.segments.len(), 5);
    assert_eq!((n.geometry.root_dir_a, n.geometry.root_dir_b), (None, None));
    assert_eq!(n.con_rad, 100.0 / 5.0 * 1.5);

    let mut seeded = UnityRandom::new(100);
    for (k, s) in n.geometry.segments.iter().enumerate() {
        let jitter = Vec2::random_unit(&mut seeded) * seeded.value();
        let at = a.lerp(b, k as f32 / 4.0) + jitter;
        assert_eq!(s[0], at);
        assert_eq!(s[1], at);
        assert_eq!(s[2], Vec2::random_unit(&mut seeded) * seeded.value());
    }
    // Three pairs (segments - 2), no rooted end: the contour is 1, 0.9, 1 for pairs
    // 0, 1, 2 (`Max(InverseLerp(10, 0, i), InverseLerp(pairs - 11, pairs - 1, i))`).
    assert_eq!(n.geometry.pairs, 3);
    assert_eq!(n.mycelia.len(), 6);
    for l in 0..3 {
        let contour = [1.0, 0.9, 1.0][l];
        for m in 0..2 {
            let random = 30.0f32.lerp(300.0, seeded.value());
            let length = random.min(30.0f32.lerp(300.0, contour));
            let strand = made.pool.get(n.mycelia[l * 2 + m]).expect("strand");
            assert_eq!(strand.length, length);
            assert!(!strand.use_static_culling);
            let points = ((length.max(length.lerp(40.0, 0.5)) / 15.0) as i32).clamp(2, 20);
            assert_eq!(strand.points.len(), points as usize);
            // `Reset` draws `2 + 2n` unit vectors (the owner's `ResetDir` none).
            for _ in 0..2 + 2 * points {
                seeded.value();
            }
        }
    }
    // Bumps follow: `(int)(5 * 1.2)` = 6, `x` then `y`.
    assert_eq!(n.bumps.len(), 6);
    for bump in &n.bumps {
        assert_eq!(bump.x, (-1.0f32).lerp(1.0, seeded.value()));
        assert_eq!(bump.y, seeded.value());
    }
    assert_eq!(
        made.pool.registry(SystemId(1)).len(),
        6,
        "the strands join the system"
    );
}

/// An end with a Solid neighbour tile is rooted on it: the direction away from the wall,
/// the end moved to the wall tile's edge along the axis through it. The last of the four
/// directions (-x, -y, +x, +y) found Solid wins, each moving the already moved end.
#[test]
fn ends_snap_to_the_wall_behind_them() {
    let r = small_room();
    // Tile (3, 1): only (3, 0) below is Solid. rootDir = up; y = middle(30) - 1 * 20 = 10.
    let mut rng = UnityRandom::new(3);
    let made = Made::new(
        &r,
        100.0,
        (Some(Vec2::new(70.0, 30.0)), Some(Vec2::new(70.0, 90.0))),
        false,
        &mut rng,
    );
    // The fields keep the placed positions; only the first line of segments used the snap.
    assert_eq!(made.neuron.pos_a, Some(Vec2::new(70.0, 30.0)));
    let first = made.neuron.geometry.segments[0][0];
    assert!(first.distance(Vec2::new(70.0, 10.0)) <= 1.0, "{first:?}");
    assert_eq!(made.neuron.geometry.root_dir_a, Some(Vec2::new(0.0, 1.0)));
    // Tile (3, 4) beside the ceiling row: (3, 5) is Solid, rootDir = down, y = 90 + 20.
    assert_eq!(made.neuron.pos_b, Some(Vec2::new(70.0, 90.0)));
    assert_eq!(made.neuron.geometry.root_dir_b, Some(Vec2::new(0.0, -1.0)));
    // The corner tile (1, 1): -x Solid first (rootDir +x, x = 30 - 20 = 10), then -y
    // (rootDir +y, y = 30 - 20 = 10): the end is at (10, 10), rooted upward.
    let corner = Made::new(
        &r,
        100.0,
        (Some(Vec2::new(30.0, 30.0)), None),
        false,
        &mut UnityRandom::new(3),
    );
    assert_eq!(corner.neuron.pos_a, Some(Vec2::new(30.0, 30.0)));
    assert_eq!(corner.neuron.geometry.root_dir_a, Some(Vec2::new(0.0, 1.0)));
    assert_eq!(corner.neuron.pos_b, None);
    // `ConnectToWalls` holds segment 0 at the placed position minus 10 along the root.
    let map = AiMap::new(&r, 0, false, false);
    let mut made = made;
    made.neuron.update(
        Vec2::ZERO,
        &mut made.pool,
        &env(&r, &map, false),
        &mut UnityRandom::new(1),
    );
    assert_eq!(made.neuron.geometry.segments[0][0], Vec2::new(70.0, 20.0));
}

/// A tick of three segments 30 apart (`conRad` 30) with a wind of (1, 0), worked by hand:
/// the spreading nudge `DirVec * 0.15`, the drift (`vel * 0.999`, wind `* 0.005`, the
/// pull toward the free ends' places over `420`), then `Connect` descending and ascending.
#[test]
fn an_update_follows_the_game() {
    let r = big_room();
    let map = AiMap::new(&r, 0, false, false);
    let mut rng = UnityRandom::new(1);
    let ends = (Some(Vec2::new(280.0, 200.0)), Some(Vec2::new(340.0, 200.0)));
    let mut made = Made::new(&r, 60.0, ends, false, &mut rng);
    let n = &mut made.neuron;
    assert_eq!((n.geometry.segments.len(), n.con_rad), (3, 30.0));
    for (s, x) in n.geometry.segments.iter_mut().zip([280.0, 310.0, 340.0]) {
        *s = [Vec2::new(x, 200.0); 3];
        s[2] = Vec2::ZERO;
    }
    n.update(
        Vec2::new(1.0, 0.0),
        &mut made.pool,
        &env(&r, &map, false),
        &mut rng,
    );
    let s = &n.geometry.segments;
    assert!(
        near(s[0][0].x, 279.9625) && near(s[0][0].y, 200.0),
        "{:?}",
        s[0]
    );
    assert!(near(s[1][0].x, 310.01875), "{:?}", s[1]);
    assert!(near(s[2][0].x, 340.01875), "{:?}", s[2]);
    assert!(near(s[0][2].x, -0.031_992_857), "{:?}", s[0]);
    assert!(near(s[1][2].x, 0.02375), "{:?}", s[1]);
    assert!(near(s[2][2].x, 0.023_242_857), "{:?}", s[2]);
    assert_eq!(
        [s[0][1].x, s[1][1].x, s[2][1].x],
        [280.0, 310.0, 340.0],
        "the last positions are the old ones"
    );
}

/// The shared stream's draws of a tick: one `Range` for each strand that looks for a
/// partner (unlinked, resting over, the system holding strands) and one for each bump
/// whose ping is 0.
#[test]
fn a_tick_draws_a_strand_pick_and_a_ping_chance() {
    let r = big_room();
    let map = AiMap::new(&r, 0, false, false);
    let ends = (Some(Vec2::new(300.0, 200.0)), Some(Vec2::new(400.0, 200.0)));
    let mut made = Made::new(&r, 100.0, ends, false, &mut UnityRandom::new(4));
    let mut rng = UnityRandom::new(9);
    made.neuron
        .update(Vec2::ZERO, &mut made.pool, &env(&r, &map, false), &mut rng);
    let mut expected = UnityRandom::new(9);
    for _ in 0..6 + 6 {
        expected.value();
    }
    assert_eq!(rng.state(), expected.state());
}

/// `if (system.Frozen) return;`: no movement, no draws, the strands untouched.
#[test]
fn a_frozen_system_stops_the_neuron() {
    let r = big_room();
    let map = AiMap::new(&r, 0, false, false);
    let ends = (Some(Vec2::new(300.0, 200.0)), Some(Vec2::new(400.0, 200.0)));
    let mut made = Made::new(&r, 100.0, ends, false, &mut UnityRandom::new(4));
    let mut rng = UnityRandom::new(9);
    let (segments, strand) = (
        made.neuron.geometry.segments.clone(),
        made.pool.get(made.neuron.mycelia[0]).cloned(),
    );
    made.neuron.update(
        Vec2::new(1.0, 1.0),
        &mut made.pool,
        &env(&r, &map, true),
        &mut rng,
    );
    assert_eq!(rng.state(), UnityRandom::new(9).state());
    assert_eq!(made.neuron.geometry.segments, segments);
    assert_eq!(made.pool.get(made.neuron.mycelia[0]).cloned(), strand);
}

/// A ping decays by 0.1 a tick to 0 and keeps the last tick's value for the draw.
#[test]
fn pings_decay_and_remember() {
    let r = big_room();
    let map = AiMap::new(&r, 0, false, false);
    let ends = (Some(Vec2::new(300.0, 200.0)), Some(Vec2::new(400.0, 200.0)));
    let mut made = Made::new(&r, 100.0, ends, false, &mut UnityRandom::new(4));
    made.neuron.bump_pings[2] = [1.0, 0.0];
    let mut rng = UnityRandom::new(9);
    made.neuron
        .update(Vec2::ZERO, &mut made.pool, &env(&r, &map, false), &mut rng);
    assert_eq!(made.neuron.bump_pings[2], [0.9, 1.0]);
}

/// The projector room's circles: circle 0 on a free start when the end is rooted or the
/// neuron is longer than 100, circle 1 on a free end; the circles join the update list
/// before the neuron does, and each takes the neuron as its owner.
#[test]
fn circles_are_added_at_the_free_ends() {
    let r = small_room();
    let free = (Some(Vec2::new(70.0, 50.0)), Some(Vec2::new(70.0, 70.0)));
    // Both ends free, 60 long: circle 1 only (circle 0 needs a rooted B or length > 100).
    let a = Made::new(&r, 60.0, free, true, &mut UnityRandom::new(1));
    assert_eq!(a.circles.len(), 1);
    assert_eq!(a.circles[0].index, 1);
    // Longer than 100: both.
    let b = Made::new(&r, 120.0, free, true, &mut UnityRandom::new(1));
    let indices: Vec<usize> = b.circles.iter().map(|c| c.index).collect();
    assert_eq!(indices, [0, 1]);
    // Orders: the circles, then the neuron.
    assert_eq!(
        (b.circles[0].update_order, b.circles[1].update_order),
        (11, 12)
    );
    assert_eq!(b.neuron.update_order, 13);
    assert_eq!(b.seq, 13);
    assert!(
        b.circles
            .iter()
            .all(|c| c.owner == crate::CircleOwner::Neuron { order: 13 })
    );
    // B rooted on the ceiling, A free and short: circle 0 only.
    let rooted = (Some(Vec2::new(70.0, 50.0)), Some(Vec2::new(70.0, 90.0)));
    let c = Made::new(&r, 60.0, rooted, true, &mut UnityRandom::new(1));
    assert_eq!(c.circles.len(), 1);
    assert_eq!(c.circles[0].index, 0);
    // No projector: none.
    let d = Made::new(&r, 120.0, free, false, &mut UnityRandom::new(1));
    assert!(d.circles.is_empty());
}

/// The circles' constructor draws come from the neuron's seeded stream: the shared stream
/// is the same with or without them.
#[test]
fn circle_draws_stay_inside_the_seed() {
    let r = small_room();
    let free = (Some(Vec2::new(70.0, 50.0)), Some(Vec2::new(70.0, 70.0)));
    let mut rng = UnityRandom::new(5);
    let before = rng.state();
    Made::new(&r, 120.0, free, true, &mut rng);
    assert_eq!(rng.state(), before);
}

/// `IClimbableVine`: one position per segment, radius 2, mass 0.5, always climbable,
/// `Push` moves the position and the velocity.
#[test]
fn the_vine_members() {
    let r = big_room();
    let ends = (Some(Vec2::new(300.0, 200.0)), Some(Vec2::new(400.0, 200.0)));
    let mut made = Made::new(&r, 100.0, ends, false, &mut UnityRandom::new(4));
    made.neuron.update_order = 7;
    let vine = VineRef::CoralNeuron { room: 0, order: 7 };
    let placed = crate::PlacedVines::default();
    let neurons = std::slice::from_ref(&made.neuron);
    let view = RoomVinesView {
        placed: &placed,
        neurons,
        stems: &[],
    };
    assert_eq!(view.kind(vine), VineKind::CoralNeuron);
    assert_eq!(view.total_positions(vine), 5);
    assert_eq!(view.rad(vine, 3), 2.0);
    assert_eq!(view.mass(vine, 3), 0.5);
    assert!(view.currently_climbable(vine));
    let (pos, vel) = (
        made.neuron.geometry.segments[2][0],
        made.neuron.geometry.segments[2][2],
    );
    made.neuron.push(2, Vec2::new(1.0, -2.0));
    assert_eq!(
        made.neuron.geometry.segments[2][0],
        pos + Vec2::new(1.0, -2.0)
    );
    assert_eq!(
        made.neuron.geometry.segments[2][2],
        vel + Vec2::new(1.0, -2.0)
    );
}

/// `Clamp(length / 20, 1, 200)` segments; fewer than two cannot make the strand array
/// (`new Mycelium[segments - 2, 2]` throws).
#[test]
fn the_segment_count_is_clamped() {
    let r = big_room();
    let long = Made::new(
        &r,
        9000.0,
        (Some(Vec2::new(300.0, 200.0)), Some(Vec2::new(400.0, 200.0))),
        false,
        &mut UnityRandom::new(4),
    );
    assert_eq!(long.neuron.geometry.segments.len(), 200);
    assert_eq!(long.neuron.geometry.pairs, 20);
    let mut pool = MyceliumPool::default();
    let system = pool.create_registry();
    let mut seq = 0;
    let mut cx = NeuronCx {
        room: &r,
        system,
        pool: &mut pool,
        projector: false,
        circles: Vec::new(),
        new_vine_system: false,
        seq: &mut seq,
    };
    assert!(CoralNeuron::build(39.0, None, None, &mut cx, &mut UnityRandom::new(1)).is_none());
}

/// `SegmentOfMycelium`: the strand pairs sit where `MycLengthContour` is built around:
/// both ends rooted centres them; one rooted end packs them toward the other; free ends
/// split them between the two halves.
#[test]
fn strands_hang_from_the_segments_by_the_ends() {
    use super::NeuronGeometry;
    let mut g = NeuronGeometry {
        segments: vec![[Vec2::ZERO; 3]; 30],
        root_dir_a: Some(Vec2::new(0.0, 1.0)),
        root_dir_b: Some(Vec2::new(0.0, 1.0)),
        pairs: 20,
    };
    // 30 / 2 - 20 / 2 + 3 = 8
    assert_eq!(g.segment_of_mycelium(6), 8);
    g.root_dir_b = None;
    // 30 - 1 - 20 + 3
    assert_eq!(g.segment_of_mycelium(7), 12);
    g.root_dir_a = None;
    g.root_dir_b = Some(Vec2::new(0.0, 1.0));
    assert_eq!(g.segment_of_mycelium(6), 4);
    g.root_dir_b = None;
    // Pair 3 is in the first half (3 < 10): segment 4; pair 12: 30 - 1 - 20 + 12 = 21.
    assert_eq!(g.segment_of_mycelium(6), 4);
    assert_eq!(g.segment_of_mycelium(24), 21);
    // A short neuron (segments - 2 <= pairs): one pair a segment from the second.
    g.segments.truncate(12);
    g.pairs = 10;
    assert_eq!(g.segment_of_mycelium(7), 4);
    // The contour of free ends: 1 at either end, 0 from 10 pairs in.
    g.pairs = 30;
    g.segments = vec![[Vec2::ZERO; 3]; 32];
    assert_eq!(g.myc_length_contour(0), 1.0);
    assert_eq!(g.myc_length_contour(10), 0.0);
    assert_eq!(g.myc_length_contour(29), 1.0);
}

/// The player's `Push(index, movement)` through the room's host lands on the live neuron
/// (the one place it is stored).
#[test]
fn a_push_through_the_host_moves_the_stored_neuron() {
    let r = big_room();
    let ends = (Some(Vec2::new(300.0, 200.0)), Some(Vec2::new(400.0, 200.0)));
    let mut made = Made::new(&r, 100.0, ends, false, &mut UnityRandom::new(4));
    let order = made.neuron.update_order;
    let was = made.neuron.geometry.segments[3];
    let mut placed = PlacedVines::default();
    let mut host = RoomVines {
        placed: &mut placed,
        neurons: std::slice::from_mut(&mut made.neuron),
        stems: &mut [],
    };
    host.push(
        VineRef::CoralNeuron { room: 0, order },
        3,
        Vec2::new(0.5, 1.5),
    );
    assert_eq!(
        host.pos(VineRef::CoralNeuron { room: 0, order }, 3),
        was[0] + Vec2::new(0.5, 1.5)
    );
    assert_eq!(
        made.neuron.geometry.segments[3][2],
        was[2] + Vec2::new(0.5, 1.5)
    );
}
