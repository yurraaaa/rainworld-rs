//! `Mycelium` and its pool: the constructor and `Reset` (the overseer hairs' values from
//! before they moved here), `Phase1`'s wall push, `Update`'s culling and pairing with their
//! draws. Expected values from `Mycelium.cs`, read for the formulas, and the draw counts
//! it makes.

use rw_random::UnityRandom;

use super::{
    MyceliaConnection, MyceliumEnv, MyceliumId, MyceliumPool, OwnerKey, PointOwner, SystemId,
    SystemState,
};
use crate::tests::room;
use crate::wall_mycelia::WallRoots;
use crate::world::AiMap;
use crate::{CameraView, FloatExt, Vec2};

/// The first system a pool makes.
const SYS: SystemId = SystemId(1);

fn draw(rng: &mut UnityRandom, n: usize) {
    for _ in 0..n {
        rng.value();
    }
}

fn camera() -> CameraView {
    CameraView {
        room: 0,
        current: 0,
        next: 0,
    }
}

/// The points an overseer's hair of length 60 at (100, 100) had before the strands moved
/// to the pool: the constructor from seed 5 and ten `Phase1`s without walls.
#[test]
fn an_overseers_hair_keeps_its_old_values() {
    let mut rng = UnityRandom::new(5);
    let mut pool = MyceliumPool::default();
    let owner = pool.new_owner();
    let at = Vec2::new(100.0, 100.0);
    let id = pool.add(None, (owner, &PointOwner(at)), 0, 60.0, at, &mut rng);
    let m = pool.get(id).unwrap();
    assert_eq!((m.points.len(), m.con_rad), (4, 15.0));
    assert_eq!(
        rng.state(),
        [1_925_640_999, 2_768_227_897, 3_537_265_001, 3_684_233_659]
    );
    assert_eq!(
        m.points[0],
        [
            Vec2::new(100.92307, 100.38462),
            Vec2::new(100.92307, 100.38462),
            Vec2::new(0.51617384, 0.8564839)
        ]
    );
    assert_eq!(
        m.points[3],
        [
            Vec2::new(68.76264, 80.17837),
            Vec2::new(68.76264, 80.17837),
            Vec2::new(0.9406188, 0.3394646)
        ]
    );
    for _ in 0..10 {
        pool.get_mut(id).unwrap().step(None);
    }
    let m = pool.get(id).unwrap();
    assert_eq!(
        m.points[1],
        [
            Vec2::new(93.78368, 95.215866),
            Vec2::new(93.27074, 94.45576),
            Vec2::new(0.69030535, 0.8503526)
        ]
    );
    assert_eq!(
        m.points[3],
        [
            Vec2::new(68.17843, 82.10211),
            Vec2::new(68.3056, 82.28022),
            Vec2::new(-0.30481327, -0.2690568)
        ]
    );
}

/// A second hair, `Reset` somewhere else and stepped seven times.
#[test]
fn a_reset_hair_keeps_its_old_values() {
    let mut rng = UnityRandom::new(5);
    let mut pool = MyceliumPool::default();
    let owner = pool.new_owner();
    let first = pool.add(
        None,
        (owner, &PointOwner(Vec2::new(100.0, 100.0))),
        0,
        60.0,
        Vec2::new(100.0, 100.0),
        &mut rng,
    );
    assert!(pool.get(first).is_some());
    let start = Vec2::new(300.0, 50.0);
    let id = pool.add(None, (owner, &PointOwner(start)), 1, 150.0, start, &mut rng);
    pool.reset(id, Vec2::new(20.0, 20.0), &PointOwner(Vec2::ZERO), &mut rng);
    for _ in 0..7 {
        pool.get_mut(id).unwrap().step(None);
    }
    let m = pool.get(id).unwrap();
    assert_eq!(m.points.len(), 10);
    assert_eq!(
        m.points[3],
        [
            Vec2::new(-17.679081, 29.2549),
            Vec2::new(-17.314583, 29.729729),
            Vec2::new(-0.36991346, -0.50405854)
        ]
    );
    assert_eq!(
        m.points[9],
        [
            Vec2::new(-69.62592, 45.657734),
            Vec2::new(-69.50638, 46.461216),
            Vec2::new(-0.3161565, -0.76792854)
        ]
    );
    assert_eq!(
        rng.state(),
        [549_394_754, 2_456_688_770, 2_232_727_165, 2_017_697_082]
    );
}

