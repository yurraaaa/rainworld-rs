//! A green lizard's graphics module on SU_A07 of the installed game (skipped without
//! it): built in the viewed room, its legs feed `LegsGripping`; none in a room nobody
//! views.

use rw_assets::{Room, Terrain};

use super::Session;
use super::params::{InputParam, SpawnCreatureParams, StepParams};
use super::reply::{LizardInfo, State};
use rw_graphics::{Color, LIZARD_BASE_SPRITES, LizardGraphics, LizardReading};
use rw_random::UnityRandom;
use rw_sim::{CosmeticKind, Input, Spark, StationaryEffect, Vec2};

/// The first air tile with air above and solid ground below, from the bottom left.
trait GroundTile {
    fn ground_tile(&self) -> (i32, i32);
}

impl GroundTile for Room {
    fn ground_tile(&self) -> (i32, i32) {
        (2..self.height())
            .flat_map(|y| (2..self.width() - 2).map(move |x| (x, y)))
            .find(|&(x, y)| {
                self.get(x, y).is_some_and(|t| t.terrain == Terrain::Air)
                    && self
                        .get(x, y + 1)
                        .is_some_and(|t| t.terrain == Terrain::Air)
                    && self
                        .get(x, y - 1)
                        .is_some_and(|t| t.terrain == Terrain::Solid)
            })
            .expect("a ground tile")
    }
}

impl Session {
    /// SU_A07 (seed 1) with a green lizard spawned on `room`'s ground tile (the
    /// camera's room by default); its EntityID.
    fn with_green_lizard(room: Option<String>) -> Option<(Self, [i32; 2])> {
        let mut s = Self::installed()?;
        s.load("SU_A07", 1);
        let tile = match &room {
            None => s.scene_mut().expect("game").game().room().ground_tile(),
            Some(name) => {
                let world = s.scene_mut().expect("game").game().sim().world();
                let index = world.room_index(name).expect("room");
                world.realized(index).expect("realized").room.ground_tile()
            }
        };
        let reply = s
            .spawn_creature(&SpawnCreatureParams {
                kind: "GreenLizard".to_owned(),
                room,
                tile: [tile.0, tile.1],
                ..Default::default()
            })
            .expect("spawns");
        Some((s, reply.id))
    }

    fn lizard(state: &State, id: [i32; 2]) -> LizardInfo {
        state
            .creatures
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| c.lizard.clone())
            .expect("realized lizard")
    }

    fn lizard_ticks(&mut self, ticks: u32) -> State {
        self.step(&StepParams {
            ticks: Some(ticks),
            input: Some(InputParam::default()),
        })
        .expect("steps")
    }
}

/// In the viewed room the lizard gets graphics with four legs, and within a few
/// seconds on the ground its legs hold terrain: `LegsGripping` is the limbs' count.
#[test]
fn a_viewed_lizards_legs_grip_the_ground() {
    let Some((mut s, id)) = Session::with_green_lizard(None) else {
        return;
    };
    let mut gripped = false;
    for _ in 0..40 {
        let state = s.lizard_ticks(5);
        let l = Session::lizard(&state, id);
        let g = l.graphics.expect("graphics in the viewed room");
        assert_eq!(g.limbs.len(), 4);
        assert_eq!(l.movement.legs_gripping, g.legs_grabbing);
        assert!(g.legs_grabbing <= 4);
        let counted = g.limbs.iter().filter(|limb| limb.grip_counter >= 1).count();
        assert!(usize::try_from(g.legs_grabbing).unwrap_or(9) <= counted);
        gripped |= g.legs_grabbing > 0;
    }
    assert!(gripped, "no leg ever held the ground");
}

/// In a realized room nobody views there is no module: `LegsGripping` stays 2.
#[test]
fn an_unviewed_lizard_has_no_graphics() {
    let Some(mut s) = Session::installed() else {
        return;
    };
    s.load("SU_A07", 1);
    let state = s.lizard_ticks(1);
    let Some(other) = state
        .realized_rooms
        .iter()
        .map(|r| r.room.clone())
        .find(|r| *r != state.room)
    else {
        return;
    };
    drop(s);
    let Some((mut s, id)) = Session::with_green_lizard(Some(other)) else {
        return;
    };
    for _ in 0..4 {
        let state = s.lizard_ticks(10);
        let l = Session::lizard(&state, id);
        assert!(l.graphics.is_none());
        assert_eq!(l.movement.legs_gripping, 2);
        assert_eq!(l.movement.no_grip_counter, 0);
    }
}

