//! The realizer's `currentlyLoadingRoom` wait under the game's minimum loading schedule:
//! a neighbour realized in update N pauses updates N+1 to N+3 (cleared on N+3, once the
//! room is `fullyLoaded` from N+2's loading step), the per-tick neighbour draw coming back
//! on N+4; and the wait on a room abstracted while it loads. Expected values from
//! `RoomRealizer.Update`, `RealizeAndTrackRoom` and `RoomPreparer.Update`.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, RoomSettings, Timeline};
use rw_random::UnityRandom;

use super::{FollowCreature, RoomRealizer};
use crate::tests::room;
use crate::world::World;

/// A 10 x 6 room as global room 0, joined to a copy of it, room 2 (1 is the off-screen
/// den), in a region world; room 0 realized and loaded.
fn two_rooms() -> World {
    let mut rows = vec!["##########"];
    rows.extend(["#........#"; 4]);
    rows.push("##########");
    let room = room(&rows);
    let mut data = WorldData::from_room(&room);
    data.single_room = false;
    data.rooms[0].connections = vec![2];
    let mut second = data.rooms[0].clone();
    second.name = "ROOM_B".to_owned();
    second.index = 2;
    second.connections = vec![0];
    data.rooms.push(second);
    let settings = RoomSettings {
        random_item_density: 0.0,
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let mut world = World::new(data, rooms, Timeline::White, ActiveMods::NONE, true);
    world.rain_cycle.cycle_length = 9600;
    world.activate_room(0).expect("loads");
    world.finish_loading();
    world
}

const FOLLOW: FollowCreature = FollowCreature {
    room: 0,
    tile: (2, 1),
    in_room: true,
    alive: true,
};

impl RoomRealizer {
    /// One tick's realizer update and loading step; whether the update drew.
    fn tick(&mut self, world: &mut World, rng: &mut UnityRandom) -> bool {
        let before = rng.state();
        self.update(world, FOLLOW, 0, rng);
        let drew = rng.state() != before;
        world.loading_step();
        drew
    }

    /// Ticks until the random neighbour (the only connection, drawn every tick) is
    /// realized: its 41st draw, `counter > 40`.
    fn realize_neighbour(&mut self, world: &mut World, rng: &mut UnityRandom) {
        for n in 1..=41 {
            let before = rng.state();
            self.update(world, FOLLOW, 0, rng);
            assert_ne!(rng.state(), before, "update {n} draws its neighbour");
            assert_eq!(world.realized(2).is_some(), n == 41, "update {n}");
            if n < 41 {
                world.loading_step();
            }
        }
    }
}

#[test]
fn a_realized_neighbour_pauses_three_updates() {
    let mut world = two_rooms();
    let mut rng = UnityRandom::new(3);
    let mut realizer = RoomRealizer::new();
    realizer.realize_neighbour(&mut world, &mut rng);
    // Update N's loading step starts the thread.
    world.loading_step();
    assert_eq!(world.realized(2).unwrap().loading_progress(), 0);
    let mut progress = Vec::new();
    let mut drew = Vec::new();
    for _ in 0..4 {
        drew.push(realizer.tick(&mut world, &mut rng));
        progress.push(world.realized(2).unwrap().loading_progress());
    }
    assert_eq!(progress, [1, 3, 3, 3]);
    assert_eq!(drew, [false, false, false, true]);
}

/// A room abstracted while it loads keeps its preparer (the game's `Abstractize` leaves it
/// in `loadingRooms`): the realizer waits for the dropped room to be `fullyLoaded`, the
/// same three updates.
#[test]
fn the_wait_outlives_an_abstracted_room() {
    let mut world = two_rooms();
    let mut rng = UnityRandom::new(3);
    let mut realizer = RoomRealizer::new();
    realizer.realize_neighbour(&mut world, &mut rng);
    world.loading_step();
    let serial = world.realized(2).unwrap().serial();
    world.abstractize(2, &mut rng);
    assert!(world.realized(2).is_none());
    assert_eq!(world.orphaned_preparers().count(), 1);
    assert!(!RoomRealizer::fully_loaded(&world, 2, serial));
    let drew: Vec<bool> = (0..4)
        .map(|_| realizer.tick(&mut world, &mut rng))
        .collect();
    assert_eq!(drew, [false, false, false, true]);
    assert!(RoomRealizer::fully_loaded(&world, 2, serial));
}

const DEN_FOLLOW: FollowCreature = FollowCreature {
    room: 1,
    tile: (0, 0),
    in_room: false,
    alive: true,
};

/// `RoomRealizer.Update`: `!followCreature.Room.offScreenDen` on the room change, and the
/// off-screen-den guard before the random neighbour's `Random.Range`.
#[test]
fn the_off_screen_den_guards_hold() {
    let mut world = two_rooms();
    world.data.rooms[1].offscreen_den = true;
    world.data.rooms[1].connections = vec![0];
    let mut rng = UnityRandom::new(3);
    let mut realizer = RoomRealizer::new();
    realizer.update(&mut world, FOLLOW, 0, &mut rng);
    let before = rng.state();
    realizer.update(&mut world, DEN_FOLLOW, 0, &mut rng);
    assert_eq!(rng.state(), before, "no draw in the den");
    assert!(
        realizer.tracked_rooms().iter().all(|&r| r != 1),
        "den not tracked"
    );
    realizer.update(&mut world, FOLLOW, 0, &mut rng);
    world.loading_step();
    let before = rng.state();
    realizer.update(&mut world, FOLLOW, 0, &mut rng);
    assert_ne!(rng.state(), before, "one draw in a normal room");
}

/// `RoomRealizer.Update`: `world.rainCycle.RainGameOver` ends the update before the random
/// neighbour's `Random.Range`; one tick before the cycle's end it still draws.
#[test]
fn the_rain_game_over_guard_stops_the_neighbour_draw() {
    let mut world = two_rooms();
    let mut rng = UnityRandom::new(3);
    let mut realizer = RoomRealizer::new();
    world.rain_cycle.timer = world.rain_cycle.cycle_length - 1;
    let before = rng.state();
    realizer.update(&mut world, FOLLOW, 0, &mut rng);
    assert_ne!(rng.state(), before, "one draw before the end");
    world.loading_step();
    world.rain_cycle.timer = world.rain_cycle.cycle_length;
    let before = rng.state();
    realizer.update(&mut world, FOLLOW, 0, &mut rng);
    assert_eq!(rng.state(), before, "no draw once the rain is over");
}