/// `IntClamp((int)(max(length, Lerp(length, 40, 0.5)) / 15), 2, 20)` points, `conRad =
/// length / points`.
#[test]
fn the_point_count_follows_the_length() {
    let mut rng = UnityRandom::new(1);
    let mut pool = MyceliumPool::default();
    let owner = pool.new_owner();
    // Length 10: `max(10, 25) / 15` is 1, raised to 2; 60: 60 / 15 = 4; 400: 26, capped.
    for (length, points) in [(10.0, 2), (30.0, 2), (60.0, 4), (400.0, 20)] {
        let id = pool.add(
            None,
            (owner, &PointOwner(Vec2::ZERO)),
            0,
            length,
            Vec2::ZERO,
            &mut rng,
        );
        let m = pool.get(id).unwrap();
        assert_eq!(m.points.len(), points);
        assert_eq!(m.con_rad, length / points as f32);
    }
}

/// `Reset` draws `ResetDir` (one for an overseer, none for a wall strand), then `cA` and
/// `cB`'s `RNV` (one each), then two a point: the constructor makes them once.
#[test]
fn construction_draws() {
    let roots = WallRoots {
        root_positions: vec![Vec2::new(40.0, 40.0)],
        directions: vec![Vec2::new(0.0, 1.0)],
    };
    for (length, points) in [(20.0, 2), (60.0, 4), (150.0, 10)] {
        let mut pool = MyceliumPool::default();
        let owner = pool.new_owner();
        let mut rng = UnityRandom::new(9);
        let mut expect = UnityRandom::new(9);
        pool.add(
            None,
            (owner, &PointOwner(Vec2::ZERO)),
            0,
            length,
            Vec2::ZERO,
            &mut rng,
        );
        draw(&mut expect, 3 + 2 * points);
        assert_eq!(rng.state(), expect.state());
        pool.add(
            None,
            (owner, &roots),
            0,
            length,
            roots.root_positions[0],
            &mut rng,
        );
        draw(&mut expect, 2 + 2 * points);
        assert_eq!(rng.state(), expect.state());
    }
}

/// A wall strand's `Reset` reaches `0.6 * length` along its direction first.
#[test]
fn a_wall_strand_reaches_along_its_direction() {
    let roots = WallRoots {
        root_positions: vec![Vec2::new(40.0, 40.0)],
        directions: vec![Vec2::new(0.0, 1.0)],
    };
    let mut pool = MyceliumPool::default();
    let owner = pool.new_owner();
    let id = pool.add(
        None,
        (owner, &roots),
        0,
        100.0,
        roots.root_positions[0],
        &mut UnityRandom::new(3),
    );
    let m = pool.get(id).unwrap();
    // The curve ends at the root plus 60 along its direction, each point jittered by one
    // unit vector.
    let tip = m.tip();
    assert!(
        tip.distance(Vec2::new(40.0, 100.0)) <= 1.0 + 1e-3,
        "{tip:?}"
    );
    let base = m.base();
    assert!(
        base.distance(Vec2::new(40.0, 40.0)) <= 1.0 + 1e-3,
        "{base:?}"
    );
}

fn walled() -> rw_assets::Room {
    // 7 x 9 tiles, solid border: a vertical corridor five tiles wide.
    let rows = [
        "#######", "#.....#", "#.....#", "#.....#", "#.....#", "#.....#", "#.....#", "#.....#",
        "#######",
    ];
    room(&rows)
}

