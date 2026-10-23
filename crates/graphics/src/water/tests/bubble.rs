use rw_random::UnityRandom;
use rw_sim::Vec2;

use super::AirBubbleSprite;
use crate::core::draw::{Color, DrawItem, Layer};

/// `Bubble.DrawSprites` halfway between ticks: size 0 to 1 at full size `f`, scale
/// `0.5 * f * 0.45`, in `(0, 1/255, 0)` in `Midground`.
#[test]
fn air_bubble_sprite() {
    let mut b = rw_sim::Bubble::new(
        Vec2::ZERO,
        Vec2::ZERO,
        false,
        false,
        &mut UnityRandom::new(1),
    );
    b.last_pos = Vec2::new(10.0, 20.0);
    b.pos = Vec2::new(14.0, 30.0);
    b.size = 1.0;
    let DrawItem::Sprite(s) = b.sprite(0.5) else {
        panic!("a sprite");
    };
    assert_eq!(s.element, "LizardBubble5");
    assert_eq!(s.pos, Vec2::new(12.0, 25.0));
    let scale = 0.5 * b.full_size * 0.45;
    assert_eq!(s.scale, Vec2::new(scale, scale));
    assert_eq!(s.color, Color::rgb(0.0, 1.0 / 255.0, 0.0));
    assert_eq!(s.layer, Layer::Midground);
}
