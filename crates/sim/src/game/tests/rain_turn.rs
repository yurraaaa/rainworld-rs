//! The rain in `RainWorldGame.Update`: `GlobalRain.Update` draws at the head of the tick
//! (after the camera's shake step), `RainHit` after the timers, the first
//! `DeathRainUpdate` the tick after; camera 0's shake terms from the cycle and the room's
//! `RumbleIntensity`; `Room.RainDrownOutSounds`. Expected values from `RainWorldGame.cs`
//! (`Update`), `RainCycle.cs`, `GlobalRain.cs`, `RoomCamera.cs` (`Update`) and `Room.cs`.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, DangerType, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::tests::room;
use crate::world::World;
use crate::{FloatExt, Input, RainWorldGame, Sounds, Start};

/// A Survivor story in a 20 x 8 walled room with `settings`.
fn game(settings: RoomSettings, seed: i32) -> RainWorldGame {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 6]);
    rows.push("####################");
    let room = room(&rows);
    let data = WorldData::from_room(&room);
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((2, 1)),
        },
        UnityRandom::new(seed),
        Sounds::default(),
    );
    for _ in 0..10 {
        g.tick(Input::NONE, &mut ());
    }
    g
}

fn rain_room(rumble: f32) -> RoomSettings {
    RoomSettings {
        random_item_density: 0.0,
        danger_type: Some(DangerType::Rain),
        rumble_intensity: rumble,
        ..RoomSettings::ANCESTOR
    }
}

/// `GlobalRain.Update` is the first thing to draw in a tick (no shake step before it
/// draws): the target direction follows the head of the stream.
#[test]
fn the_direction_draws_head_the_tick() {
    let mut g = game(rain_room(1.0), 11);
    let mut target = g.world.rain_cycle.global_rain.rain_direction_get_to;
    let mut twos = 0;
    for _ in 0..800 {
        assert_eq!(g.camera_shake.screen_shake, 0.0, "no jump draw before it");
        let mut head = g.rng.clone();
        if head.value() < 0.025 {
            target = (-1.0_f32).lerp(1.0, head.value());
            twos += 1;
        }
        g.tick(Input::NONE, &mut ());
        assert_eq!(g.world.rain_cycle.global_rain.rain_direction_get_to, target);
    }
    assert!(twos > 5, "{twos} two-draw ticks");
}

/// `RainHit` runs in `RainCycle.Update` (after the realizer and the world processes), so
/// the tick that brings the timer to the length starts the death rain without updating
/// it; the next tick's `GlobalRain.Update` runs its first `DeathRainUpdate`.
#[test]
fn the_first_death_rain_update_is_the_tick_after_the_hit() {
    let mut g = game(rain_room(1.0), 4);
    let length = g.world.rain_cycle.cycle_length;
    g.dev_set_rain_timer(length - 2);
    g.tick(Input::NONE, &mut ());
    assert!(!g.world.rain_cycle.death_rain_has_hit);
    assert_eq!(g.world.rain_cycle.timer, length - 1);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.world.rain_cycle.timer, length);
    assert!(g.world.rain_cycle.death_rain_has_hit);
    let hit = g.world.rain_cycle.global_rain.death_rain.unwrap();
    assert_eq!(hit.progression, 0.0);
    // The ramp's last value: the pre-rain branch ran on the tick before.
    assert!((g.world.rain_cycle.global_rain.intensity - 0.24).abs() < 1e-6);
    g.tick(Input::NONE, &mut ());
    let next = g.world.rain_cycle.global_rain.death_rain.unwrap();
    assert_eq!(next.progression, 1.0 / hit.time_in_this_mode);
    assert_eq!(next.mode, hit.mode);
}