/// `Phase1` pushes a strand's point off the walls when the terrain proximity of its tile
/// is below 4: along the sum of the four directions weighted by the proximities around
/// each, as strong as `LerpMap(proximity, 0, 3, 2, 0.2)`. Next to the left wall of a
/// room mirrored top to bottom, the push is straight right.
#[test]
fn a_strand_is_pushed_off_a_wall() {
    let r = walled();
    let map = AiMap::new(&r, 0, false, false);
    let proximity = map.terrain_proximity(1, 4);
    assert!((1..4).contains(&proximity), "{proximity}");
    let mut rng = UnityRandom::new(2);
    let mut pool = MyceliumPool::default();
    let owner = pool.new_owner();
    let id = pool.add(
        None,
        (owner, &PointOwner(Vec2::ZERO)),
        0,
        20.0,
        Vec2::ZERO,
        &mut rng,
    );
    {
        let m = pool.get_mut(id).unwrap();
        assert_eq!(m.points.len(), 2);
        // The first point inside tile (1, 4); the second a connection radius to its right,
        // so the pull between them moves nothing.
        let at = Vec2::middle_of_tile((1, 4));
        m.con_rad = 20.0;
        m.points[0] = [at, at, Vec2::ZERO];
        let far = Vec2::middle_of_tile((4, 4));
        m.points[1] = [
            Vec2::new(at.x + 20.0, at.y),
            Vec2::new(at.x + 20.0, at.y),
            Vec2::ZERO,
        ];
        assert!(m.points[1][0].x < far.x);
        m.step(Some(&map));
    }
    let want = (proximity as f32).lerp_map(0.0, 3.0, 2.0, 0.2);
    let v = pool.get(id).unwrap().points[0][2];
    assert!((v.x - want).abs() < 1e-5, "{v:?} {want}");
    assert!(v.y.abs() < 1e-5, "{v:?}");
    // Not at all with `moveAwayFromWalls` off, as an overseer's hair.
    let m = pool.get_mut(id).unwrap();
    m.move_away_from_walls = false;
    let at = Vec2::middle_of_tile((1, 4));
    m.points[0] = [at, at, Vec2::ZERO];
    m.points[1] = [
        at + Vec2::new(20.0, 0.0),
        at + Vec2::new(20.0, 0.0),
        Vec2::ZERO,
    ];
    m.step(Some(&map));
    assert_eq!(m.points[0][2], Vec2::ZERO);
}

/// A tile four or more away from every wall adds nothing.
#[test]
fn a_strand_far_from_the_walls_is_not_pushed() {
    let rows: Vec<String> = (0..13)
        .map(|y| {
            if y == 0 || y == 12 {
                "#".repeat(13)
            } else {
                format!("#{}#", ".".repeat(11))
            }
        })
        .collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let r = room(&refs);
    let map = AiMap::new(&r, 0, false, false);
    assert!(map.terrain_proximity(6, 6) >= 4);
    let mut pool = MyceliumPool::default();
    let owner = pool.new_owner();
    let mut rng = UnityRandom::new(2);
    let id = pool.add(
        None,
        (owner, &PointOwner(Vec2::ZERO)),
        0,
        20.0,
        Vec2::ZERO,
        &mut rng,
    );
    let m = pool.get_mut(id).unwrap();
    let at = Vec2::middle_of_tile((6, 6));
    m.con_rad = 20.0;
    m.points[0] = [at, at, Vec2::ZERO];
    m.points[1] = [
        at + Vec2::new(20.0, 0.0),
        at + Vec2::new(20.0, 0.0),
        Vec2::ZERO,
    ];
    m.step(Some(&map));
    assert_eq!(m.points[0][2], Vec2::ZERO);
}

/// A strand in a system: made in a registry; owned by `owner`; at (30, 70) in a room big
/// enough for the camera to see it.
fn strand_pool(owner: OwnerKey, base: Vec2, length: f32) -> (MyceliumPool, MyceliumId) {
    let mut pool = MyceliumPool::default();
    pool.create_registry();
    let roots = WallRoots {
        root_positions: vec![base],
        directions: vec![Vec2::new(1.0, 0.0)],
    };
    let id = pool.add(
        Some(SYS),
        (owner, &roots),
        0,
        length,
        base,
        &mut UnityRandom::new(1),
    );
    (pool, id)
}