impl Session {
    /// The lizard's graphics module, as the game keeps it.
    fn lizard_module(&mut self, id: [i32; 2]) -> LizardGraphics {
        self.scene_mut()
            .expect("game")
            .game()
            .lizard_graphics()
            .iter()
            .find(|g| [g.id.spawner, g.id.number] == id)
            .cloned()
            .expect("module")
    }
}

/// `Reset` places the seven tail segments, the four limbs and the head, a
/// `DegToVec(value * 360)` each: twelve draws. The constructor runs under the ID's
/// seed: the same module again, its cosmetics' sprites after the 20 of the body.
#[test]
fn reset_makes_twelve_draws_and_the_constructor_is_seeded() {
    let Some((mut s, id)) = Session::with_green_lizard(None) else {
        return;
    };
    s.lizard_ticks(20);
    let mut g = s.lizard_module(id);
    let scene = s.scene_mut().expect("game");
    let world = scene.game().sim().world();
    let view = world.lizard_view(g.creature, None).expect("view");
    let mut rng = UnityRandom::new(9);
    let mut expected = rng.clone();
    g.reset(&view, &mut rng);
    for _ in 0..12 {
        expected.value();
    }
    assert_eq!(rng, expected);
    let a = LizardGraphics::new(&view);
    let b = LizardGraphics::new(&view);
    assert_eq!(a.ivars, b.ivars);
    assert_eq!(a.cosmetics, b.cosmetics);
    let mut next = LIZARD_BASE_SPRITES;
    for c in &a.cosmetics {
        assert_eq!(c.start_sprite, next);
        next += c.number_of_sprites;
    }
    assert_eq!(a.tail.len(), 7);
    assert_eq!(a.limbs.len(), 4);
}

/// A calm lizard's `DrawSprites` (jaw shut, no flicker, no bubbles, no threat) draws
/// once, for the every-second-draw toggle, and lists the body, tail, four legs with
/// their colour layer and five head sprites in container order.
#[test]
fn a_calm_lizards_draw_makes_one_draw() {
    let Some((mut s, id)) = Session::with_green_lizard(None) else {
        return;
    };
    s.lizard_ticks(80);
    let mut g = s.lizard_module(id);
    let scene = s.scene_mut().expect("game");
    let world = scene.game().sim().world();
    let view = world.lizard_view(g.creature, None).expect("view");
    assert!(!g.culled && g.white_flicker == 0 && g.flicker == 0);
    let mut rng = UnityRandom::new(4);
    let mut expected = rng.clone();
    let toggles = expected.value() > 0.025;
    let before = g.every_second_draw;
    let list = g.draw(&view, Color::BLACK, 0.5, &mut rng);
    assert_eq!(rng, expected);
    assert_eq!(g.every_second_draw, before != toggles);
    let order: Vec<usize> = list
        .items
        .iter()
        .map(rw_graphics::DrawItem::index)
        .collect();
    assert_eq!(
        order,
        [
            7, 16, 9, 18, 5, 0, 1, 2, 3, 6, 8, 17, 10, 19, 11, 12, 13, 14, 15
        ]
    );
    assert!(
        list.sprite(11)
            .expect("jaw")
            .element
            .starts_with("LizardJaw")
    );
    assert!(
        list.sprite(15)
            .expect("eyes")
            .element
            .starts_with("LizardEyes")
    );
}

