use rw_random::UnityRandom;
use rw_sim::Vec2;
use rw_sim::creature::CreatureId;
use rw_sim::creature::overseer::HologramLight;

use super::HologramLightSprites;
use crate::core::draw::{DrawItem, Shader};

/// Unpowered: the grid sprite only, after the flicker exponent's one draw. Fully
/// powered next to the eye: the grid at 20 x 20 (`Lerp(15, 20, 1) + sin(pi) * 30`),
/// coloured (eye direction, 1) at alpha 1, and lines.
#[test]
fn draws_the_grid_and_lines_when_powered() {
    let room = crate::tests::room(&["..........", ".........."]);
    let mut light = HologramLight::new(1, 0, CreatureId(1));
    light.pos = Vec2::new(100.0, 20.0);
    light.last_pos = light.pos;
    let mut rng = UnityRandom::new(4);
    let mut expected = rng.clone();
    expected.value();
    let (list, set) = light.draw(&room, Some(Vec2::new(100.0, 30.0)), 1.0, &mut rng);
    assert_eq!(rng, expected);
    assert_eq!(list.items.len(), 1);
    // Unpowered: the light source at the eye, rad `2 * 8 * Lerp(6, 16, 0)`, dark.
    assert_eq!(set.pos, Vec2::new(100.0, 30.0));
    assert_eq!((set.rad, set.alpha), (96.0, 0.0));

    light.power = 1.0;
    light.last_power = 1.0;
    let (list, set) = light.draw(&room, Some(Vec2::new(100.0, 30.0)), 1.0, &mut rng);
    // Fully powered: rad `2 * 8 * 16`, alpha `InverseLerp(0.75, 1, 1) * 0.1`.
    assert_eq!(set.pos, Vec2::new(100.0, 20.0));
    assert_eq!((set.rad, set.alpha), (256.0, 0.1));
    let DrawItem::Sprite(grid) = &list.items[0] else {
        panic!("a sprite");
    };
    assert_eq!(grid.shader, Shader::HoloGrid);
    assert!((grid.scale.x - 20.0).abs() < 1e-4 && grid.scale.y == 20.0);
    assert_eq!(grid.pos, Vec2::new(100.0, 20.0));
    assert_eq!((grid.color.r, grid.color.b, grid.color.a), (0.5, 1.0, 1.0));
    assert!(list.items.len() > 1);
}
