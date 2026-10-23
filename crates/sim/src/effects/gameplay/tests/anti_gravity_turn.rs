//! `Room.Loaded`'s `AntiGravity`, its gravity per tick and the world's switch in a test
//! room (`AntiGravity.cs`).
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, ClipNames, PlacedObject, RoomEffect, RoomSettings, SoundTable};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::tests::{named_room, room_with_water};
use crate::world::World;
use crate::{Input, RainWorldGame, SoundEvent, SoundId, Sounds, Start};

use crate::effects::lights::turn::tests::WithGraphics;

const SOUNDS: &str = "Volume: 6\nVolume exponent: 1.8\nBroken_Anti_Gravity_Switch_On : clip\nBroken_Anti_Gravity_Switch_Off : clip\n";

fn zero_g_game(effects: &[(&str, f32)]) -> RainWorldGame {
    let mut rows: Vec<String> = vec![".".repeat(20); 10];
    rows[9] = "#".repeat(20);
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    game_in(room_with_water(&refs, -1), effects, false)
}

/// SS_E08's size (960 x 3780 px) and name, with `RoomSpecificScriptActive: ON`.
fn gradient_game() -> RainWorldGame {
    let mut rows: Vec<String> = vec![".".repeat(48); 189];
    rows[188] = "#".repeat(48);
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    game_in(named_room("SS_E08", &refs), &[], true)
}

/// A story in `room`, loaded by its loading steps alone (no tick run).
fn game_in(room: rw_assets::Room, effects: &[(&str, f32)], script: bool) -> RainWorldGame {
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        room_specific_script: script,
        placed_objects: PlacedObject::parse_list(""),
        effects: effects
            .iter()
            .map(|&(kind, amount)| RoomEffect {
                kind: kind.to_owned(),
                amount,
            })
            .collect(),
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let names = ClipNames::new(["clip".to_owned()], None);
    let table = Arc::new(SoundTable::parse(SOUNDS, &names).unwrap());
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((2, 1)),
        },
        UnityRandom::new(5),
        Sounds::new(table),
    );
    g.load_room_now(0, &mut WithGraphics);
    g
}

/// A `ZeroG` amount of 1 makes the room's gravity 0 on the `AntiGravity`'s first turn; below
/// 1 the amount is `Lerp(0, 0.85, a)`.
#[test]
fn zero_g_sets_the_room_gravity() {
    let mut g = zero_g_game(&[("ZeroG", 1.0)]);
    assert!(g.world.realized(0).unwrap().anti_gravity.is_some());
    assert_eq!(g.world.realized(0).unwrap().gravity, 1.0);
    g.tick(Input::NONE, &mut WithGraphics);
    assert_eq!(g.world.realized(0).unwrap().gravity, 0.0);
    assert!(g.world.rain_cycle.broken_anti_grav.is_none());
    let mut g = zero_g_game(&[("ZeroG", 0.5)]);
    g.tick(Input::NONE, &mut WithGraphics);
    assert!((g.world.realized(0).unwrap().gravity - 0.575).abs() < 1e-6);
}

/// A room with both effects gets one `AntiGravity`.
#[test]
fn both_effects_make_one_object() {
    let g = zero_g_game(&[("ZeroG", 0.2), ("BrokenZeroG", 1.0)]);
    let r = g.world.realized(0).unwrap();
    assert!(r.anti_gravity.is_some());
    assert!(g.world.rain_cycle.broken_anti_grav.is_some());
}

/// A `BrokenZeroG` room makes the world's switch on load (one draw), flips it on the first
/// tick with the switch sound for the camera's room at the amount as its volume, and the
/// room's power follows the switch's lights.
#[test]
fn broken_zero_g_switches_on_the_first_tick() {
    let mut g = zero_g_game(&[("BrokenZeroG", 0.5)]);
    let b = g.world.rain_cycle.broken_anti_grav.expect("switch");
    assert_eq!((b.counter, b.progress), (0, 0.0));
    let was_on = b.on;
    g.tick(Input::NONE, &mut WithGraphics);
    let b = g.world.rain_cycle.broken_anti_grav.expect("switch");
    assert_eq!(b.on, !was_on);
    assert!((320..720).contains(&(b.counter + 1)));
    let want = if b.on {
        SoundId::BrokenAntiGravitySwitchOn
    } else {
        SoundId::BrokenAntiGravitySwitchOff
    };
    let heard: Vec<_> = g
        .sounds
        .events()
        .iter()
        .filter_map(|e| match e {
            SoundEvent::Play(p) if p.id == want => Some(p.emitter_volume),
            _ => None,
        })
        .collect();
    assert_eq!(heard, vec![0.5]);
    let r = g.world.realized(0).unwrap();
    assert_eq!(r.electric_power(&g.world.rain_cycle), b.lights);
}

/// A room without `BrokenZeroG` has full power whatever the switch does.
#[test]
fn power_is_full_without_broken_zero_g() {
    let g = zero_g_game(&[("ZeroG", 1.0)]);
    assert_eq!(
        g.world
            .realized(0)
            .unwrap()
            .electric_power(&g.world.rain_cycle),
        1.0
    );
}

/// `SSMusicTrigger` asks for the song only while the room's gravity is 0 (the trigger is
/// newer than the `AntiGravity` here, so it reads last tick's gravity: no request on the
/// first tick).
#[test]
fn ss_music_asks_only_in_zero_gravity() {
    use crate::trigger::MusicRequest;
    let mut g = zero_g_game(&[("ZeroG", 1.0), ("SSMusic", 0.3)]);
    g.tick(Input::NONE, &mut WithGraphics);
    assert!(g.take_music_requests().is_empty());
    g.tick(Input::NONE, &mut WithGraphics);
    assert_eq!(
        g.take_music_requests(),
        vec![MusicRequest::SsSong { amount: 0.3 }]
    );
    let mut g = zero_g_game(&[("ZeroG", 0.5), ("SSMusic", 0.3)]);
    for _ in 0..3 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    assert!(g.take_music_requests().is_empty());
}

