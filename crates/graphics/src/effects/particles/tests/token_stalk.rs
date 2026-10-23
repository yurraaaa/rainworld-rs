use rw_random::UnityRandom;
use rw_sim::{TokenStalk, Vec2};

use super::TokenStalkSprites;
use crate::core::draw::{Color, DrawItem};

/// Ten sprites plus one per rope segment, in leaser order, all black but the lamp;
/// with the lamp off (`lampPower` 0) the lamp is `Lerp(black, white, 0.15)`. One
/// cosmetic draw a frame (`Pow(Random.value, 0.5)`), drawn even with the lamp off.
#[test]
fn dead_stalk_sprites_and_lamp_draw() {
    let s = TokenStalk::new(
        0,
        Vec2::new(200.0, 250.0),
        Vec2::new(200.0, 200.0),
        &mut UnityRandom::new(1),
    );
    let black = Color::rgb(0.1, 0.05, 0.0);
    let mut rng = UnityRandom::new(4);
    let list = s.draw(black, 0.5, &mut rng);
    let mut one = UnityRandom::new(4);
    one.value();
    assert_eq!(rng.state(), one.state());
    assert_eq!(list.items.len(), 10 + s.coord.len());
    for (i, item) in list.items.iter().enumerate() {
        let DrawItem::Sprite(sprite) = item else {
            panic!("sprite {i}");
        };
        assert_eq!(sprite.index, i);
        let expected = if i == 9 {
            black.lerp(Color::WHITE, 0.15)
        } else {
            black
        };
        assert_eq!(sprite.color, expected, "sprite {i}");
    }
}
