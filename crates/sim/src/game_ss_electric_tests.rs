//! The superstructure's electric objects in one test room: `Room.Loaded` making them in the
//! settings' order with their draws, the coil's zap killing the player and flashing, the
//! light finding its coil, the fuses running only where a camera sees them.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, PlacedObject, Room, RoomEffect, RoomSettings};
use rw_random::UnityRandom;

use super::super::lights::tests::WithGraphics;
use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::room_fixtures::FixtureRef;
use crate::tests::room_with_water;
use crate::world::World;
use crate::{Input, LightTie, RainWorldGame, Sounds, Start, Vec2};

/// A 20 x 10 room with three rows of floor, the player standing on it at (2, 3).
fn electric_game(placed: &str, effects: &[(&str, f32)], ticks: usize) -> RainWorldGame {
    let mut rows: Vec<String> = vec![".".repeat(20); 10];
    for row in &mut rows[7..] {
        *row = "#".repeat(20);
    }
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let room: Room = room_with_water(&refs, -1);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        placed_objects: PlacedObject::parse_list(placed),
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
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((2, 3)),
        },
        UnityRandom::new(5),
        Sounds::default(),
        &mut WithGraphics,
    );
    for _ in 0..ticks {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    g
}

const ROD: &str = "SSLightRod><100><100><0~0~0.2~90~100~0.5";
const COIL: &str = "ZapCoil><0><0><180~40";
const FUSES: &str = "SuperStructureFuses><200><100><40~20";
const COIL_LIGHT: &str = "LightFixture><60><30><ZapCoilLight~1~2~30";

/// `Room.Loaded` makes each where the settings list it: a rod's lights (one for each 45
/// px: two), then the rod; the coil; the fuses; a coil light's light, then the light, which
/// `AIMapReady` ties to the first coil holding it. The rod's constructor makes six draws.
#[test]
fn room_loaded_makes_them_in_settings_order() {
    let placed = format!("{ROD}, {COIL}, {FUSES}, {COIL_LIGHT}");
    let g = electric_game(&placed, &[], 0);
    let r = g.world.realized(0).expect("room");
    let f = &r.fixtures;
    let (rod, coil, fuses, light) = (
        &f.light_rods[0],
        &f.zap_coils[0],
        &f.fuses[0],
        &f.zap_coil_lights[0],
    );
    assert_eq!(rod.lights.len(), 2);
    let ids: Vec<u64> = r.light_sources.iter().map(|l| l.id).collect();
    assert_eq!(ids.len(), 3);
    assert_eq!([rod.lights[0].light, rod.lights[1].light], [ids[0], ids[1]]);
    assert_eq!(rod.update_order, ids[0] + 2);
    assert_eq!(coil.update_order, rod.update_order + 1);
    assert_eq!(fuses.update_order, coil.update_order + 1);
    assert_eq!(light.light, ids[2]);
    assert_eq!(light.update_order, ids[2] + 1);
    assert_eq!(ids[2], fuses.update_order + 1);
    assert_eq!(light.coil, Some(coil.update_order));
    assert_eq!(
        r.light_sources[0].tied_to,
        Some(LightTie::Fixture(rod.update_order))
    );
    assert_eq!(coil.rect, [0, 0, 9, 2]);
    assert!(coil.horizontal);
    // Fuses: from (200, 100) by (40, 20) is tiles 10..12 by 5..6; 20 px a tile over the
    // default 10 px cell; depth 2 (the open room).
    assert_eq!(fuses.rect, [10, 5, 12, 6]);
    assert_eq!((fuses.width, fuses.height), (4, 2));
    assert_eq!((fuses.size, fuses.depth, fuses.broken), (10.0, 2, 0.0));
    let seqs: Vec<(u64, FixtureRef)> = r.fixtures.objects();
    assert_eq!(
        seqs.iter().map(|&(_, f)| f).collect::<Vec<_>>(),
        [
            FixtureRef::LightRod(rod.update_order),
            FixtureRef::ZapCoil(coil.update_order),
            FixtureRef::ZapCoilLight(light.update_order),
            FixtureRef::Fuses(fuses.update_order),
        ]
    );
    // Six draws for the rod (a vessel's speed, size and progression) and none for the rest.
    let without = electric_game(&format!("{COIL}, {FUSES}, {COIL_LIGHT}"), &[], 0);
    let mut shifted = without.rng.clone();
    for _ in 0..6 {
        shifted.value();
    }
    assert_eq!(g.rng.state(), shifted.state());
}

