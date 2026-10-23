//! `DeathFallGraphic`'s state. Expected values from `DeathFallGraphic.cs`
//! (`RoomRequiresDeathFall`, `InitiateSprites`, `Update`, `DrawSprites`) and `Room.cs`
//! (`CameraViewingPoint`).

use super::*;

/// A 20 x 12 room of air, its bottom row solid if asked, with cameras at file
/// positions (0, 0) and (0, 300): room y -560 and -860.
fn room(solid_bottom: bool) -> Room {
    let (w, h) = (20, 12);
    let column = format!(
        "{}{}",
        "0|".repeat(h - 1),
        if solid_bottom { "1|" } else { "0|" }
    );
    let text = format!(
        "Test\n{w}*{h}|-1|0\n1*1|0|0\n0,0|0,300\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{}\n",
        column.repeat(w)
    );
    Room::parse(&text).expect("test room parses")
}

fn focus(pos: [f32; 2]) -> PlacedObject {
    PlacedObject {
        kind: PlacedObjectType::DeathFallFocus.value().to_owned(),
        pos,
        data: String::new(),
        active: true,
    }
}

/// `RoomRequiresDeathFall`: an open bottom tile and not an unbroken shelter; the height
/// starts at the lowest camera's bottom.
#[test]
fn open_bottom_rooms_have_a_death_fall() {
    let fall = DeathFall::new(&room(false), None, false, &[]).expect("open bottom");
    assert_eq!(fall.height, -860.0);
    assert_eq!(fall.default_height(), -860.0);
    assert!(DeathFall::new(&room(true), None, false, &[]).is_none());
    assert!(DeathFall::new(&room(false), None, true, &[]).is_none());
}

/// `Update`: a focal point the camera shows sets the height to the bottom of the first
/// camera rect (1366x768) holding it; out of view, the default. Before the first `Update`
/// the height is `InitiateSprites`' default, whatever the focal points.
#[test]
fn a_viewed_focal_point_moves_the_height() {
    let r = room(false);
    let mut fall = DeathFall::new(&r, None, false, &[focus([100.0, 0.0])]).unwrap();
    let view = CameraView {
        room: 3,
        current: 0,
        next: 0,
    };
    assert_eq!(fall.height, -860.0);
    assert_eq!(fall.counter, 0);
    fall.update(&r, 3, &view);
    assert_eq!(fall.height, -560.0);
    fall.initiate_sprites();
    assert_eq!(fall.height, -860.0, "InitiateSprites resets it");
    fall.update(&r, 4, &view);
    assert_eq!(fall.height, -860.0, "camera in another room");
    assert_eq!(fall.height_under(&r, 3, &view), -560.0);
}

/// `Update` counts every tick, whichever room the camera is in.
#[test]
fn the_counter_ticks_whether_or_not_the_room_is_viewed() {
    let r = room(false);
    let mut fall = DeathFall::new(&r, None, false, &[]).unwrap();
    let view = CameraView {
        room: 3,
        current: 0,
        next: 0,
    };
    fall.update(&r, 3, &view);
    fall.update(&r, 4, &view);
    fall.update(&r, 4, &view);
    assert_eq!(fall.counter, 3);
    let a = 0.85 + 0.15 * (3.5f32 / 25.0).sin();
    assert!((fall.alpha(0.5) - a).abs() < 1e-6);
}