/// From `RainGameOver` the cycle's `ShaderLight` and shakes are the global rain's.
#[test]
fn the_cycle_reads_the_global_rain_after_the_hit() {
    let mut g = game(rain_room(1.0), 4);
    let length = g.world.rain_cycle.cycle_length;
    g.dev_set_rain_timer(length - 1);
    for _ in 0..300 {
        g.tick(Input::NONE, &mut ());
    }
    let c = &g.world.rain_cycle;
    assert!(c.rain_game_over());
    assert_eq!(c.shader_light(), c.global_rain.shader_light);
    assert_eq!(c.screen_shake(), c.global_rain.screen_shake);
    assert_eq!(c.micro_screen_shake(), c.global_rain.micro_screen_shake);
}

/// `RoomCamera.Update`: in a room with rain danger the camera takes the cycle's
/// `ScreenShake` times `RumbleIntensity` (here `0.25 * 0.5` 240 ticks before the rain),
/// jumps and decays by 0.025; `microShake` is the cycle's times `RumbleIntensity`.
#[test]
fn the_camera_shakes_with_the_approaching_rain() {
    let mut g = game(rain_room(0.5), 4);
    let length = g.world.rain_cycle.cycle_length;
    g.world.rain_cycle.timer = length - 240;
    let mut m = g.rng.clone();
    g.update_camera_shake();
    let jump = crate::Vec2::from_degrees(m.value() * 360.0) * (8.0 * 0.125);
    assert!((g.camera_shake.screen_shake - 0.1).abs() < 1e-6);
    assert!((g.camera_shake.take_displacement() - jump).length() < 1e-4);
    assert_eq!(g.rng, m, "one draw");
    let micro = (1.0_f32 - 0.1 / 0.6).powf(1.5) * 0.5;
    assert!((g.camera_shake.micro_shake - micro).abs() < 1e-5);
}

/// A room without rain danger is not shaken by the cycle.
#[test]
fn a_room_without_danger_is_not_shaken() {
    let settings = RoomSettings {
        danger_type: Some(DangerType::None),
        ..rain_room(1.0)
    };
    let mut g = game(settings, 4);
    let length = g.world.rain_cycle.cycle_length;
    g.world.rain_cycle.timer = length - 100;
    let before = g.rng.clone();
    g.update_camera_shake();
    assert_eq!(g.rng, before);
    assert_eq!(
        (g.camera_shake.screen_shake, g.camera_shake.micro_shake),
        (0.0, 0.0)
    );
    assert!(!g.rain_shake().rain_danger);
}

/// `rain_shake` reads the settings: the room's danger and `RumbleIntensity`, and the
/// cycle's values.
#[test]
fn the_rain_shake_reads_the_room_and_the_cycle() {
    let mut g = game(rain_room(0.25), 4);
    let length = g.world.rain_cycle.cycle_length;
    g.world.rain_cycle.timer = length - 240;
    let shake = g.rain_shake();
    assert!(shake.rain_danger && !shake.aerie_blizzard && !shake.shelter);
    assert_eq!(shake.rumble, 0.25);
    assert!((shake.screen_shake - 0.25).abs() < 1e-6);
    assert_eq!(
        shake.micro_screen_shake,
        g.world.rain_cycle.micro_screen_shake()
    );
}

/// `Room.RainDrownOutSounds`: 0 in a `None` room whatever the rain, else `InverseLerp(0,
/// 0.6, roomRain.intensity)`, 0 without a `RoomRain`.
#[test]
fn the_drown_out_follows_the_room_rain() {
    let g = game(rain_room(1.0), 4);
    let room = g.world.realized(g.camera_room).unwrap();
    assert_eq!(room.rain_drown_out_sounds(None), 0.0);
    assert!((room.rain_drown_out_sounds(Some(0.3)) - 0.5).abs() < 1e-6);
    assert_eq!(room.rain_drown_out_sounds(Some(0.9)), 1.0);
    let settings = RoomSettings {
        danger_type: Some(DangerType::None),
        ..rain_room(1.0)
    };
    let g = game(settings, 4);
    let room = g.world.realized(g.camera_room).unwrap();
    assert_eq!(room.rain_drown_out_sounds(Some(0.3)), 0.0);
    assert_eq!(g.rain_drown_out_sounds(), 0.0);
}
