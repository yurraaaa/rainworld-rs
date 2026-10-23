//! `Room.Loaded`'s `DeathFallGraphic` and its counter in the update list. Expected values
//! from `Room.cs` (`Loaded`) and `DeathFallGraphic.cs` (`RoomRequiresDeathFall`,
//! `Update`).

use crate::creature::lizard::perception_turn::tests::{HALL, game};
use crate::{Input, RainWorldGame};

/// A 30 x 6 room whose floor stops at x 10: the bottom row has open tiles.
const OPEN_BOTTOM: [&str; 6] = [
    "##############################",
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "##########....................",
];

fn counter(g: &RainWorldGame) -> i32 {
    g.world()
        .realized(0)
        .unwrap()
        .death_fall
        .as_ref()
        .unwrap()
        .counter
}

/// A solid bottom row needs no death fall; an open tile in it does.
#[test]
fn only_a_room_with_an_open_bottom_gets_one() {
    assert!(
        game(&HALL, (3, 1))
            .world()
            .realized(0)
            .unwrap()
            .death_fall
            .is_none()
    );
    let g = game(&OPEN_BOTTOM, (3, 1));
    let fall = g.world().realized(0).unwrap().death_fall.as_ref().unwrap();
    assert!(fall.update_order > 0);
}

/// The counter is one per tick once the room is loaded, in the room's update list.
#[test]
fn the_counter_counts_one_per_tick() {
    let mut g = game(&OPEN_BOTTOM, (3, 1));
    let before = counter(&g);
    assert!(before > 0);
    for _ in 0..10 {
        g.tick(Input::NONE, &mut ());
    }
    assert_eq!(counter(&g), before + 10);
}