/// A coil under a player standing on it kills it: the zap makes a flash (with a light on
/// its first update), the player dies, and the coil lights up.
#[test]
fn a_coil_under_the_player_zaps_and_kills() {
    let mut g = electric_game(COIL, &[], 0);
    assert!(!g.player.as_ref().expect("player").is_dead());
    let mut ticks = 0;
    while !g.player.as_ref().expect("player").is_dead() {
        g.tick(Input::NONE, &mut WithGraphics);
        ticks += 1;
        assert!(ticks < 60, "never zapped");
    }
    let r = g.world.realized(0).expect("room");
    let flash = r.fixtures.zap_flashes.first().expect("a flash");
    assert!(flash.light.is_none() || r.light_sources.iter().any(|l| Some(l.id) == flash.light));
    assert!(r.fixtures.zap_coils[0].zap_lit > 0.0);
    // The flash is at the player's foot, on the coil's face.
    assert!(flash.pos.y < 3.0 * 20.0 + 20.0);
    g.tick(Input::NONE, &mut WithGraphics);
    let r = g.world.realized(0).expect("room");
    let flash = &r.fixtures.zap_flashes[0];
    let light = flash.light.expect("made on the first update");
    let l = r.light_sources.iter().find(|l| l.id == light).expect("light");
    assert_eq!(l.tied_to, Some(LightTie::Fixture(flash.update_order)));
    assert!(flash.update_order < light);
}

/// A flash and its light go together a few ticks after the zap.
#[test]
fn the_flash_and_its_light_burn_out() {
    let mut g = electric_game(COIL, &[], 0);
    let mut ticks = 0;
    while g.world.realized(0).expect("room").fixtures.zap_flashes.is_empty() {
        g.tick(Input::NONE, &mut WithGraphics);
        ticks += 1;
        assert!(ticks < 60);
    }
    let order = g.world.realized(0).expect("room").fixtures.zap_flashes[0].update_order;
    for _ in 0..12 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    let r = g.world.realized(0).expect("room");
    assert!(
        r.fixtures
            .zap_flashes
            .iter()
            .all(|f| f.update_order != order || f.slated)
    );
}

/// A coil off its mark does nothing to a body off its face: the player standing 3 tiles
/// from the coil's rectangle is not zapped.
#[test]
fn a_far_coil_zaps_nothing() {
    let mut g = electric_game("ZapCoil><200><0><80~40", &[], 0);
    for _ in 0..40 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    assert!(!g.player.as_ref().expect("player").is_dead());
    assert!(g.world.realized(0).expect("room").fixtures.zap_flashes.is_empty());
}

/// The coil light follows its coil: its set alpha is `num^0.8` with `num` near the coil's
/// `turnedOn` once the coil has taken its first update.
#[test]
fn the_coil_light_follows_the_coil() {
    let placed = format!("{COIL}, {COIL_LIGHT}");
    let mut g = electric_game(&placed, &[], 0);
    // Keep the player clear of the coil.
    g.world.realized_mut(0).expect("room").fixtures.zap_coils[0].turned_on = 0.0;
    g.tick(Input::NONE, &mut WithGraphics);
    g.tick(Input::NONE, &mut WithGraphics);
    let r = g.world.realized(0).expect("room");
    let l = r
        .light_sources
        .iter()
        .find(|l| l.id == r.fixtures.zap_coil_lights[0].light)
        .expect("light");
    assert_eq!(l.pos, Vec2::new(60.0, 30.0));
    assert!(l.alpha >= 0.0 && l.alpha <= 1.0);
}

/// The fuses update while a camera sees one of their corners (the camera is in the room)
/// and not otherwise; the rod's lights set their sets.
#[test]
fn the_fuses_run_where_a_camera_sees_them() {
    let placed = format!("{ROD}, {FUSES}");
    let mut g = electric_game(&placed, &[], 0);
    for _ in 0..20 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    let r = g.world.realized(0).expect("room");
    let f = &r.fixtures.fuses[0];
    assert!(!f.culled);
    let moved = (0..f.width)
        .flat_map(|j| (0..f.height).map(move |k| (j, k)))
        .any(|(j, k)| f.cell(j, k)[0] != 0.0);
    assert!(moved, "no cell moved in 20 ticks");
    let rod = &r.fixtures.light_rods[0];
    let lit = r
        .light_sources
        .iter()
        .find(|l| l.id == rod.lights[0].light)
        .expect("light");
    assert!(lit.rad > 0.0 || rod.lights[0].visible > 0.0);
    // Another room's camera culls them.
    let mut g = electric_game(&placed, &[], 0);
    g.set_camera_view(0, 0);
    g.camera_view.room = 7;
    for _ in 0..5 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    let f = &g.world.realized(0).expect("room").fixtures.fuses[0];
    assert!(f.culled);
    assert_eq!(f.cell(0, 0), [0.0; 5]);
}

/// A single-room world has no region, so the fuses take the room's `CorruptionSpores`
/// amount; the region rule itself is `SuperStructureFuses::broken_for`'s test.
#[test]
fn broken_is_the_effect_without_a_region() {
    let g = electric_game(FUSES, &[("CorruptionSpores", 0.4)], 0);
    let f = &g.world.realized(0).expect("room").fixtures.fuses[0];
    assert_eq!(f.broken, 0.4);
}
