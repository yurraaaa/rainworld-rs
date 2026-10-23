use rw_sim::Vec2;

use super::SplashSprites;
use crate::core::draw::{DrawItem, Layer, Shader};

/// `Splash.DrawSprites` halfway between ticks: both quads at the splash, anchored
/// `(0.5, 0)`, `rad / 8` wide and `distance / 16` long, aimed at the linger point,
/// alpha `0.5 * life` (clamped at 0), colour `(0, 0, i)`, sprite 0 in `Water` and 1 in
/// `GrabShaders`, both on `WaterSplash`.
#[test]
fn splash_sprites() {
    let mut s = rw_sim::Splash::new(Vec2::ZERO, Vec2::new(0.0, 1.0), 8.0, 4.0, 0.0);
    s.last_pos = Vec2::new(10.0, 50.0);
    s.pos = Vec2::new(14.0, 54.0);
    s.last_linger_pos = Vec2::new(12.0, 20.0);
    s.linger_pos = Vec2::new(12.0, 20.0);
    s.last_rad = 4.0;
    s.rad = 6.0;
    s.last_life = 0.8;
    s.life = 0.6;
    let items = s.sprites(0.5);
    assert_eq!(items.len(), 2);
    for (i, item) in items.iter().enumerate() {
        let DrawItem::Sprite(sp) = item else {
            panic!("a sprite");
        };
        assert_eq!(sp.index, i);
        assert_eq!(sp.element, "Futile_White");
        assert_eq!(sp.shader, Shader::WaterSplash);
        assert_eq!(sp.pos, Vec2::new(12.0, 52.0));
        assert_eq!(sp.anchor, Vec2::new(0.5, 0.0));
        assert_eq!(sp.scale, Vec2::new(5.0 / 8.0, 32.0 / 16.0));
        assert_eq!(sp.rotation, 180.0);
        assert!((sp.color.a - 0.35).abs() < 1e-6);
        assert_eq!((sp.color.r, sp.color.g, sp.color.b), (0.0, 0.0, i as f32));
    }
    assert_eq!(items[0].layer(), Layer::Water);
    assert_eq!(items[1].layer(), Layer::GrabShaders);
    s.last_life = -0.1;
    s.life = -0.2;
    let DrawItem::Sprite(sp) = &s.sprites(0.5)[0] else {
        panic!("a sprite");
    };
    assert_eq!(sp.color.a, 0.0);
}
