//! Camera shake in a game: `Room.ScreenMovement`'s drips in a realized room and the
//! camera's jump at the head of the tick. Expected values from `Room.cs`
//! (`ScreenMovement`) and `RoomCamera.cs` (`Update`).

use rw_random::UnityRandom;

use crate::camera::CameraView;
use crate::game::lizard_perception::tests::{HALL, game};
use crate::{FloatExt, Input, ScreenMovement, Vec2, WaterDrip};

fn assert_close(a: Vec2, b: Vec2) {
    assert!(a.distance(b) < 1e-4, "{a:?} vs {b:?}");
}

/// The draws of the drips in `HALL` (28 ceiling tiles at y 4, `CeilingDrips` 0.5): a
/// shake of 2 (force 6) makes `RoundToInt(Lerp(28, 150, 0.25) / 100 * 0.5 * 6)` =
/// `RoundToInt(1.755)` = 2 drips; each a tile, an x jitter and, seen, an x speed and a
/// life: 4 draws each. Unseen, 2 draws each and no drip.
#[test]
fn shaken_drips_draw_two_or_four_each() {
    let g = game(&HALL, (3, 1));
    let room = g.world().realized(0).expect("realized");
    assert_eq!(room.ceiling_tiles.len(), 28);
    assert_eq!(room.ceiling_tiles[0], (1, 4));
    let m = ScreenMovement {
        source: None,
        bump: Vec2::ZERO,
        shake: 2.0,
    };
    let seen = CameraView {
        room: 0,
        current: 0,
        next: 0,
    };
    let mut rng = UnityRandom::new(9);
    let mut expect = rng.clone();
    let mut want = Vec::new();
    for _ in 0..2 {
        let tile = room.ceiling_tiles[expect.range_i32(0, 28) as usize];
        let pos = Vec2::middle_of_tile(tile) + Vec2::new((-10.0).lerp(10.0, expect.value()), 9.0);
        let vel = Vec2::new((-1.5).lerp(1.5, expect.value()), 0.0);
        want.push(WaterDrip::new(pos, vel, false, &mut expect));
    }
    let drips = room.shaken_ceiling_drips(0, &m, &seen, &mut rng);
    assert_eq!(rng, expect);
    assert_eq!(drips, want);

    let unseen = CameraView { room: 1, ..seen };
    let mut rng = UnityRandom::new(9);
    let mut expect = rng.clone();
    for _ in 0..4 {
        let _ = expect.value();
    }
    assert!(
        room.shaken_ceiling_drips(0, &m, &unseen, &mut rng)
            .is_empty()
    );
    assert_eq!(rng, expect, "2 draws a drip unseen");

    // Below the threshold (bump 1.5 = 0.375, a shelter's 0.5 shake = 1.5): nothing.
    let mut rng = UnityRandom::new(9);
    let before = rng.clone();
    let weak = ScreenMovement {
        source: None,
        bump: Vec2::new(1.5, 0.0),
        shake: 0.5,
    };
    assert!(
        room.shaken_ceiling_drips(0, &weak, &seen, &mut rng)
            .is_empty()
    );
    assert_eq!(rng, before);
}

/// `room.ScreenMovement` in the camera's room reaches the camera; the next tick's first
/// draw is the camera's jump (`RoomCamera.Update` runs before the rooms).
#[test]
fn the_jump_is_the_first_draw_of_the_tick() {
    let mut g = game(&HALL, (3, 1));
    let drips = g.world().realized(0).map(|r| r.drips.len());
    g.room_screen_movement(
        0,
        ScreenMovement {
            source: None,
            bump: Vec2::new(0.0, 2.0),
            shake: 0.5,
        },
    );
    assert_eq!(g.world().realized(0).map(|r| r.drips.len()), drips);
    assert_eq!(g.camera_shake().screen_shake, 0.5);
    assert_eq!(g.take_camera_displacement(), Vec2::new(0.0, -2.0));
    let mut probe = g.rng().clone();
    let jump = Vec2::from_degrees(probe.value() * 360.0) * 4.0;
    g.tick(Input::NONE, &mut ());
    assert_close(g.take_camera_displacement(), jump);
    assert!((g.camera_shake().screen_shake - 0.475).abs() < 1e-6);
}