fn env<'a>(
    r: &'a rw_assets::Room,
    map: Option<&'a AiMap>,
    tick: i32,
    frozen: Option<bool>,
) -> MyceliumEnv<'a> {
    MyceliumEnv {
        room: r,
        room_index: 0,
        ai_map: map,
        camera: camera(),
        cameras_changed_tick: tick,
        system: frozen.map(|frozen| SystemState { id: SYS, frozen }),
    }
}

/// With static culling a strand is culled until a camera has been asked whether it sees
/// it (the first update asks), then comes back with a `Reset` (`2 + 2 * points` draws for a
/// wall strand) and, with a system and nothing resting, one `Range` draw to pick a partner.
#[test]
fn a_strand_waits_for_the_camera_then_resets_and_looks_for_a_partner() {
    let r = walled();
    let map = AiMap::new(&r, 0, false, false);
    let base = Vec2::new(70.0, 70.0);
    let (mut pool, id) = strand_pool(OwnerKey(7), base, 60.0);
    let roots = WallRoots {
        root_positions: vec![base],
        directions: vec![Vec2::new(1.0, 0.0)],
    };
    let mut rng = UnityRandom::new(4);
    let mut expect = UnityRandom::new(4);
    let e = env(&r, Some(&map), 3, Some(false));
    pool.update(id, &roots, &e, &mut rng);
    assert!(pool.get(id).unwrap().culled);
    assert!(pool.get(id).unwrap().viewed_by_camera);
    assert_eq!(rng.state(), expect.state());
    pool.update(id, &roots, &e, &mut rng);
    assert!(!pool.get(id).unwrap().culled);
    // `Reset` (2 + 2 * 4), then the single registry entry is the strand itself.
    draw(&mut expect, 2 + 2 * 4 + 1);
    assert_eq!(rng.state(), expect.state());
    // The base is held at the owner's root, its velocity cleared.
    assert_eq!(pool.get(id).unwrap().base(), base);
    assert_eq!(pool.get(id).unwrap().points[0][2], Vec2::ZERO);
    // From then on one draw a tick: `Range(0, 1)`.
    pool.update(id, &roots, &e, &mut rng);
    draw(&mut expect, 1);
    assert_eq!(rng.state(), expect.state());
}

/// A frozen system culls its strands (an overseer's, which do not cull by camera, too):
/// no draws, no movement.
#[test]
fn a_frozen_system_culls_the_strands() {
    let r = walled();
    let map = AiMap::new(&r, 0, false, false);
    let base = Vec2::new(70.0, 70.0);
    let (mut pool, id) = strand_pool(OwnerKey(7), base, 60.0);
    let roots = WallRoots {
        root_positions: vec![base],
        directions: vec![Vec2::new(1.0, 0.0)],
    };
    let mut rng = UnityRandom::new(4);
    let expect = UnityRandom::new(4);
    {
        let m = pool.get_mut(id).unwrap();
        m.use_static_culling = false;
    }
    let before = pool.get(id).unwrap().points.clone();
    let frozen = env(&r, Some(&map), 3, Some(true));
    pool.update(id, &roots, &frozen, &mut rng);
    assert!(pool.get(id).unwrap().culled);
    assert_eq!(pool.get(id).unwrap().points, before);
    assert_eq!(rng.state(), expect.state());
    // Thawed, it is not culled and resets at once (`lastCulled && !culled`).
    let thawed = env(&r, Some(&map), 3, Some(false));
    pool.update(id, &roots, &thawed, &mut rng);
    assert!(!pool.get(id).unwrap().culled);
    assert_ne!(rng.state(), expect.state());
}

/// Without an aimap nothing moves (`owner.OwnerRoom.aimap == null`), though the strand is
/// uncull-reset first.
#[test]
fn a_strand_does_not_move_without_an_aimap() {
    let r = walled();
    let base = Vec2::new(70.0, 70.0);
    let (mut pool, id) = strand_pool(OwnerKey(7), base, 60.0);
    pool.get_mut(id).unwrap().use_static_culling = false;
    let roots = WallRoots {
        root_positions: vec![base],
        directions: vec![Vec2::new(1.0, 0.0)],
    };
    let before = pool.get(id).unwrap().points.clone();
    let mut rng = UnityRandom::new(4);
    pool.update(id, &roots, &env(&r, None, 3, Some(false)), &mut rng);
    assert_eq!(pool.get(id).unwrap().points, before);
}

