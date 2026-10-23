//! `WallMycelia` against `WallMycelia.cs`: the seeded constructor's strands (which tiles
//! make them, in which order the draws go), the shared stream left alone, the per-tick
//! draws and the bend of the first points.

use rw_random::UnityRandom;

use super::{WallMycelia, WallPlace};
use crate::mycelium::{MyceliumEnv, MyceliumPool, SystemId, SystemState};
use crate::shared_physics::ExactRayTrace;
use crate::tests::room;
use crate::world::AiMap;
use crate::{CameraView, FloatExt, Vec2};

fn corridor() -> rw_assets::Room {
    // The free row is tile row 1 (y 20 to 40): tiles 1 to 3 of five.
    room(&["#####", "#...#", "#####"])
}

fn place() -> WallPlace {
    WallPlace {
        pos: Vec2::new(50.0, 30.0),
        rad: 100.0,
    }
}

/// What the constructor makes in [`corridor`] with [`place`], replayed from the seed
/// `(int)50 + (int)30`: tile 2 has no open side opposite a wall (so no strands and no
/// draws); tiles 1 and 3 each draw `Lerp(1, 4, v)` and `v` for their strand count (the
/// distance, 20, is under a third of the radius, so the first), then per strand the two
/// offsets, and the length when the ray from the point meets a wall.
fn expected_strands(r: &rw_assets::Room) -> Vec<(Vec2, Vec2, f32)> {
    let p = place();
    let mut seeded = UnityRandom::new(80);
    let reach = 20.0 + 160.0 * (50.0f32 / 450.0).sqrt();
    let mut out = Vec::new();
    for (tile, side) in [(1, 1.0), (3, -1.0)] {
        let from_b = 1.0f32.lerp(4.0, seeded.value());
        seeded.value();
        let mut vector = Vec2::new(side, 0.0);
        let middle = Vec2::middle_of_tile((tile, 1));
        for _ in 0..from_b.to_int_cs() {
            let dx = (-9.0f32).lerp(9.0, seeded.value());
            let dy = (-9.0f32).lerp(9.0, seeded.value());
            let at = middle + Vec2::new(dx, dy);
            vector = (vector + p.pos.direction_to(at)).normalized();
            if let Some(root) = r.exact_terrain_ray_trace_pos(at, at - vector * 20.0) {
                let from_b = 20.0 + reach * seeded.value().powf(0.25);
                let length = from_b.lerp(20.0, root.distance(p.pos) / 100.0);
                out.push((root, vector, length));
            }
        }
    }
    out
}

/// The constructor's strands: their roots, directions and lengths in the order the draws
/// make them, the shared stream untouched, each strand registered in the room's system.
#[test]
fn the_constructor_follows_its_seed() {
    let r = corridor();
    let mut rng = UnityRandom::new(11);
    let before = rng.state();
    let mut pool = MyceliumPool::default();
    let sys = pool.create_registry();
    let wall = WallMycelia::new(1, vec![place()], &r, sys, &mut pool, &mut rng);
    assert_eq!(rng.state(), before);
    let want = expected_strands(&r);
    assert!(want.len() >= 3, "{}", want.len());
    assert_eq!(wall.mycelia.len(), want.len());
    assert_eq!(wall.roots.root_positions.len(), want.len());
    assert_eq!(wall.random_movements.len(), want.len());
    assert_eq!(pool.registry(sys).len(), want.len());
    for (i, &id) in wall.mycelia.iter().enumerate() {
        assert_eq!(wall.roots.root_positions[i], want[i].0);
        assert_eq!(wall.roots.directions[i], want[i].1);
        let m = pool.get(id).unwrap();
        assert!(
            (m.length - want[i].2).abs() < 1e-2,
            "{} {}",
            m.length,
            want[i].2
        );
        assert_eq!((m.system, m.index, m.owner), (Some(sys), i, wall.owner));
        // A root is on a wall's edge: the floor (y 20) or ceiling (y 40) of the corridor.
        let y = wall.roots.root_positions[i].y;
        assert!((y - 20.0).abs() < 1e-3 || (y - 40.0).abs() < 1e-3, "{y}");
    }
}

/// A place whose radius reaches no free tile makes no strands; a place's tiles outside the
/// room or Solid are skipped.
#[test]
fn a_place_among_walls_makes_nothing() {
    let r = room(&["#####", "#####", "#####"]);
    let mut pool = MyceliumPool::default();
    let sys = pool.create_registry();
    let wall = WallMycelia::new(
        1,
        vec![place()],
        &r,
        sys,
        &mut pool,
        &mut UnityRandom::new(1),
    );
    assert!(wall.mycelia.is_empty());
}