/// `HeadRotation(t)`: the aim from `drawPositions[0]` to the head, both lerped by `t`,
/// plus `jawOpenAngle * (1 - jawOpenLowerJawFac)` times the lerped jaw (clamped) and
/// head depth rotation.
#[test]
fn head_rotation_lerps_the_head_and_jaw() {
    let Some((mut s, id)) = Session::with_green_lizard(None) else {
        return;
    };
    s.lizard_ticks(20);
    let mut g = s.lizard_module(id);
    g.draw_positions[0] = [Vec2::ZERO, Vec2::ZERO];
    g.head.last_pos = Vec2::new(0.0, 10.0);
    g.head.pos = Vec2::new(20.0, 10.0);
    g.last_head_depth_rotation = 0.0;
    g.head_depth_rotation = 1.0;
    let lizard = LizardReading {
        excitement: 0.5,
        last_jaw_open: 0.0,
        jaw_open: 2.0,
        jaw_turn: 40.0,
    };
    // Aim from (0, 0) to (10, 10): 45; jaw Clamp(Lerp(0, 2, 0.5)) = 1, depth 0.5.
    assert!((g.head_rotation(&lizard, 0.5) - 65.0).abs() < 1e-4);
    // At 1: aim from (0, 0) to (20, 10), jaw 1, depth 1.
    let aim = 20f32.atan2(10.0).to_degrees();
    assert!((g.head_rotation(&lizard, 1.0) - (aim + 40.0)).abs() < 1e-4);
    let b = g.for_cosmetics(&lizard, Color::BLACK, 0.25);
    assert_eq!(b.head, Vec2::new(5.0, 10.0));
}

/// A bubble keeps the module that made it: once the module is dropped its bubbles
/// still read its head colour, and it is forgotten once no bubble refers to it.
#[test]
fn a_dropped_module_still_colours_its_bubbles() {
    let Some((mut s, id)) = Session::with_green_lizard(None) else {
        return;
    };
    s.lizard_ticks(20);
    let g = s.lizard_module(id);
    assert_ne!(g.module, 0);
    let game = s.scene_mut().expect("game").game_mut();
    let view = game
        .sim()
        .world()
        .lizard_view(g.creature, None)
        .expect("view");
    let expected = g.for_cosmetics(&LizardReading::of(&view), Color::BLACK, 0.5);
    assert_eq!(
        game.lizard_module(g.creature, g.module, 0.5),
        Some(expected)
    );
    game.remove_lizard_module(g.id);
    assert!(game.lizard_graphics().iter().all(|m| m.module != g.module));
    assert_eq!(
        game.lizard_module(g.creature, g.module, 0.5),
        Some(expected)
    );
    game.tick(Input::NONE);
    let world = game.sim().world();
    let referred = world.active_rooms().into_iter().any(|r| {
        world
            .room_cosmetics(r)
            .iter()
            .any(|c| matches!(&c.kind, CosmeticKind::LizardBubble(b) if b.module == g.module))
    });
    assert_eq!(
        game.lizard_module(g.creature, g.module, 0.5).is_some(),
        referred
    );
}

/// A head-shield flash and spark keep the module they were given (`Lizard.Violence`
/// passes `graphicsModule`): once it is dropped they still read its head colour and
/// keep it alive.
#[test]
fn a_dropped_module_still_colours_its_flash_and_sparks() {
    let Some((mut s, id)) = Session::with_green_lizard(None) else {
        return;
    };
    s.lizard_ticks(20);
    let g = s.lizard_module(id);
    let game = s.scene_mut().expect("game").game_mut();
    let room = game
        .sim()
        .world()
        .creature(g.creature)
        .lizard()
        .and_then(|l| l.room)
        .expect("room");
    let pos = game
        .sim()
        .world()
        .lizard_view(g.creature, None)
        .expect("view")
        .lizard
        .body
        .chunks[0]
        .pos;
    let mut spark = Spark::new(pos, Vec2::ZERO, [1.0; 4], 10, 170, &mut UnityRandom::new(1));
    spark.lizard = Some((g.creature, g.module));
    let world = game.sim_mut().world_mut();
    world.add_cosmetic(room, CosmeticKind::Spark(spark));
    let flash = StationaryEffect::flashing_orb(pos, Some((g.creature, g.module)));
    world.add_cosmetic(room, CosmeticKind::Flash(flash));
    game.remove_lizard_module(g.id);
    game.tick(Input::NONE);
    assert!(game.lizard_graphics().iter().all(|m| m.module != g.module));
    let view = game
        .sim()
        .world()
        .lizard_view(g.creature, None)
        .expect("view");
    let expected = g.for_cosmetics(&LizardReading::of(&view), Color::BLACK, 0.5);
    assert_eq!(
        game.lizard_module(g.creature, g.module, 0.5),
        Some(expected)
    );
}
