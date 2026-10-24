//! `RoomRain`'s flood in play: the constructor's `fWaterLevel`, `Update`'s flood arm (the
//! lerp toward `FloodLevel` and `GeneralUpsetSurface`'s three draws in a room with water,
//! `Room.AddWater` in a dry flood room once the death rain is on) and a body going under
//! the risen water. Expected values from `RoomRain.cs` (constructor, `Update`,
//! `FloodLevel`), `Room.cs` (`AddWater`) and `Water.cs` (constructor,
//! `GeneralUpsetSurface`), replayed by hand on a second generator.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, DangerType, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::tests::room_with_water;
use crate::world::{DeathRain, World};
use crate::{FloatExt, Input, RainWorldGame, Sounds, Start, Vec2, Water};

/// A 30 x 8 room, floor at y 1.
const POOL: [&str; 8] = [
    "##############################",
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "##############################",
];

/// A Survivor story in `POOL` (water row `water`, -1 for none) whose settings' danger is
/// `danger`, ready for AI.
fn game(water: i32, danger: DangerType) -> RainWorldGame {
    let room = room_with_water(&POOL, water);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        danger_type: Some(danger),
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((5, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
    );
    g.tick_until_ready(&mut ());
    g
}

fn water(g: &RainWorldGame) -> Option<&Water> {
    g.world().realized(0).unwrap().water.as_ref()
}

fn main_submersion(g: &RainWorldGame) -> f32 {
    g.player().unwrap().body.chunks[0].submersion(water(g))
}

fn order(g: &RainWorldGame) -> u64 {
    g.world()
        .realized(0)
        .unwrap()
        .room_rain
        .as_ref()
        .unwrap()
        .update_order
}

/// Sets the global rain's flood and intensity (0.3: no push, so `ThrowAroundObjects`
/// does not run).
fn set_rain(g: &mut RainWorldGame, flood: f32, death_rain: bool) {
    let global = &mut g.world_mut().rain_cycle.global_rain;
    global.flood = flood;
    global.intensity = 0.3;
    global.death_rain = death_rain.then(DeathRain::default);
    assert!(!global.any_push_around());
}

/// The constructor sets `fWaterLevel = originalWaterLevel + flood` in every room with a
/// water object, flooding or not (no precycle; user, 2026-10-24).
#[test]
fn the_constructor_starts_any_water_room_at_the_flood() {
    for danger in [DangerType::Rain, DangerType::Flood] {
        let mut g = game(3, danger);
        assert_eq!(water(&g).unwrap().f_water_level, 70.0);
        set_rain(&mut g, 40.0, false);
        g.world_mut().realized_mut(0).unwrap().room_rain = None;
        g.add_room_rain(0);
        assert_eq!(water(&g).unwrap().f_water_level, 110.0, "{danger:?}");
    }
}

/// A flood room with water: `fWaterLevel` goes a fifth of the way to
/// `originalWaterLevel + flood` (70 + 50) and `GeneralUpsetSurface(InverseLerp(0, 0.5,
/// 0.3) * 4)` kicks one point: `Range(0, totalPoints)` and two values, nothing else.
#[test]
fn the_flood_arm_raises_the_water_and_stirs_it() {
    let mut g = game(3, DangerType::Flood);
    set_rain(&mut g, 50.0, false);
    let total = water(&g).unwrap().point_count();
    let expected = water(&g).unwrap().clone();
    let mut replay = g.rng.clone();
    g.run_room_rain(0, order(&g));
    let w = water(&g).unwrap();
    assert!((w.f_water_level - 80.0).abs() < 1e-4);
    let i = replay.range_i32(0, total as i32) as usize;
    let intensity = 0.3_f32.inverse_lerp(0.0, 0.5) * 4.0;
    let moved = Vec2::from_degrees(replay.value() * 360.0) * (2.0 * intensity);
    let height = (-1.0_f32).lerp(1.0, replay.value()) * 4.0 * intensity;
    assert_eq!(g.rng.state(), replay.state(), "three draws");
    assert_eq!(w.point(i), expected.point(i) + moved);
    assert_eq!(w.wave_height(i), expected.wave_height(i) + height);
}

/// A `Rain` room's water never follows the flood (no arm), and a flood room before the
/// rain, with no flood and no level water, does nothing either: no draws.
#[test]
fn the_arm_skips_rain_rooms_and_dry_rooms_before_the_rain() {
    let mut g = game(3, DangerType::Rain);
    set_rain(&mut g, 50.0, true);
    let before = g.rng.state();
    g.run_room_rain(0, order(&g));
    assert_eq!(water(&g).unwrap().f_water_level, 70.0);
    assert_eq!(g.rng.state(), before);

    let mut g = game(-1, DangerType::Flood);
    assert!(g.world().rain_cycle.time_until_rain() > 0);
    set_rain(&mut g, 0.0, true);
    let before = g.rng.state();
    g.run_room_rain(0, order(&g));
    assert!(water(&g).is_none());
    assert_eq!(g.rng.state(), before);
}

/// A dry flood room once the flood has started: no water until the death rain is on, then
/// `AddWater` gives it `defaultWaterLevel` -10 and a `Water` at `MiddleOfTile(0, -10).y`
/// (-190) with its surface loop, without draws; the next turn lerps it toward -190 + flood.
#[test]
fn a_dry_flood_room_gets_water_with_the_death_rain() {
    let mut g = game(-1, DangerType::Flood);
    set_rain(&mut g, 20.0, false);
    let order = order(&g);
    g.run_room_rain(0, order);
    assert!(water(&g).is_none());
    set_rain(&mut g, 20.0, true);
    let before = g.rng.state();
    g.run_room_rain(0, order);
    assert_eq!(g.rng.state(), before);
    let r = g.world().realized(0).unwrap();
    assert!(r.added_water);
    assert_eq!(r.room.water_level(), Some(-10));
    assert!(r.water_sound.is_some());
    let w = r.water.as_ref().unwrap();
    assert_eq!(
        (w.level_row(), w.original_level(), w.f_water_level),
        (-10, -190.0, -190.0)
    );
    g.run_room_rain(0, order);
    assert!((water(&g).unwrap().f_water_level - (-190.0_f32).lerp(-170.0, 0.2)).abs() < 1e-4);
}

/// The player standing on the floor (y 1) of a flood room with water at row 0 goes under
/// once the flood has lifted the level well above it: its main chunk's submersion
/// follows `fWaterLevel` through the surface.
#[test]
fn the_player_goes_under_the_risen_water() {
    let mut g = game(0, DangerType::Flood);
    assert_eq!(main_submersion(&g), 0.0);
    set_rain(&mut g, 200.0, false);
    for _ in 0..40 {
        g.tick(Input::NONE, &mut ());
    }
    let w = water(&g).unwrap();
    assert!(w.f_water_level > 150.0, "{}", w.f_water_level);
    assert!(main_submersion(&g) > 0.9);
}

/// The dev rewind (D44) after a death rain with flood in a dry room and in a water room:
/// the global rain, the rooms' intensity, levels, added water and `rainDeath` start over,
/// with no draws.
#[test]
fn the_dev_rewind_resets_what_the_rain_built() {
    let mut g = game(-1, DangerType::Flood);
    set_rain(&mut g, 20.0, true);
    g.world_mut().rain_cycle.death_rain_has_hit = true;
    let first = order(&g);
    g.run_room_rain(0, first);
    assert!(g.world().realized(0).unwrap().added_water);
    g.world_mut()
        .realized_mut(0)
        .unwrap()
        .room_rain
        .as_mut()
        .unwrap()
        .intensity = 0.7;
    g.player.as_mut().unwrap().rain_death = 0.4;
    let before = g.rng.state();
    g.dev_rewind_rain();
    assert_eq!(g.rng.state(), before);
    let c = &g.world().rain_cycle;
    assert!(!c.death_rain_has_hit);
    assert_eq!(c.global_rain, crate::world::GlobalRain::default());
    let r = g.world().realized(0).unwrap();
    assert!(!r.added_water && r.water.is_none() && r.water_sound.is_none());
    assert_eq!(r.room.water_level(), None);
    let rain = r.room_rain.as_ref().unwrap();
    assert_eq!((rain.intensity, rain.last_intensity), (0.0, 0.0));
    assert_eq!(g.player().unwrap().rain_death, 0.0);

    let mut g = game(3, DangerType::Flood);
    set_rain(&mut g, 40.0, true);
    let second = order(&g);
    g.run_room_rain(0, second);
    assert!(water(&g).unwrap().f_water_level > 70.0);
    g.dev_rewind_rain();
    let w = water(&g).unwrap();
    assert_eq!((w.f_water_level, w.last_f_water_level()), (70.0, 70.0));
}

/// A timer at or past the cycle's end leaves everything alone.
#[test]
fn the_dev_rewind_leaves_a_timer_past_the_end() {
    let mut g = game(-1, DangerType::Flood);
    set_rain(&mut g, 20.0, true);
    let length = g.world().rain_cycle.cycle_length;
    g.dev_set_rain_timer(length + 5);
    let global = g.world().rain_cycle.global_rain;
    let order = order(&g);
    g.run_room_rain(0, order);
    g.dev_rewind_rain();
    assert_eq!(g.world().rain_cycle.global_rain, global);
    assert!(g.world().realized(0).unwrap().added_water);
}
