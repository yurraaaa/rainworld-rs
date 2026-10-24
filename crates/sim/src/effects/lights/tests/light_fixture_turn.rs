//! Placed light fixtures and the tile parse's waterfalls in one test room: their places in
//! the update list, the lights they make and the draws of a tick (`Room.Loaded`,
//! `Redlight.Update`, `HolyFire.Update`, `WaterFall.Update`).
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, PlacedObject, Room, RoomEffect, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::game::RoomObject;
use crate::superstructure::electric::room_fixtures::FixtureRef;
use crate::tests::room_with_water;
use crate::world::World;
use crate::{Input, LightElement, LightTie, RainWorldGame, Sounds, Start, Vec2};

use crate::effects::lights::turn::tests::WithGraphics;

/// A 20 x 10 room with a floor, water at tile row `water` (-1 for none) and, when
/// `fall`, a two-high waterfall of two tiles at x 8..10 from the top; the player at (2, 1).
fn fixture_game(placed: &str, water: i32, fall: bool, ticks: usize) -> RainWorldGame {
    fixture_game_with(placed, &[], water, fall, ticks)
}

fn fixture_game_with(
    placed: &str,
    effects: &[(&str, f32)],
    water: i32,
    fall: bool,
    ticks: usize,
) -> RainWorldGame {
    let mut rows: Vec<String> = vec![".".repeat(20); 10];
    rows[9] = "#".repeat(20);
    if fall {
        for row in [1, 2] {
            rows[row].replace_range(8..10, "ww");
        }
    }
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    game_in(room_with_water(&refs, water), placed, effects, ticks)
}