/// Two owners' strands in one system: the draw is `Range(0, Count)`; the strand pairs with
/// the pick when it is another owner's, free and its base within `(len + len) * 0.75`.
#[test]
fn an_unlinked_strand_pairs_with_a_near_strand_of_another_owner() {
    let r = walled();
    let map = AiMap::new(&r, 0, false, false);
    let base = Vec2::new(70.0, 70.0);
    let (mut pool, a) = strand_pool(OwnerKey(7), base, 60.0);
    let roots = WallRoots {
        root_positions: vec![base],
        directions: vec![Vec2::new(1.0, 0.0)],
    };
    let other_owner = pool.new_owner();
    let other = pool.add(
        Some(SYS),
        (other_owner, &PointOwner(base + Vec2::new(30.0, 0.0))),
        0,
        60.0,
        base + Vec2::new(30.0, 0.0),
        &mut UnityRandom::new(2),
    );
    // A seed whose first `Range(0, 2)` is the second registered strand (the other owner's).
    let seed = (0..200)
        .find(|&s| UnityRandom::new(s).range_i32(0, 2) == 1)
        .unwrap();
    {
        let m = pool.get_mut(a).unwrap();
        m.use_static_culling = false;
        m.viewed_by_camera = true;
    }
    let mut rng = UnityRandom::new(seed);
    pool.update(a, &roots, &env(&r, Some(&map), 3, Some(false)), &mut rng);
    let want = MyceliaConnection { a, b: other };
    assert_eq!(pool.get(a).unwrap().connection, Some(want));
    assert_eq!(pool.get(other).unwrap().connection, Some(want));
    assert_eq!(want.other(a), other);
    assert_eq!(want.other(other), a);
}

/// The pick is refused when it is the strand itself, its own owner's, taken, or far:
/// still one draw.
#[test]
fn pairing_is_refused_for_the_wrong_partner() {
    let r = walled();
    let map = AiMap::new(&r, 0, false, false);
    let base = Vec2::new(70.0, 70.0);
    let (mut pool, a) = strand_pool(OwnerKey(7), base, 60.0);
    let roots = WallRoots {
        root_positions: vec![base],
        directions: vec![Vec2::new(1.0, 0.0)],
    };
    // Same owner, near.
    let same = pool.add(
        Some(SYS),
        (OwnerKey(7), &PointOwner(base)),
        0,
        60.0,
        base + Vec2::new(10.0, 0.0),
        &mut UnityRandom::new(2),
    );
    // Another owner, far (the bases are 200 apart, more than `(60 + 60) * 0.75`).
    let far_owner = pool.new_owner();
    let far = pool.add(
        Some(SYS),
        (far_owner, &PointOwner(base)),
        0,
        60.0,
        base + Vec2::new(200.0, 0.0),
        &mut UnityRandom::new(2),
    );
    pool.get_mut(a).unwrap().use_static_culling = false;
    pool.get_mut(a).unwrap().viewed_by_camera = true;
    let e = env(&r, Some(&map), 3, Some(false));
    for seed in 0..40 {
        let mut rng = UnityRandom::new(seed);
        let mut expect = UnityRandom::new(seed);
        pool.update(a, &roots, &e, &mut rng);
        draw(&mut expect, 1);
        assert_eq!(rng.state(), expect.state());
        assert_eq!(pool.get(a).unwrap().connection, None, "seed {seed}");
        assert_eq!(pool.get(same).unwrap().connection, None);
        assert_eq!(pool.get(far).unwrap().connection, None);
    }
    // While it rests it does not even draw.
    pool.get_mut(a).unwrap().rest = 5;
    let mut rng = UnityRandom::new(1);
    pool.update(a, &roots, &e, &mut rng);
    assert_eq!(rng.state(), UnityRandom::new(1).state());
    assert_eq!(pool.get(a).unwrap().rest, 4);
}

