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
    let base: Vec<usize> = order.iter().copied().filter(|&i| i < 20).collect();
    assert_eq!(
        base,
        [
            7, 16, 9, 18, 5, 0, 1, 2, 3, 6, 8, 17, 10, 19, 11, 12, 13, 14, 15
        ]
    );
    // `containers[0]` (what the jaws hold) sits after the lower jaw and teeth (11, 12),
    // before the upper teeth, head and eyes.
    let after_lower = order.iter().position(|&o| o == 12).expect("teeth") + 1;
    assert_eq!(
        g.internal_container,
        Some(rw_graphics::InternalContainer {
            at: after_lower,
            layer: rw_graphics::Layer::Midground,
        })
    );
    // `AddToContainer`: the cosmetics behind the head go between the near legs and the
    // head, those in front after the head; each in sprite order.
    let place: Vec<usize> = (0..order.len().max(20) + 1)
        .map(|i| order.iter().position(|&o| o == i).unwrap_or(usize::MAX))
        .collect();
    for c in &g.cosmetics {
        let range = c.start_sprite..c.start_sprite + c.number_of_sprites;
        let drawn: Vec<usize> = order
            .iter()
            .copied()
            .filter(|i| range.contains(i))
            .collect();
        assert_eq!(
            drawn,
            range.clone().collect::<Vec<_>>(),
            "{}",
            c.shape.name()
        );
        for i in range {
            match c.sprites_overlap {
                rw_graphics::SpritesOverlap::BehindHead => {
                    assert!(place[i] > place[19] && place[i] < place[11]);
                }
                rw_graphics::SpritesOverlap::InFront => assert!(place[i] > place[15]),
                o => panic!("a green's cosmetic is never {o:?}"),
            }
        }
    }
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

/// `SpineSpikes.DrawSprites` on a straight lizard facing right, unturned: chunks 20 px
/// apart, the head 10 px ahead, the tail's seven segments 10 px apart behind. Then
/// `BodyPosition` is 10 px steps from the head back (0 to 3), so `SpinePosition(s)` is
/// `-40 s / split` along the body and `-40 - 70 (s - split) / (1 - split)` along the tail
/// (`split` the body rods over the rods and tail). With `spineLength` half of
/// `BodyAndTailLength` the three spikes sit at 0.05, 0.275 and 0.5, sized by
/// `Sin(Pow(k / 2, 1) * PI)` between min and max, flattened to a fifth (`depthRotation`
/// 0), turned -180 degrees (see below); the coloured layer follows at
/// `startSprite + bumps`.
#[test]
fn spine_spikes_follow_a_straight_spine() {
    let Some((mut s, id)) = Session::with_green_lizard(None) else {
        return;
    };
    s.lizard_ticks(10);
    let mut g = s.lizard_module(id);
    let scene = s.scene_mut().expect("game");
    let world = scene.game().sim().world();
    let view = world.lizard_view(g.creature, None).expect("view");
    let o = Vec2::new(500.0, 300.0);
    for i in 0..3 {
        let p = o + Vec2::new(-20.0 * i as f32, 0.0);
        g.draw_positions[i] = [p, p];
    }
    g.head.pos = o + Vec2::new(10.0, 0.0);
    g.head.last_pos = g.head.pos;
    for (j, t) in g.tail.iter_mut().enumerate() {
        t.part.pos = o + Vec2::new(-40.0 - 10.0 * (j + 1) as f32, 0.0);
        t.part.last_pos = t.part.pos;
    }
    g.depth_rotation = 0.0;
    g.last_depth_rotation = 0.0;
    let c = &view.lizard.body.connections;
    let rods = c[0].distance + c[1].distance;
    let split = rods / (rods + g.tail_length);
    let body_and_tail = g.body_length + g.tail_length;
    let spikes = rw_graphics::LizardCosmetic {
        start_sprite: 20,
        number_of_sprites: 6,
        sprites_overlap: rw_graphics::SpritesOverlap::BehindHead,
        shape: rw_graphics::CosmeticShape::SpineSpikes(rw_graphics::SpineBumps {
            bumps: 3,
            spine_length: 0.5 * body_and_tail,
            size_skew_exponent: 1.0,
            size_range_min: 0.4,
            size_range_max: 1.0,
            graphic: 3,
            scale_x: -1.0,
            colored: 1,
        }),
    };
    let cx = rw_graphics::CosmeticDraw {
        g: &g,
        view: &view,
        black: Color::BLACK,
        t: 1.0,
    };
    let items = spikes.draw(&cx);
    assert_eq!(items.len(), 6);
    for (k, s) in [0.05f32, 0.275, 0.5].into_iter().enumerate() {
        let x = if s < split {
            -40.0 * s / split
        } else {
            -40.0 - 70.0 * (s - split) / (1.0 - split)
        };
        let size = 0.4 + 0.6 * (k as f32 / 2.0 * std::f32::consts::PI).sin();
        for (index, element) in [(20 + k, "LizardScaleA3"), (23 + k, "LizardScaleB3")] {
            let rw_graphics::DrawItem::Sprite(sp) = &items[index - 20] else {
                panic!("a sprite");
            };
            assert_eq!(sp.index, index);
            assert_eq!(sp.element, element);
            assert!(
                (sp.pos - (o + Vec2::new(x, 0.0))).length() < 1e-3,
                "spike {k}"
            );
            // Unturned, the aim is between two zero vectors: `-perp * 0` is (0, 0) and
            // `perp * 0` (-0, -0), whose difference's `Atan2(-0, -0)` is -180 degrees.
            assert_eq!(sp.rotation, -180.0);
            assert!((sp.scale.x + size).abs() < 1e-5 && (sp.scale.y - 0.2 * size).abs() < 1e-5);
            assert_eq!(sp.anchor, Vec2::new(0.5, 0.15));
        }
    }
}