fn env<'a>(r: &'a rw_assets::Room, map: &'a AiMap, tick: i32) -> MyceliumEnv<'a> {
    MyceliumEnv {
        room: r,
        room_index: 0,
        ai_map: Some(map),
        camera: CameraView {
            room: 0,
            current: 0,
            next: 0,
        },
        cameras_changed_tick: tick,
        system: Some(SystemState {
            id: SystemId(1),
            frozen: false,
        }),
    }
}

/// While its strands are culled each tick draws one `RNV` a strand, for the strand's own
/// drift: `randomMovements = ClampMagnitude(randomMovements + RNV * 0.1, 1)`.
#[test]
fn culled_strands_draw_one_each() {
    let r = corridor();
    let map = AiMap::new(&r, 0, false, false);
    let mut pool = MyceliumPool::default();
    let sys = pool.create_registry();
    let mut wall = WallMycelia::new(
        1,
        vec![place()],
        &r,
        sys,
        &mut pool,
        &mut UnityRandom::new(1),
    );
    let mut rng = UnityRandom::new(5);
    let mut expect = UnityRandom::new(5);
    wall.update(&mut pool, &env(&r, &map, 4), &mut rng);
    let mut moves = Vec::new();
    for _ in &wall.mycelia {
        moves.push((Vec2::random_unit(&mut expect) * 0.1).clamp_magnitude(1.0));
    }
    assert_eq!(rng.state(), expect.state());
    assert_eq!(wall.random_movements, moves);
    assert!(wall.mycelia.iter().all(|&id| pool.get(id).unwrap().culled));
}

/// An unculled strand adds `(direction + randomMovement) * (sin(num pi) * 0.75)` to the
/// velocity of its points 1 to `min(5, points) - 1`, `num` running 0 to 1 along them.
#[test]
fn the_first_points_bend_along_the_direction() {
    let r = corridor();
    let map = AiMap::new(&r, 0, false, false);
    let mut pool = MyceliumPool::default();
    let sys = pool.create_registry();
    let mut wall = WallMycelia::new(
        1,
        vec![place()],
        &r,
        sys,
        &mut pool,
        &mut UnityRandom::new(1),
    );
    let id = wall.mycelia[0];
    // Four points a connection radius apart on a line from the root, at rest.
    let root = wall.roots.root_positions[0];
    {
        let m = pool.get_mut(id).unwrap();
        m.points.truncate(4);
        while m.points.len() < 4 {
            m.points.push([Vec2::ZERO; 3]);
        }
        m.con_rad = 15.0;
        m.move_away_from_walls = false;
        for (k, p) in m.points.iter_mut().enumerate() {
            let at = root + Vec2::new(15.0 * k as f32, 0.0);
            *p = [at, at, Vec2::ZERO];
        }
        m.viewed_by_camera = true;
        m.last_camera_cull_tick = 4;
        m.use_static_culling = true;
    }
    // Only this strand: the others stay culled and rest.
    let mut rng = UnityRandom::new(9);
    let mut expect = UnityRandom::new(9);
    wall.update(&mut pool, &env(&r, &map, 4), &mut rng);
    // The strand's own draws first: the registry pick (`Range(0, Count)`), then its drift.
    let count = i32::try_from(pool.registry(sys).len()).unwrap();
    expect.range_i32(0, count);
    let drift = (Vec2::random_unit(&mut expect) * 0.1).clamp_magnitude(1.0);
    assert!(!pool.get(id).unwrap().culled);
    assert_eq!(wall.random_movements[0], drift);
    let m = pool.get(id).unwrap();
    // After `Phase1` the line's velocities are -0.2, -0.2, +0.2, +0.2 along x (the
    // straightening push); the base is held at its root, its velocity cleared.
    let dir = wall.roots.directions[0] + drift;
    let bend = (
        std::f32::consts::FRAC_PI_3.sin() * 0.75,
        std::f32::consts::FRAC_PI_3.sin() * 0.75,
    );
    assert_eq!(m.points[0][2], Vec2::ZERO);
    let want1 = Vec2::new(-0.2, 0.0) + dir * bend.0;
    let want2 = Vec2::new(0.2, 0.0) + dir * bend.1;
    // `sin(pi)` at the last point is nearly nothing.
    let want3 = Vec2::new(0.2, 0.0);
    for (got, want) in [
        (m.points[1][2], want1),
        (m.points[2][2], want2),
        (m.points[3][2], want3),
    ] {
        assert!(got.distance(want) < 1e-4, "{got:?} {want:?}");
    }
}