fn linked(
    base_a: Vec2,
    base_b: Vec2,
    tip_a: Vec2,
    tip_b: Vec2,
) -> (MyceliumPool, MyceliumId, MyceliumId) {
    let (mut pool, a) = strand_pool(OwnerKey(7), base_a, 20.0);
    let b = pool.add(
        Some(SYS),
        (OwnerKey(8), &PointOwner(base_b)),
        0,
        20.0,
        base_b,
        &mut UnityRandom::new(2),
    );
    let connection = MyceliaConnection { a, b };
    for (id, base, tip) in [(a, base_a, tip_a), (b, base_b, tip_b)] {
        let m = pool.get_mut(id).unwrap();
        assert_eq!(m.points.len(), 2);
        m.connection = Some(connection);
        m.points[0] = [base, base, Vec2::ZERO];
        m.points[1] = [tip, tip, Vec2::new(2.0, 2.0)];
    }
    (pool, a, b)
}

/// A link whose other end let go breaks: no `value < 0.005` draw (the `||` stops first),
/// one `Range(20, 200)` for the rest.
#[test]
fn a_link_the_other_end_dropped_breaks_with_one_draw() {
    let (mut pool, a, b) = linked(
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(30.0, 0.0),
    );
    pool.get_mut(b).unwrap().connection = None;
    let connection = pool.get(a).unwrap().connection.unwrap();
    let mut rng = UnityRandom::new(6);
    pool.update_connected(a, connection, &mut rng);
    assert_eq!(pool.get(a).unwrap().connection, None);
    let rest = UnityRandom::new(6).range_i32(20, 200);
    assert_eq!(pool.get(a).unwrap().rest, rest);
    assert_eq!(rng.state(), {
        let mut e = UnityRandom::new(6);
        e.range_i32(20, 200);
        e.state()
    });
}

/// Bases farther apart than `A.length + B.length` break it the same way.
#[test]
fn a_stretched_link_breaks() {
    let (mut pool, a, _b) = linked(
        Vec2::new(0.0, 0.0),
        Vec2::new(100.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(90.0, 0.0),
    );
    let connection = pool.get(a).unwrap().connection.unwrap();
    let mut rng = UnityRandom::new(6);
    pool.update_connected(a, connection, &mut rng);
    assert_eq!(pool.get(a).unwrap().connection, None);
    assert!((20..200).contains(&pool.get(a).unwrap().rest));
}

/// A healthy link draws `value < 0.005` once; a seed whose first value is above it keeps
/// the link, and with tips 10 or more apart the free tip's velocity eases halfway to the
/// other tip's offset clamped to 5.
#[test]
fn a_link_with_tips_apart_pulls_the_tip() {
    let (mut pool, a, _b) = linked(
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(5.0, 0.0),
        Vec2::new(35.0, 0.0),
    );
    let seed = (0..100)
        .find(|&s| UnityRandom::new(s).value() >= 0.005)
        .unwrap();
    let connection = pool.get(a).unwrap().connection.unwrap();
    let mut rng = UnityRandom::new(seed);
    pool.update_connected(a, connection, &mut rng);
    assert_eq!(pool.get(a).unwrap().connection, Some(connection));
    let mut expect = UnityRandom::new(seed);
    draw(&mut expect, 1);
    assert_eq!(rng.state(), expect.state());
    // `Lerp((2, 2), ClampMagnitude((30, 0), 5) = (5, 0), 0.5)`.
    assert_eq!(pool.get(a).unwrap().points[1][2], Vec2::new(3.5, 1.0));
}

/// Tips under 10 apart are pushed together by `(distance - 1) / 2` each; `value < 0.05`
/// then makes a `NeuronSpark` at their middle (its constructor draws once).
#[test]
fn touching_tips_make_a_spark() {
    let (mut pool, a, b) = linked(
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(0.0, 0.0),
        Vec2::new(6.0, 0.0),
    );
    let seed = (0..2000)
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            r.value() >= 0.005 && r.value() < 0.05
        })
        .unwrap();
    let connection = pool.get(a).unwrap().connection.unwrap();
    let mut rng = UnityRandom::new(seed);
    pool.update_connected(a, connection, &mut rng);
    assert_eq!(pool.get(a).unwrap().tip(), Vec2::new(2.5, 0.0));
    assert_eq!(pool.get(b).unwrap().tip(), Vec2::new(3.5, 0.0));
    assert_eq!(pool.get(a).unwrap().points[1][2], Vec2::new(4.5, 2.0));
    assert_eq!(pool.get(b).unwrap().points[1][2], Vec2::new(-0.5, 2.0));
    let mut expect = UnityRandom::new(seed);
    draw(&mut expect, 3);
    assert_eq!(rng.state(), expect.state());
    assert_eq!(pool.sparks.len(), 1);
    assert_eq!(pool.sparks[0].pos, Vec2::new(3.0, 0.0));
    // Without the spark roll's success, two draws and no spark.
    pool.sparks.clear();
    let seed = (0..2000)
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            r.value() >= 0.005 && r.value() >= 0.05
        })
        .unwrap();
    let mut rng = UnityRandom::new(seed);
    pool.update_connected(a, connection, &mut rng);
    let mut expect = UnityRandom::new(seed);
    draw(&mut expect, 2);
    assert_eq!(rng.state(), expect.state());
    assert!(pool.sparks.is_empty());
}

