use rw_random::UnityRandom;
use rw_sim::{GreenSpark, Vec2};

use super::GreenSparkSprites;
use crate::core::draw::{Color, Layer, Shader};

/// A play-layer spark is a 1.5 px wide `pixel` in `Items`, `2 + 1.1 *` its last
/// step long, anchored at its start and aimed along the step; one on a depth is in
/// `Foreground` under `CustomDepth` with alpha 0, narrower, tinted toward the sky.
#[test]
fn spark_sprite_stretches_along_its_step() {
    let mut s = GreenSpark::new(Vec2::new(100.0, 100.0), &mut UnityRandom::new(1));
    s.depth = 0.0;
    s.last_last_pos = Vec2::new(100.0, 100.0);
    s.last_pos = Vec2::new(100.0, 100.0);
    s.pos = Vec2::new(100.0, 110.0);
    let sky = Color::rgb(0.5, 0.5, 1.0);
    let list = s.draw(1.0, sky);
    let sp = list.sprite(0).expect("sprite");
    assert_eq!((sp.layer, sp.shader), (Layer::Items, Shader::Basic));
    assert_eq!(sp.color, Color::rgb(0.0, 1.0, 1.0 / 85.0));
    assert_eq!(sp.scale, Vec2::new(1.5, 2.0 + 1.1 * 10.0));
    s.last_last_pos = Vec2::new(100.0, 100.0);
    s.last_pos = Vec2::new(110.0, 100.0);
    s.pos = Vec2::new(110.0, 100.0);
    let sp = *s.draw(0.0, sky).sprite(0).expect("sprite");
    assert_eq!(sp.scale.y, 2.0 + 1.1 * 10.0);
    assert!((sp.rotation - 90.0).abs() < 1e-4);

    s.depth = 5.0;
    let sp = *s.draw(1.0, sky).sprite(0).expect("sprite");
    assert_eq!(
        (sp.layer, sp.shader),
        (Layer::Foreground, Shader::CustomDepth)
    );
    assert!((sp.scale.x - 0.1).abs() < 1e-6);
    assert_eq!(sp.color.a, 0.0);
    assert_eq!((sp.color.r, sp.color.g), (0.0, 1.0));
}
