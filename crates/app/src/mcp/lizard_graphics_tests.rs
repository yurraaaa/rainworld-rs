//! A green lizard's graphics module on SU_A07 of the installed game (skipped without
//! it): built in the viewed room, its legs feed `LegsGripping`; none in a room nobody
//! views.

use rw_assets::{Room, Terrain};

use super::Session;
use super::params::{InputParam, SpawnCreatureParams, StepParams};
use super::reply::{LizardInfo, State};
use rw_graphics::{Color, LIZARD_BASE_SPRITES, LizardGraphics};
use rw_random::UnityRandom;

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