/// A first draw below 0.005 breaks a sound link: that draw and `Range(20, 200)`.
#[test]
fn a_link_breaks_by_chance() {
    let (mut pool, a, _b) = linked(
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(5.0, 0.0),
        Vec2::new(35.0, 0.0),
    );
    let seed = (0..100_000)
        .find(|&s| UnityRandom::new(s).value() < 0.005)
        .unwrap();
    let connection = pool.get(a).unwrap().connection.unwrap();
    let mut rng = UnityRandom::new(seed);
    pool.update_connected(a, connection, &mut rng);
    assert_eq!(pool.get(a).unwrap().connection, None);
    let mut expect = UnityRandom::new(seed);
    expect.value();
    let rest = expect.range_i32(20, 200);
    assert_eq!(pool.get(a).unwrap().rest, rest);
    assert_eq!(rng.state(), expect.state());
}

/// `OverseerGraphics.UpdateNeuronSystemForMycelia`: a strand leaves the registry of its
/// old system and joins the new one's; one already in the new system stays where it is.
#[test]
fn a_strand_moves_between_systems() {
    let mut pool = MyceliumPool::default();
    let (first, second) = (pool.create_registry(), pool.create_registry());
    let owner = pool.new_owner();
    let id = pool.add(
        Some(first),
        (owner, &PointOwner(Vec2::ZERO)),
        0,
        20.0,
        Vec2::ZERO,
        &mut UnityRandom::new(1),
    );
    assert_eq!(pool.registry(first), [id]);
    pool.move_to_system(id, Some(second));
    assert_eq!(pool.registry(first), []);
    assert_eq!(pool.registry(second), [id]);
    assert_eq!(pool.get(id).unwrap().system, Some(second));
    pool.move_to_system(id, Some(second));
    assert_eq!(pool.registry(second), [id]);
    pool.move_to_system(id, None);
    assert_eq!(pool.registry(second), []);
    assert_eq!(pool.get(id).unwrap().system, None);
    // A removed owner's strands are gone, and links to them read as absent.
    pool.release_owner(owner);
    assert!(pool.get(id).is_none());
}

/// A strand of a gone owner lives on only while a live registry holds it: freed at once with
/// no system, or when its registry is dropped; its owner's others are untouched.
#[test]
fn a_gone_owners_strands_are_freed_once_nothing_reaches_them() {
    let mut pool = MyceliumPool::default();
    let sys = pool.create_registry();
    let (gone, kept) = (pool.new_owner(), pool.new_owner());
    let at = PointOwner(Vec2::ZERO);
    let rng = &mut UnityRandom::new(1);
    let free = pool.add(None, (gone, &at), 0, 20.0, Vec2::ZERO, rng);
    let listed = pool.add(Some(sys), (gone, &at), 0, 20.0, Vec2::ZERO, rng);
    let other = pool.add(None, (kept, &at), 0, 20.0, Vec2::ZERO, rng);
    pool.release_owner(gone);
    assert!(pool.get(free).is_none());
    assert!(pool.get(listed).is_some() && pool.get(other).is_some());
    assert_eq!(pool.registry(sys), [listed]);
    pool.drop_registry(sys);
    assert!(pool.get(listed).is_none() && pool.get(other).is_some());
    assert_eq!(pool.live(), 1);
}

