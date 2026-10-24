//! The water's flood level: `fWaterLevel` moves the surface through `TargetWaterLevel`
//! (each point's `defaultPos.y` in `Surface.Update`), `lastFWaterLevel` is set at the end
//! of `Water.Update`, `DrawSprites` lerps between the two, and `GeneralUpsetSurface`
//! kicks one random point (three draws). Expected values from `Water.cs` (`Update`,
//! `DefaultSurface.TargetWaterLevel`, `Surface.GeneralUpsetSurface`, `DrawSprites`).
//!
//! Mirrors: - (port-only: tests)

use rw_assets::{Room, RoomSettings};
use rw_random::UnityRandom;

use crate::tests::room_with_water;
use crate::{BodyChunk, FloatExt, Sounds, Vec2, Water};

/// 20 x 10 open tiles, water row 3: `originalWaterLevel` 70, 43 surface points.
fn pool() -> (Room, Water) {
    let row = ".".repeat(20);
    let room = room_with_water(&[row.as_str(); 10], 3);
    let water = Water::new(&room, &RoomSettings::ANCESTOR).expect("water");
    (room, water)
}

fn step(room: &Room, w: &mut Water, seed: i32) {
    w.update(
        room,
        std::iter::empty(),
        &mut UnityRandom::new(seed),
        &mut Sounds::default(),
    );
}

/// The constructor: `fWaterLevel = originalWaterLevel`; `lastFWaterLevel` is 0 until
/// the first `Update`, which sets it to `fWaterLevel`.
#[test]
fn levels_start_at_the_original_one() {
    let (room, mut w) = pool();
    assert_eq!(w.original_level(), 70.0);
    assert_eq!(w.f_water_level, 70.0);
    assert_eq!(w.last_f_water_level(), 0.0);
    w.f_water_level = 90.0;
    step(&room, &mut w, 3);
    assert_eq!(w.last_f_water_level(), 90.0);
    assert_eq!(w.original_level(), 70.0);
}

/// In an open room the surface's motion does not depend on its height, so a level
/// raised by 200 px lifts every point (and `FloatWaterLevel`) by 200 after one
/// `Update`, and a chunk at y 150 goes from dry to fully under.
#[test]
fn the_surface_follows_a_raised_level() {
    let (room, mut still) = pool();
    let mut flooded = still.clone();
    flooded.f_water_level = 270.0;
    let chunk = BodyChunk::new(Vec2::new(100.0, 150.0), 9.0, 0.7);
    assert_eq!(flooded.submersion(&chunk), 0.0);
    step(&room, &mut still, 7);
    step(&room, &mut flooded, 7);
    for i in 0..still.point_count() {
        let (a, b) = (still.point(i), flooded.point(i));
        assert_eq!(a.x, b.x);
        assert!((b.y - a.y - 200.0).abs() < 1e-3, "point {i}: {a:?} {b:?}");
    }
    assert!((flooded.level_at(133.0) - still.level_at(133.0) - 200.0).abs() < 1e-3);
    assert_eq!(still.submersion(&chunk), 0.0);
    assert_eq!(flooded.submersion(&chunk), 1.0);
}

/// `DrawSprites`' level: `Lerp(lastFWaterLevel, fWaterLevel, timeStacker)`.
#[test]
fn the_drawn_level_lerps_from_last_tick() {
    let (room, mut w) = pool();
    step(&room, &mut w, 1);
    w.f_water_level = 80.0;
    assert_eq!(w.draw_level(0.0), 70.0);
    assert_eq!(w.draw_level(0.25), 72.5);
    assert_eq!(w.draw_level(1.0), 80.0);
}

/// `GeneralUpsetSurface(intensity)`: `Range(0, totalPoints)` picks a front point, its
/// `pos` moves `DegToVec(value * 360) * 2 * intensity` and its height by
/// `Lerp(-1, 1, value) * 4 * intensity`; exactly three draws.
#[test]
fn general_upset_surface_kicks_one_point_with_three_draws() {
    let (_, mut w) = pool();
    let before: Vec<(Vec2, f32)> = (0..w.point_count())
        .map(|i| (w.point(i), w.wave_height(i)))
        .collect();
    let intensity = 2.5;
    let mut rng = UnityRandom::new(11);
    w.general_upset_surface(intensity, &mut rng);
    let mut replay = UnityRandom::new(11);
    let i = replay.range_i32(0, 43) as usize;
    let moved = Vec2::from_degrees(replay.value() * 360.0) * (2.0 * intensity);
    let height = (-1.0_f32).lerp(1.0, replay.value()) * 4.0 * intensity;
    assert_eq!(rng.value(), replay.value(), "three draws");
    for (j, &(pos, h)) in before.iter().enumerate() {
        if j == i {
            assert_eq!(w.point(j), pos + moved);
            assert_eq!(w.wave_height(j), h + height);
        } else {
            assert_eq!((w.point(j), w.wave_height(j)), (pos, h));
        }
    }
}
