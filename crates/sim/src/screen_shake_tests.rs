//! Camera shake. Expected values from `RoomCamera.cs` (`Update`, `ScreenMovement`,
//! `DistanceFromViewedScreen`), `Room.cs` (`ScreenMovement`, `Loaded`'s
//! `ceilingTiles`) and `PhysicalObject.TerrainImpact`.

use rw_random::UnityRandom;

use super::{CameraShake, ScreenMovement};
use crate::tests::{room, room_with_water};
use crate::world::RealizedRoom;
use crate::{BodyChunk, Contact, TerrainImpact, Vec2};

fn assert_close(a: Vec2, b: Vec2) {
    assert!(a.distance(b) < 1e-4, "{a:?} vs {b:?}");
}

fn floor_impact(speed: f32, first_contact: bool) -> TerrainImpact {
    TerrainImpact {
        chunk: 0,
        direction: Contact { x: 0, y: -1 },
        speed,
        first_contact,
    }
}

fn shaking(screen_shake: f32, micro_shake: f32) -> CameraShake {
    CameraShake {
        screen_shake,
        micro_shake,
        ..CameraShake::default()
    }
}

/// While `screenShake` is up: one draw, a jump of `8 * screenShake` at `value * 360`
/// degrees, then 2 -> 1.3 -> 1 (`Max(s - 0.7, 1)`) -> 0.975.
#[test]
fn the_jump_draws_once_and_decays() {
    let mut s = shaking(2.0, 0.0);
    let mut rng = UnityRandom::new(3);
    let mut expect = rng.clone();
    let jump = Vec2::from_degrees(expect.value() * 360.0) * 16.0;
    s.update(&mut rng);
    assert_eq!(rng, expect, "one draw");
    assert_close(s.take_displacement(), jump);
    assert_close(s.pos, jump);
    assert!((s.screen_shake - 1.3).abs() < 1e-6, "{}", s.screen_shake);
    s.update(&mut rng);
    assert_eq!(s.screen_shake, 1.0);
    s.update(&mut rng);
    assert!((s.screen_shake - 0.975).abs() < 1e-6);
    let mut still = shaking(0.0, 0.0);
    let before = rng.clone();
    still.update(&mut rng);
    assert_eq!(rng, before, "no draw without screenShake");
    assert_eq!(still.take_displacement(), Vec2::ZERO);
}

/// `microShake`: minus 0.025 a tick, then cut to 80 % while above 2.
#[test]
fn micro_shake_decays() {
    let mut s = shaking(0.0, 3.0);
    let mut rng = UnityRandom::new(1);
    s.update(&mut rng);
    assert!(
        (s.micro_shake - 2.975 * 0.8).abs() < 1e-6,
        "{}",
        s.micro_shake
    );
    let mut s = shaking(0.0, 0.01);
    s.update(&mut rng);
    assert_eq!(s.micro_shake, 0.0);
}

/// A movement without a source acts in full; one inside the window is divided by
/// `1 + Lerp(0, 1, 0.95)` = 1.95; one 100 px outside by `1 + Lerp(100, 1, 0.95)` = 6.95.
#[test]
fn screen_movement_weakens_with_distance() {
    let mut s = CameraShake::default();
    s.set_pos(Vec2::new(100.0, 50.0), 1366.0);
    s.screen_movement(ScreenMovement {
        source: None,
        bump: Vec2::new(4.0, 0.0),
        shake: 5.0,
    });
    assert_eq!(s.take_displacement(), Vec2::new(-4.0, 0.0));
    assert_eq!((s.screen_shake, s.micro_shake), (5.0, 2.0));
    assert_eq!(s.screen_movement_shake, 5.0);

    let mut s = CameraShake::default();
    s.set_pos(Vec2::new(100.0, 50.0), 1366.0);
    assert_eq!(s.distance_from_viewed_screen(Vec2::new(500.0, 500.0)), 0.0);
    s.screen_movement(ScreenMovement {
        source: Some(Vec2::new(500.0, 500.0)),
        bump: Vec2::new(0.0, -3.9),
        shake: 0.39,
    });
    assert_close(s.take_displacement(), Vec2::new(0.0, 2.0));
    assert!((s.screen_shake - 0.2).abs() < 1e-6);
    assert_eq!(s.micro_shake, 0.0, "only a shake above 3 raises it");

    // 100 px right of the window's right edge (100 + 1366).
    let mut s = CameraShake::default();
    s.set_pos(Vec2::new(100.0, 50.0), 1366.0);
    let far = Vec2::new(1566.0, 300.0);
    assert!((s.distance_from_viewed_screen(far) - 100.0).abs() < 1e-3);
    s.screen_movement(ScreenMovement {
        source: Some(far),
        bump: Vec2::new(6.95, 0.0),
        shake: 0.695,
    });
    assert_close(s.take_displacement(), Vec2::new(-1.0, 0.0));
    assert!((s.screen_shake - 0.1).abs() < 1e-5);
    // A weaker shake does not lower it.
    s.screen_movement(ScreenMovement {
        source: None,
        bump: Vec2::ZERO,
        shake: 0.05,
    });
    assert!((s.screen_shake - 0.1).abs() < 1e-5);
}

/// `PhysicalObject.TerrainImpact`: above 7 (speed times mass) on a first contact, a
/// bump of a tenth of it toward the surface and a shake of `(f - 30) / 50`.
#[test]
fn terrain_impact_bumps_above_7() {
    let chunk = BodyChunk::new(Vec2::new(40.0, 30.0), 5.0, 0.7);
    assert_eq!(
        floor_impact(10.0, true).screen_movement(&chunk),
        None,
        "7 is not above 7"
    );
    assert_eq!(floor_impact(20.0, false).screen_movement(&chunk), None);
    let m = floor_impact(20.0, true)
        .screen_movement(&chunk)
        .expect("14");
    assert_eq!(m.source, Some(chunk.pos));
    assert_close(m.bump, Vec2::new(0.0, -1.4));
    assert_eq!(m.shake, 0.0);
    let m = floor_impact(100.0, true)
        .screen_movement(&chunk)
        .expect("70");
    assert_close(m.bump, Vec2::new(0.0, -7.0));
    assert!((m.shake - 0.8).abs() < 1e-6);
}

/// `ceilingTiles`: open tiles under a Solid one and over an open one, above the water
/// level; column by column (x, then y up).
#[test]
fn ceiling_tiles() {
    let rows = [
        "######", // y 4
        "#..#.#", // y 3
        "#....#", // y 2
        "#.#..#", // y 1
        "######", // y 0
    ];
    // (1, 3), (2, 3), (4, 3): under the Solid top row, over open tiles. (3, 2): under
    // the Solid (3, 3), over the open (3, 1). Every other open tile has an open one above.
    let tiles = RealizedRoom::find_ceiling_tiles(&room(&rows));
    assert_eq!(tiles, vec![(1, 3), (2, 3), (3, 2), (4, 3)]);
    // Water at row 2: only rows above 2 count.
    let tiles = RealizedRoom::find_ceiling_tiles(&room_with_water(&rows, 2));
    assert_eq!(tiles, vec![(1, 3), (2, 3), (4, 3)]);
}