/// A re-created system is another system: a strand still pointing at the old one is not
/// "in" the new one, and joins it.
#[test]
fn a_new_system_is_not_the_old_one() {
    let mut pool = MyceliumPool::default();
    let old = pool.create_registry();
    let owner = pool.new_owner();
    let at = PointOwner(Vec2::ZERO);
    let id = pool.add(
        Some(old),
        (owner, &at),
        0,
        20.0,
        Vec2::ZERO,
        &mut UnityRandom::new(1),
    );
    pool.drop_registry(old);
    let new = pool.create_registry();
    assert_ne!(old, new);
    assert!(pool.registry(old).is_empty());
    pool.move_to_system(id, Some(new));
    assert_eq!(pool.registry(new), [id]);
    assert_eq!(pool.get(id).unwrap().system, Some(new));
}

/// Two strands linked, the wall's owner gone with its room: the wall strand stays while its
/// live partner names it (the game's object is still alive and its `connection` still equal), so
/// the partner's turn draws only `value < 0.005` and keeps the link; once it breaks, the strand
/// is freed.
#[test]
fn a_linked_strand_outlives_its_owner_until_unlinked() {
    let (mut pool, wall, other) = linked(
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(5.0, 0.0),
        Vec2::new(35.0, 0.0),
    );
    let wall_owner = pool.get(wall).unwrap().owner;
    let system = pool.get(wall).unwrap().system.unwrap();
    pool.release_owner(wall_owner);
    pool.drop_registry(system);
    assert!(pool.get(wall).is_some(), "kept for its partner");
    let connection = pool.get(other).unwrap().connection.unwrap();
    let seed = (0..100)
        .find(|&s| UnityRandom::new(s).value() >= 0.005)
        .unwrap();
    let mut rng = UnityRandom::new(seed);
    pool.update_connected(other, connection, &mut rng);
    assert_eq!(pool.get(other).unwrap().connection, Some(connection));
    let mut expect = UnityRandom::new(seed);
    draw(&mut expect, 1);
    assert_eq!(rng.state(), expect.state());
    // The partner lets go by chance: the wall strand has nothing left to reach it.
    let breaking = (0..100_000)
        .find(|&s| UnityRandom::new(s).value() < 0.005)
        .unwrap();
    pool.update_connected(other, connection, &mut UnityRandom::new(breaking));
    assert_eq!(pool.get(other).unwrap().connection, None);
    assert!(pool.get(wall).is_none());
}

/// A dropped system keeps its last `Frozen` for the strands still pointing at it, a strand
/// of a teleporting overseer in another room's update.
#[test]
fn a_dropped_systems_frozen_is_kept_for_its_strands() {
    let r = walled();
    let map = AiMap::new(&r, 0, false, false);
    let base = Vec2::new(70.0, 70.0);
    let (mut pool, id) = strand_pool(OwnerKey(7), base, 60.0);
    pool.get_mut(id).unwrap().use_static_culling = false;
    let roots = WallRoots {
        root_positions: vec![base],
        directions: vec![Vec2::new(1.0, 0.0)],
    };
    pool.update(
        id,
        &roots,
        &env(&r, Some(&map), 3, Some(true)),
        &mut UnityRandom::new(1),
    );
    assert!(pool.get(id).unwrap().culled);
    pool.drop_registry(SYS);
    let elsewhere = env(&r, Some(&map), 3, None);
    pool.update(id, &roots, &elsewhere, &mut UnityRandom::new(1));
    assert!(
        pool.get(id).unwrap().culled,
        "still frozen as the dead system was"
    );
    // Once nothing points at it the state is forgotten.
    pool.release_owner(OwnerKey(7));
    assert!(pool.get(id).is_none());
}
