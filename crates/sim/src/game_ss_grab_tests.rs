//! The superstructure's screen-grab objects in one test room: `Room.Loaded` making them in
//! the settings' order with their draws, the disruptor's swirl over the player's chunks
//! and the room's specks, its power following the broken anti-gravity, the light following
//! the electric power.
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
use crate::{ChunkOwner, FloatExt, Input, LightTie, RainWorldGame, Sounds, Start, Vec2};

/// A 40 x 20 room with three rows of floor, the player standing on it at (2, 3).
fn grab_game(placed: &str, effects: &[(&str, f32)], ticks: usize) -> RainWorldGame {
    let mut rows: Vec<String> = vec![".".repeat(40); 20];
    for row in &mut rows[17..] {
        *row = "#".repeat(40);
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

const DEEP: &str = "DeepProcessing><100><100><0~100~200~100~200~0~0~0~0.25~0.9~0.5";
const DISRUPTOR: &str = "GravityDisruptor><300><200><";
const LIGHT: &str = "LightFixture><60><30><DeepProcessing~1~2~25";

/// `Room.Loaded` makes each where the settings list it: the quad; the disruptor's twenty
/// specks (consecutive places), then the disruptor; the light's light, then the light. The
/// disruptor makes 98 draws and the rest none; its depth is 2 in the open.
#[test]
fn room_loaded_makes_them_in_settings_order() {
    let placed = format!("{DEEP}, {DISRUPTOR}, {LIGHT}");
    let g = grab_game(&placed, &[], 0);
    let r = g.world.realized(0).expect("room");
    let f = &r.fixtures;
    let (deep, disruptor, light) = (
        &f.deep_processing[0],
        &f.gravity_disruptors[0],
        &f.deep_processing_lights[0],
    );
    assert_eq!(f.disruptor_specks.len(), 20);
    assert_eq!(f.disruptor_specks[0].update_order, deep.update_order + 1);
    assert_eq!(f.disruptor_specks[19].update_order, deep.update_order + 20);
    assert_eq!(disruptor.update_order, deep.update_order + 21);
    assert!(
        f.disruptor_specks
            .iter()
            .all(|s| s.disruptor == disruptor.update_order)
    );
    assert_eq!(light.light, disruptor.update_order + 1);
    assert_eq!(light.update_order, light.light + 1);
    assert_eq!(disruptor.depth, 2);
    assert_eq!(
        (deep.pos, disruptor.pos),
        (Vec2::new(100.0, 100.0), Vec2::new(300.0, 200.0))
    );
    let l = r
        .light_sources
        .iter()
        .find(|l| l.id == light.light)
        .expect("light");
    assert_eq!(l.tied_to, Some(LightTie::Fixture(light.update_order)));
    let order: Vec<FixtureRef> = f.objects().into_iter().map(|(_, f)| f).collect();
    assert_eq!(order.len(), 1 + 20 + 1 + 1);
    assert_eq!(order[0], FixtureRef::DeepProcessing(deep.update_order));
    // The disruptor's 98 draws are the only ones the objects make.
    let without = grab_game(&format!("{DEEP}, {LIGHT}"), &[], 0);
    let mut shifted = without.rng.clone();
    for _ in 0..98 {
        shifted.value();
    }
    assert_eq!(g.rng.state(), shifted.state());
}

/// The disruptor's depth is read from the tile it stands on: 0 inside a wall, 2 in the open
/// here (the test room has no back walls).
#[test]
fn the_depth_is_read_from_the_tile() {
    let g = grab_game("GravityDisruptor><100><30><", &[], 0);
    let d = &g
        .world
        .realized(0)
        .expect("room")
        .fixtures
        .gravity_disruptors[0];
    assert_eq!(d.depth, 0);
}

/// One run of the disruptor pushes the player's chunks within reach: position and
/// velocity by the push, the velocity by the swirl as well.
#[test]
fn the_swirl_moves_the_players_chunks() {
    let mut g = grab_game("GravityDisruptor><100><100><", &[], 0);
    let order = g
        .world
        .realized(0)
        .expect("room")
        .fixtures
        .gravity_disruptors[0]
        .update_order;
    let d = g
        .world
        .realized(0)
        .expect("room")
        .fixtures
        .gravity_disruptors[0]
        .clone();
    let chunks: Vec<_> = (0..)
        .map_while(|i| g.electric_chunk(ChunkOwner::Player, i))
        .collect();
    assert!(chunks.len() >= 2);
    let reach = chunks
        .iter()
        .filter(|c| d.pos.dist_less(c.pos, 450.0))
        .count();
    assert!(reach >= 1, "the player stands within 450 px");
    g.run_gravity_disruptor(0, order);
    for (i, before) in chunks.iter().enumerate() {
        let after = g.electric_chunk(ChunkOwner::Player, i).expect("chunk");
        match d.chunk_push(before.pos, before.vel) {
            Some((push, swirl)) => {
                assert_eq!(after.pos, before.pos + push);
                assert_eq!(after.vel, before.vel + push + swirl);
            }
            None => assert_eq!((after.pos, after.vel), (before.pos, before.vel)),
        }
    }
}

/// Without power nothing is swirled, but the streaks still run (their draws).
#[test]
fn without_power_nothing_moves() {
    let mut g = grab_game("GravityDisruptor><100><100><", &[], 0);
    let order = g
        .world
        .realized(0)
        .expect("room")
        .fixtures
        .gravity_disruptors[0]
        .update_order;
    g.world
        .realized_mut(0)
        .expect("room")
        .fixtures
        .gravity_disruptors[0]
        .power = 0.0;
    let before = g.electric_chunk(ChunkOwner::Player, 0).expect("chunk");
    let draws_before = g.rng.clone();
    g.run_gravity_disruptor(0, order);
    let after = g.electric_chunk(ChunkOwner::Player, 0).expect("chunk");
    assert_eq!((after.pos, after.vel), (before.pos, before.vel));
    assert_ne!(g.rng.state(), draws_before.state());
}

/// The session's first room runs `Loaded`, then `NowViewed` (the constructor's
/// `MoveCamera`, which seeds the specks), then `AIMapReady`: `roomSpecks` holds the specks,
/// and the swirl moves them by the speck push. Nothing is linked before the first tick.
#[test]
fn the_swirl_moves_the_rooms_specks() {
    let g0 = grab_game("GravityDisruptor><100><100><", &[("ZeroGSpecks", 0.01)], 0);
    assert!(
        g0.world
            .realized(0)
            .expect("room")
            .fixtures
            .gravity_disruptors[0]
            .room_specks
            .is_empty()
    );
    let mut g = grab_game("GravityDisruptor><100><100><", &[("ZeroGSpecks", 0.01)], 1);
    let order = g
        .world
        .realized(0)
        .expect("room")
        .fixtures
        .gravity_disruptors[0]
        .update_order;
    let r = g.world.realized(0).expect("room");
    let n = r.particles.specks.len();
    assert_eq!(n, 10);
    assert_eq!(r.fixtures.gravity_disruptors[0].room_specks.len(), n);
    let d = r.fixtures.gravity_disruptors[0].clone();
    let before: Vec<_> = r.particles.specks.iter().map(|s| (s.pos, s.vel)).collect();
    g.run_gravity_disruptor(0, order);
    let r = g.world.realized(0).expect("room");
    let mut pushed = 0;
    for (s, (pos, vel)) in r.particles.specks.iter().zip(before) {
        match d.speck_push(pos, vel) {
            Some(push) => {
                pushed += 1;
                assert_eq!((s.pos, s.vel), (pos, vel + push));
            }
            None => assert_eq!((s.pos, s.vel), (pos, vel)),
        }
    }
    assert!(pushed > 0, "a speck lies within 450 px");
}

/// In a `BrokenZeroG` room the power is the world's `CurrentAntiGravity` after an update.
#[test]
fn the_power_follows_the_broken_anti_gravity() {
    let mut g = grab_game("GravityDisruptor><100><100><", &[("BrokenZeroG", 1.0)], 3);
    let order = g
        .world
        .realized(0)
        .expect("room")
        .fixtures
        .gravity_disruptors[0]
        .update_order;
    let current = g
        .world
        .rain_cycle
        .broken_anti_grav
        .expect("the switch")
        .current_anti_gravity();
    g.run_gravity_disruptor(0, order);
    let d = &g
        .world
        .realized(0)
        .expect("room")
        .fixtures
        .gravity_disruptors[0];
    assert_eq!(d.power, current);
}

/// A speck updates in its own slot against its disruptor's position and power.
#[test]
fn a_speck_follows_its_disruptor() {
    let mut g = grab_game("GravityDisruptor><100><100><", &[], 0);
    let r = g.world.realized(0).expect("room");
    let (order, before) = (
        r.fixtures.disruptor_specks[0].update_order,
        r.fixtures.disruptor_specks[0].clone(),
    );
    g.world
        .realized_mut(0)
        .expect("room")
        .fixtures
        .gravity_disruptors[0]
        .power = 0.0;
    g.run_disruptor_speck(0, order);
    let s = &g.world.realized(0).expect("room").fixtures.disruptor_specks[0];
    assert_eq!(s.last_pos, before.pos);
    assert_eq!(s.vel, crate::Vec2::ZERO);
}

/// The light sets its rad from the room's electric power (1 here) and alpha to half of it.
#[test]
fn the_light_follows_the_electric_power() {
    let mut g = grab_game(LIGHT, &[], 0);
    g.tick(Input::NONE, &mut WithGraphics);
    let r = g.world.realized(0).expect("room");
    let light = &r.fixtures.deep_processing_lights[0];
    let l = r
        .light_sources
        .iter()
        .find(|l| l.id == light.light)
        .expect("light");
    assert_eq!(l.pos, Vec2::new(60.0, 30.0));
    let top = 80.0f32.lerp(800.0, 0.25);
    assert!(l.rad > 0.0 && l.rad <= top * 1.1 + 1.0, "rad {}", l.rad);
    assert!(l.alpha <= 0.5 + 1e-3);
}

/// The quad's power chases the electric power: 1 in an ordinary room, so it stays.
#[test]
fn the_quad_keeps_full_power_in_a_powered_room() {
    let mut g = grab_game(DEEP, &[], 0);
    for _ in 0..30 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    assert_eq!(
        g.world.realized(0).expect("room").fixtures.deep_processing[0].power,
        1.0
    );
}

/// A second camera move before the first tick replaces the first (`loadingRoom` is
/// overwritten): one `NowViewed` (the room the last move named, here the same room), one
/// `AIMapReady` link; the room an overwritten move named gets neither.
#[test]
fn a_second_move_before_the_first_tick_replaces_the_first() {
    let mut g = grab_game("GravityDisruptor><100><100><", &[("ZeroGSpecks", 0.01)], 0);
    g.camera_moves_to(0, 0, &mut WithGraphics);
    g.camera_moves_to(0, 0, &mut WithGraphics);
    let room = g.world.realized(0).expect("room");
    assert!(
        room.particles.specks.is_empty(),
        "nothing before the first tick"
    );
    g.tick(Input::NONE, &mut WithGraphics);
    let room = g.world.realized(0).expect("room");
    assert_eq!(room.particles.specks.len(), 10);
    assert_eq!(room.fixtures.gravity_disruptors[0].room_specks.len(), 10);
}
