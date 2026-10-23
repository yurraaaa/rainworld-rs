//! `Room.Loaded` adding `CustomDecal`s and `Rainbow`s in the settings' order, the
//! rainbow's chance gate and its end with the cycle's start-up. Expected values follow
//! `Room.Loaded` and `Rainbow.Update`, replayed on seeded streams.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, PlacedObject, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::game::RoomObject;
use crate::tests::room;
use crate::world::World;
use crate::{Input, RainWorldGame, Sounds, Start, Vec2};

const DECAL: &str = "0~40~40~40~40~0~0~0~0.2~0.9~0~Rust1";
const RAINBOW_HALF: &str = "-30~40~0~0~1,1,1,1,0.5,0.5";

/// A story game in an open room with `placed` objects, in cycle `cycle` (cycle 0 has no
/// start-up, so no rainbow), made with random seed `seed`.
fn game(placed: &str, cycle: i32, seed: i32) -> RainWorldGame {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 10]);
    rows.push("####################");
    let room = room(&rows);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        placed_objects: PlacedObject::parse_list(placed),
        ..RoomSettings::ANCESTOR
    };
    let campaign = &SlugcatCampaign::SURVIVOR;
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let mut story = StoryStart::new_game(campaign);
    story.cycle_number = cycle;
    RainWorldGame::new_story(
        world,
        campaign,
        story,
        Start {
            room: 0,
            tile: Some((2, 1)),
        },
        UnityRandom::new(seed),
        Sounds::default(),
        &mut (),
    )
}

/// A decal, a rainbow and a `RainbowNoFade` join the update list in the settings' order;
/// the decal takes no turns, the rainbows do. Rainbows are made only in a cycle with a
/// start-up, and then by the `SeededRandom` chance: the pair is found by trying seeds.
#[test]
fn room_loaded_adds_them_in_settings_order() {
    let placed = format!(
        "CustomDecal><100><100><{DECAL}, Rainbow><300><100><{RAINBOW_HALF}, \
         RainbowNoFade><200><60><{RAINBOW_HALF}"
    );
    let (mut with, mut without) = (None, None);
    for seed in 1..200 {
        let g = game(&placed, 1, seed);
        let r = g.world.realized(0).expect("realized");
        assert_eq!(r.decals.len(), 1);
        let gate = UnityRandom::new(g.world.rain_cycle.rainbow_seed).value();
        assert!(g.world.rain_cycle.cycle_start_up() < 1.0);
        let made = r.rainbows.len();
        assert_eq!(made, if gate < 0.5 { 2 } else { 1 }, "seed {seed}");
        if made == 2 {
            with = Some(seed);
        } else {
            without = Some(seed);
        }
        if with.is_some() && without.is_some() {
            break;
        }
    }
    let g = game(&placed, 1, with.expect("a seed that passes the gate"));
    let r = g.world.realized(0).expect("realized");
    let decal = r.decals[0].update_order;
    let rainbow = r.rainbows[0].update_order;
    let no_fade = r.rainbows[1].update_order;
    assert!(decal < rainbow && rainbow < no_fade);
    assert!(!r.rainbows[0].always_show && r.rainbows[1].always_show);
    assert_eq!(r.rainbows[0].pos, Vec2::new(330.0, 60.0));
    assert_eq!(r.rainbows[0].rad, 100.0);
    assert_eq!(
        g.room_rainbow_objects(0),
        [
            (rainbow, RoomObject::Rainbow(rainbow)),
            (no_fade, RoomObject::Rainbow(no_fade))
        ]
    );
    let g = game(&placed, 1, without.expect("a seed that fails the gate"));
    let r = g.world.realized(0).expect("realized");
    assert_eq!(r.rainbows.len(), 1);
    assert!(r.rainbows[0].always_show);
}

/// Cycle 0 has no start-up (`CycleStartUp` is 1): the chance-gated rainbow is not made
/// whatever the seed; the decal and the `RainbowNoFade` are.
#[test]
fn no_rainbow_without_a_start_up() {
    let placed = format!(
        "CustomDecal><100><100><{DECAL}, Rainbow><300><100><{RAINBOW_HALF}, \
         RainbowNoFade><200><60><{RAINBOW_HALF}"
    );
    for seed in 1..20 {
        let g = game(&placed, 0, seed);
        let r = g.world.realized(0).expect("realized");
        assert_eq!((r.decals.len(), r.rainbows.len()), (1, 1), "seed {seed}");
        assert!(r.rainbows[0].always_show);
    }
}

/// Neither the placed objects' constructors nor the gate draw from the shared stream.
#[test]
fn room_loaded_makes_no_shared_draws() {
    let placed = format!("CustomDecal><100><100><{DECAL}, Rainbow><300><100><{RAINBOW_HALF}");
    let with = game(&placed, 1, 11);
    let without = game("", 1, 11);
    assert_eq!(with.rng.state(), without.rng.state());
}

/// A chance-gated rainbow is destroyed on the first update at which the start-up is over;
/// before that it fades as `sin(start-up^0.75 * pi)^0.6`.
#[test]
fn a_rainbow_ends_with_the_start_up() {
    let placed = format!("Rainbow><300><100><{RAINBOW_HALF}");
    let seed = (1..500)
        .find(|&s| {
            game(&placed, 1, s)
                .world
                .realized(0)
                .unwrap()
                .rainbows
                .len()
                == 1
        })
        .expect("a seed that passes the gate");
    let mut g = game(&placed, 1, seed);
    // The rain cycle ticks before the rooms: at timer 600 of 2400 the start-up is 0.25.
    g.world.rain_cycle.timer = 599;
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.world.rain_cycle.cycle_start_up(), 0.25);
    let want = (0.25_f64.powf(0.75) * std::f64::consts::PI).sin().powf(0.6);
    let r = g.world.realized(0).unwrap();
    assert_eq!(r.rainbows.len(), 1);
    assert!(
        (f64::from(r.rainbows[0].fade) - want).abs() < 1e-6,
        "{}",
        r.rainbows[0].fade
    );
    // Reaching the end of the start-up: that update removes it.
    g.world.rain_cycle.timer = 2399;
    g.tick(Input::NONE, &mut ());
    assert!(g.world.realized(0).unwrap().rainbows.is_empty());
}