fn game_in(room: Room, placed: &str, effects: &[(&str, f32)], ticks: usize) -> RainWorldGame {
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
            tile: Some((2, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
    );
    for _ in 0..ticks {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    g
}

const RED: &str = "LightFixture><300><100><RedLight~17.32051~-10~79";
const FIRE: &str = "LightFixture><200><100><HolyFire~17.32051~-10~65";
const SLIME: &str = "LightFixture><100><100><SlimeMoldLight~17.32051~-10~30";

/// `Room.Loaded` makes each fixture where the settings list it: the red light's flat light
/// first, then the fixture; the holy fire's three lights, its flat light, then the fixture;
/// a slime mold light just itself, a `DeepProcessingLight` its light then itself. Unregistered
/// types make nothing (the `ZapCoilLight` is in `superstructure/electric/tests/turn.rs`).
#[test]
fn fixtures_join_the_update_list_in_order() {
    let placed = format!(
        "{RED}, {FIRE}, {SLIME}, LightFixture><50><50><LightRod~1~2~30, \
         LightFixture><60><50><DeepProcessing~1~2~30"
    );
    let g = fixture_game(&placed, -1, false, 0);
    let r = g.world.realized(0).expect("room");
    let f = &r.fixtures;
    assert_eq!(
        (f.redlights.len(), f.holy_fires.len(), f.slime_lights.len()),
        (1, 1, 1)
    );
    let (red, fire, slime) = (&f.redlights[0], &f.holy_fires[0], &f.slime_lights[0]);
    assert_eq!(r.light_sources.len(), 1 + 4 + 1);
    let ids: Vec<u64> = r.light_sources.iter().map(|l| l.id).collect();
    // flat, [fixture], 3 lights, flat, [fixture], [slime]
    assert_eq!(red.flat_light, ids[0]);
    assert_eq!(red.update_order, ids[0] + 1);
    assert_eq!(fire.lights, [ids[1], ids[2], ids[3]]);
    assert_eq!(fire.flat_light, ids[4]);
    assert_eq!(fire.update_order, ids[4] + 1);
    assert!(slime.update_order > fire.update_order);
    let deep = &f.deep_processing_lights[0];
    assert_eq!(deep.light, ids[5]);
    assert_eq!(deep.update_order, ids[5] + 1);
    assert!(deep.update_order > slime.update_order);
    assert!(r.light_sources.iter().all(|l| !l.environmental_light));
    assert_eq!(
        r.light_sources[0].tied_to,
        Some(LightTie::Fixture(red.update_order))
    );
    assert_eq!(r.light_sources[0].color, [1.0, 0.05, 0.05]);
    assert!(red.light.is_none());
    let seqs: Vec<u64> = g.room_fixture_objects(0).iter().map(|&(s, _)| s).collect();
    assert_eq!(
        seqs,
        [
            red.update_order,
            fire.update_order,
            slime.update_order,
            deep.update_order
        ]
    );
}

/// The first update makes the `RedlightLight` (of the `LightMask0` element) at the end of
/// the update list and gives both lights their sets; the light is made after the
/// objects that already exist.
#[test]
fn redlight_makes_its_light_on_the_first_update() {
    let mut g = fixture_game(RED, -1, false, 0);
    let seq_before = g.world.update_seq();
    g.tick(Input::NONE, &mut WithGraphics);
    let r = g.world.realized(0).expect("room");
    let red = &r.fixtures.redlights[0];
    let id = red.light.expect("made");
    assert!(id > seq_before);
    let main = r.light_sources.iter().find(|l| l.id == id).expect("light");
    assert_eq!(main.element, LightElement::LightMask0);
    assert_eq!(main.tied_to, Some(LightTie::Fixture(red.update_order)));
    // After its second update the sets have landed: the alpha near 1 and the rad `0.16 *`
    // 290..310.
    g.tick(Input::NONE, &mut WithGraphics);
    let r = g.world.realized(0).expect("room");
    let main = r.light_sources.iter().find(|l| l.id == id).expect("light");
    assert!(main.alpha > 0.0 && main.alpha <= 1.0, "{}", main.alpha);
    assert!(
        (0.16 * 290.0..=0.16 * 310.0).contains(&main.rad),
        "{}",
        main.rad
    );
    let flat = &r.light_sources[0];
    assert_eq!(flat.pos, Vec2::new(300.0, 100.0));
}

/// A `BrokenAntiGravity` that stays in one state: `on` with full lights, or off.
fn steady_switch(on: bool) -> crate::BrokenAntiGravity {
    let to = if on { 1.0 } else { 0.0 };
    crate::BrokenAntiGravity {
        counter: 100_000,
        on,
        from: to,
        to,
        progress: 1.0,
        lights: to,
        lights_get_to: to,
        cycle_min: 8,
        cycle_max: 18,
    }
}

/// A red light in a room with `BrokenZeroG` and a positive seed is gravity-dependent and
/// starts unpowered; with the electric power at 1 (the switch on) it stays dark. A seed
/// of 0 is an ordinary red light.
#[test]
fn gravity_dependent_redlight_stays_dark_with_full_power() {
    let mut g = fixture_game_with(RED, &[("BrokenZeroG", 0.5)], -1, false, 0);
    g.world.rain_cycle.broken_anti_grav = Some(steady_switch(true));
    let r = g.world.realized(0).expect("room");
    let red = &r.fixtures.redlights[0];
    assert!(red.gravity_dependent && !red.powered);
    assert_eq!(r.electric_power(&g.world.rain_cycle), 1.0);
    g.tick(Input::NONE, &mut WithGraphics);
    g.tick(Input::NONE, &mut WithGraphics);
    let r = g.world.realized(0).expect("room");
    let main = r
        .light_sources
        .iter()
        .find(|l| Some(l.id) == r.fixtures.redlights[0].light);
    assert_eq!(main.map(|l| l.alpha), Some(0.0));
    assert_eq!(r.light_sources[0].alpha, 0.0);
    assert!(!r.fixtures.redlights[0].powered);
    let plain = "LightFixture><300><100><RedLight~17.32051~-10~0";
    let g = fixture_game_with(plain, &[("BrokenZeroG", 0.5)], -1, false, 0);
    let red = &g.world.realized(0).expect("room").fixtures.redlights[0];
    assert!(!red.gravity_dependent && red.powered);
}

/// With the power off (the switch off, lights 0) a gravity-dependent red light powers up
/// by chance (`NoElectricity > Lerp(0.65, 0.95, seed / 100)` and `value < 1 / Lerp(20, 80,
/// ..)`) and then shines at the lack of power.
#[test]
fn gravity_dependent_redlight_lights_when_power_is_off() {
    let mut g = fixture_game_with(RED, &[("BrokenZeroG", 0.5)], -1, false, 0);
    g.world.rain_cycle.broken_anti_grav = Some(steady_switch(false));
    assert_eq!(
        g.world
            .realized(0)
            .expect("room")
            .electric_power(&g.world.rain_cycle),
        0.0
    );
    let mut ticks = 0;
    while !g.world.realized(0).expect("room").fixtures.redlights[0].powered {
        g.tick(Input::NONE, &mut WithGraphics);
        ticks += 1;
        assert!(ticks < 3000, "never powered");
    }
    g.tick(Input::NONE, &mut WithGraphics);
    let r = g.world.realized(0).expect("room");
    let main = r
        .light_sources
        .iter()
        .find(|l| Some(l.id) == r.fixtures.redlights[0].light)
        .expect("light");
    assert!(main.alpha > 0.0 || r.fixtures.redlights[0].flicker > 0);
}

/// A holy fire sheds one ember per tick, each at the fixture's position, in the room's
/// update list at the end; three draws a tick for it besides the lights' draws.
#[test]
fn holy_fire_sheds_an_ember_a_tick() {
    let mut g = fixture_game(FIRE, -1, false, 0);
    let r = g.world.realized(0).expect("room");
    assert!(r.fixtures.holy_sprites.is_empty());
    for n in 1..=3 {
        g.tick(Input::NONE, &mut WithGraphics);
        let r = g.world.realized(0).expect("room");
        assert_eq!(r.fixtures.holy_sprites.len(), n);
        let s = r.fixtures.holy_sprites.last().expect("sprite");
        assert!(s.update_order > r.fixtures.holy_fires[0].update_order);
        assert_eq!(s.last_pos, Vec2::new(200.0, 100.0));
    }
    let fire = g.world.realized(0).expect("room").fixtures.holy_fires[0].update_order;
    assert!(
        g.room_fixture_objects(0)
            .iter()
            .any(|&(s, o)| s == fire && o == RoomObject::Fixture(FixtureRef::HolyFire(fire)))
    );
    // The embers burn out (life time at most 40 ticks) and leave the list.
    for _ in 0..45 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    let r = g.world.realized(0).expect("room");
    assert!(r.fixtures.holy_sprites.len() <= 41);
    assert!(r.fixtures.holy_sprites.iter().all(|s| !s.slated));
}

/// The waterfall comes first in the update list, before the fixtures and before anything
/// `Room.Loaded` adds; with water in the room it is connected to it and strikes at the
/// still level less 10.
#[test]
fn waterfall_is_first_in_the_update_list() {
    let g = fixture_game(RED, 3, true, 0);
    let r = g.world.realized(0).expect("room");
    assert_eq!(r.fixtures.waterfalls.len(), 1);
    let w = &r.fixtures.waterfalls[0];
    assert_eq!((w.tile_pos, w.width, w.flow), ((8, 8), 2, 1.0));
    assert!(w.update_order < r.fixtures.redlights[0].update_order);
    assert!(w.update_order < r.light_sources[0].id);
    assert_eq!(w.bubbles_per_tile, 10);
    assert_eq!(crate::WaterFall::strike_level(r.water.as_ref()), 60.0);
}

/// Over a few ticks the fall's bottom drops to the strike level, it throws drips and
/// bubbles into the room, and the surface under it is kicked.
#[test]
fn waterfall_runs_in_a_room_with_water() {
    let mut g = fixture_game("", 3, true, 0);
    g.tick_until_ready(&mut ());
    for _ in 0..40 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    let r = g.world.realized(0).expect("room");
    let w = &r.fixtures.waterfalls[0];
    assert_eq!(w.bottom_pos[0], 60.0);
    assert!(w.visual_density > 0.9, "{}", w.visual_density);
    // Drips and bubbles it made: water-coloured drips and air bubbles are in the room.
    assert!(r.drips.iter().any(|d| d.water_color), "no drips");
    assert!(!r.cosmetics.is_empty(), "no bubbles");
    let kicked = (0..r.water.as_ref().expect("water").point_count())
        .any(|i| r.water.as_ref().expect("water").wave_height(i) != 0.0);
    assert!(kicked);
}

/// A waterfall in a room with no water: no bubbles, the fall reaches -200 and throws drips
/// off the floor.
#[test]
fn waterfall_in_a_dry_room() {
    let mut g = fixture_game("", -1, true, 0);
    for _ in 0..40 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    let r = g.world.realized(0).expect("room");
    let w = &r.fixtures.waterfalls[0];
    assert_eq!((w.bubbles.len(), w.bottom_pos[0]), (0, -200.0));
    assert!(r.drips.iter().any(|d| d.water_color));
}