/// `PlaceInRoom`'s `SetRandomSpin` reads the room's gravity at that moment, before the
/// object's first update: a spear placed in a zero-gravity room spins at 0.05 of its speed.
#[test]
fn a_spear_placed_in_a_zero_g_room_spins_with_its_gravity() {
    use crate::creature::{EntityId, WorldCoordinate};
    use crate::item::AbstractObject;
    let mut g = zero_g_game(&[("ZeroG", 1.0)]);
    g.tick(Input::NONE, &mut WithGraphics);
    assert_eq!(g.world.realized(0).unwrap().gravity, 0.0);
    let id = g.world.add_object(AbstractObject::new_spear(
        EntityId::new(0, 77),
        WorldCoordinate::new(0, 8, 1, -1),
        false,
    ));
    g.world.realize_object_in_room(id, &mut g.rng);
    let spin = g.world.weapon_mut(id).unwrap().rotation_speed.abs();
    assert!((2.5..=7.5).contains(&spin), "{spin}");
}

/// The player reads the room's gravity as it is after the `AntiGravity` turn, not as of its
/// own last update.
#[test]
fn the_player_sees_the_gravity_the_anti_gravity_set() {
    let mut g = zero_g_game(&[("ZeroG", 1.0)]);
    assert_eq!(g.player_ref().effective_room_gravity(), 1.0);
    g.tick(Input::NONE, &mut WithGraphics);
    assert_eq!(g.player_ref().effective_room_gravity(), 0.0);
}

/// `SS_E08GradientGravity`: `InverseLerp(700, PixelHeight - 400, y)` of 3780 px: 0 up to
/// y 700, 1 from 3380.
#[test]
fn the_gradient_follows_the_formula() {
    use crate::room_script::SsE08GradientGravity as G;
    assert_eq!(G::gravity_at(0.0, 3780.0), 0.0);
    assert_eq!(G::gravity_at(700.0, 3780.0), 0.0);
    assert_eq!(G::gravity_at(3380.0, 3780.0), 1.0);
    assert_eq!(G::gravity_at(3700.0, 3780.0), 1.0);
    assert!((G::gravity_at(3000.0, 3780.0) - 2300.0 / 2680.0).abs() < 1e-6);
}

/// The script is added only to SS_E08 with the room setting on, and sets the room's gravity
/// from the player's first chunk after the player's own turn; the player reads it on its
/// next turn.
#[test]
fn the_script_sets_the_gravity_from_the_players_height() {
    let mut g = gradient_game();
    assert_eq!(g.world.realized(0).unwrap().scripts.len(), 1);
    assert!(
        zero_g_game(&[])
            .world
            .realized(0)
            .unwrap()
            .scripts
            .is_empty()
    );
    g.set_player(
        crate::Vec2::new(100.0, 2000.0),
        crate::Vec2::ZERO,
        &mut WithGraphics,
    );
    let mut last = 1.0;
    for _ in 0..3 {
        g.tick(Input::NONE, &mut WithGraphics);
        let y = g.player_ref().body.chunks[0].pos.y;
        let now = g.world.realized(0).unwrap().gravity;
        assert_eq!(now, (y - 700.0) / (3380.0 - 700.0));
        assert_eq!(g.player_ref().effective_room_gravity(), last);
        last = now;
    }
    assert!(last > 0.0 && last < 1.0);
}

/// Below y 700 the room is weightless (the player floats in zero-g mode); above 3380 it is
/// at full gravity.
#[test]
fn the_ends_of_the_gradient_are_zero_and_full_gravity() {
    let mut g = gradient_game();
    g.set_player(
        crate::Vec2::new(100.0, 500.0),
        crate::Vec2::ZERO,
        &mut WithGraphics,
    );
    for _ in 0..4 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    assert_eq!(g.world.realized(0).unwrap().gravity, 0.0);
    assert_eq!(g.player_ref().effective_room_gravity(), 0.0);
    assert_eq!(g.player_ref().body_mode(), crate::player::BodyMode::ZeroG);
    g.set_player(
        crate::Vec2::new(100.0, 3500.0),
        crate::Vec2::ZERO,
        &mut WithGraphics,
    );
    for _ in 0..3 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    assert_eq!(g.world.realized(0).unwrap().gravity, 1.0);
    assert_eq!(g.player_ref().effective_room_gravity(), 1.0);
}

/// A spear in the room is scaled by the same room gravity, from the turn after the script
/// set it (the spear is newer than the script, so it updates first).
#[test]
fn a_spear_follows_the_gradient_gravity() {
    use crate::creature::{EntityId, WorldCoordinate};
    use crate::item::AbstractObject;
    let mut g = gradient_game();
    g.set_player(
        crate::Vec2::new(100.0, 2000.0),
        crate::Vec2::ZERO,
        &mut WithGraphics,
    );
    let id = g.world.add_object(AbstractObject::new_spear(
        EntityId::new(0, 77),
        WorldCoordinate::new(0, 30, 100, -1),
        false,
    ));
    g.world.realize_object_in_room(id, &mut g.rng);
    g.tick(Input::NONE, &mut WithGraphics);
    let set = g.world.realized(0).unwrap().gravity;
    assert!(set > 0.0 && set < 1.0);
    g.tick(Input::NONE, &mut WithGraphics);
    let seen = g
        .world
        .object(id)
        .realized
        .as_deref()
        .unwrap()
        .body
        .params
        .room_gravity;
    assert_eq!(seen, set);
}
