//! The slugcat's look at the sky in the cycle's last 2400 ticks (`PlayerGraphics.Update`,
//! `PlayerObjectLooker.LookAtPoint`): a value above `RainApproaching`, a value below 1/102
//! and, in a `Rain` or `FloodAndRain` room, a value for the point's x; the point is
//! `(PixelWidth x, PixelHeight + 100)` with interest `(1 - RainApproaching) * 0.6`.
//! Expected values from `PlayerGraphics.cs` (`Update`, `PlayerObjectLooker`).
//!
//! Mirrors: - (port-only: tests)

use rw_assets::{DangerType, Room, RoomSettings};
use rw_random::UnityRandom;
use rw_sim::world::World;
use rw_sim::{Player, Vec2};

use super::{alone_world, grounded_player, surroundings, update_in, world_with_settings};
use crate::PlayerGraphics;
use crate::tests::flat_room;

/// A world with `room` realized, 2400-tick story cycle `until` ticks from the rain, in a
/// room of `danger`.
fn world_with(room: &Room, danger: DangerType, until: i32) -> World {
    let settings = RoomSettings {
        danger_type: Some(danger),
        ..RoomSettings::ANCESTOR
    };
    let mut world = world_with_settings(room, settings);
    world.rain_cycle.story_mode = true;
    world.rain_cycle.cycle_length = 10_000;
    world.rain_cycle.timer = 10_000 - until;
    world.activate_room(0).expect("loads");
    world
}

/// A player and its graphics after their first update (the first one resets the module).
fn settled(room: &Room, rng: &mut UnityRandom) -> (Player, PlayerGraphics) {
    let (player, mut graphics) = grounded_player(rng);
    let far = alone_world(room);
    update_in(&mut graphics, &player, room, &far, rng);
    (player, graphics)
}

/// The first seed whose next two draws are above `approaching` and below 1/102.
fn seed_passing_both(approaching: f32) -> i32 {
    (0..)
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            r.value() > approaching && r.value() < 1.0 / 102.0
        })
        .unwrap()
}

#[test]
fn a_rain_room_in_the_last_2400_ticks_looks_up() {
    let room = flat_room();
    let until = 1200;
    let approaching = 0.5;
    let seed = seed_passing_both(approaching);
    for danger in [DangerType::Rain, DangerType::FloodAndRain] {
        let world = world_with(&room, danger, until);
        let mut rng = UnityRandom::new(0);
        let (player, mut graphics) = settled(&room, &mut rng);
        let mut rng = UnityRandom::new(seed);
        let mut m = rng.clone();
        m.value();
        m.value();
        let x = m.value();
        update_in(&mut graphics, &player, &room, &world, &mut rng);
        let point = Vec2::new(room.pixel_width() * x, room.pixel_height() + 100.0);
        assert_eq!(graphics.looker.look_at_point, Some(point), "{danger:?}");
        assert!((graphics.looker.look_at_point_interest - 0.3).abs() < 1e-6);
        assert!(graphics.looker.looking());
    }
}

/// Other danger types draw the two values and stop there.
#[test]
fn other_rooms_draw_but_do_not_look() {
    let room = flat_room();
    let seed = seed_passing_both(0.5);
    for danger in [DangerType::None, DangerType::Flood, DangerType::Thunder] {
        let world = world_with(&room, danger, 1200);
        let mut rng = UnityRandom::new(0);
        let (player, mut graphics) = settled(&room, &mut rng);
        update_in(
            &mut graphics,
            &player,
            &room,
            &world,
            &mut UnityRandom::new(seed),
        );
        assert_eq!(graphics.looker.look_at_point, None, "{danger:?}");
    }
}

/// With `RainApproaching` at 1 nothing is drawn for it: the seed that passes both draws at
/// half does not look.
#[test]
fn far_from_the_rain_it_draws_nothing_and_does_not_look() {
    let room = flat_room();
    let seed = seed_passing_both(0.5);
    let world = world_with(&room, DangerType::Rain, 2400);
    let mut rng = UnityRandom::new(0);
    let (player, mut graphics) = settled(&room, &mut rng);
    update_in(
        &mut graphics,
        &player,
        &room,
        &world,
        &mut UnityRandom::new(seed),
    );
    assert_eq!(graphics.looker.look_at_point, None);
}

/// While it looks at the point `lookDirection` is the direction to it; `LookAtNothing`
/// drops the point.
#[test]
fn the_looker_follows_the_point_and_forgets_it() {
    let room = flat_room();
    let seed = seed_passing_both(0.5);
    let world = world_with(&room, DangerType::Rain, 1200);
    let mut rng = UnityRandom::new(0);
    let (player, mut graphics) = settled(&room, &mut rng);
    update_in(
        &mut graphics,
        &player,
        &room,
        &world,
        &mut UnityRandom::new(seed),
    );
    let point = graphics.looker.look_at_point.expect("looks up");
    // `lookDirection` is `DirVec(head, mostInterestingLookPoint)` while looking.
    let expect = graphics.head.pos.direction_to(point);
    assert!((graphics.look_direction - expect).length() < 1e-4);
    // `LookAtNothing` drops the point.
    graphics.looker.look_at_nothing();
    assert_eq!(graphics.looker.look_at_point, None);
    assert!(!graphics.looker.looking());
}

/// The draws: none with `RainApproaching` at 1; else a value that stops at one when it is
/// not above `RainApproaching`, a second that stops at two when it is not below 1/102 or
/// the room has no rain, and the point's x as the third.
#[test]
fn the_sky_look_draws_one_to_three_values() {
    let room = flat_room();
    let mut rng = UnityRandom::new(0);
    let (_, mut graphics) = settled(&room, &mut rng);
    let far = world_with(&room, DangerType::Rain, 2400);
    let near = world_with(&room, DangerType::Rain, 1200);
    let flood = world_with(&room, DangerType::Flood, 1200);
    let both = seed_passing_both(0.5);
    let low_first = (0..).find(|&s| UnityRandom::new(s).value() <= 0.5).unwrap();
    let high_second = (0..)
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            r.value() > 0.5 && r.value() >= 1.0 / 102.0
        })
        .unwrap();
    for (world, seed, draws) in [
        (&far, both, 0),
        (&near, low_first, 1),
        (&near, high_second, 2),
        (&flood, both, 2),
        (&near, both, 3),
    ] {
        let mut rng = UnityRandom::new(seed);
        let mut mirror = rng.clone();
        for _ in 0..draws {
            mirror.value();
        }
        graphics.looker.look_at_nothing();
        graphics.sky_look(&room, &surroundings(world), &mut rng);
        assert_eq!(rng, mirror, "seed {seed}: {draws} draws");
    }
}

/// `PlayerGraphics.Update`'s reach for a fly is held back by the side of the fly itself
/// (`currentMostInteresting.bodyChunks[0].pos.x`), not of `mostInterestingLookPoint`, which
/// the sky look can leave elsewhere.
#[test]
fn the_reach_is_blocked_by_the_flys_side() {
    // Moving right (input 1) with the fly to the left: blocked; fly to the right: not.
    assert!(PlayerGraphics::reach_blocked(90.0, 100.0, 1));
    assert!(!PlayerGraphics::reach_blocked(110.0, 100.0, 1));
    assert!(PlayerGraphics::reach_blocked(110.0, 100.0, -1));
    assert!(!PlayerGraphics::reach_blocked(90.0, 100.0, -1));
    assert!(!PlayerGraphics::reach_blocked(90.0, 100.0, 0));
}
