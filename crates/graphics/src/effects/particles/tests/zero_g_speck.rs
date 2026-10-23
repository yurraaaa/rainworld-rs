use rw_random::UnityRandom;
use rw_sim::{SpeckKind, SpeckType, Vec2, ZeroGSpeck};

use super::SpeckSprites;
use crate::core::draw::Color;

/// A speck is a grey `pixel` of `Lerp(lastBlink, blink, t)`; lit by a light in a dark
/// room it blends toward that colour by the darkness; hidden while `reset`. It
/// makes no cosmetic draw.
#[test]
fn blink_speck_sprite() {
    let mut s = ZeroGSpeck::new(SpeckType::Blink, &mut UnityRandom::new(1));
    s.pos = Vec2::new(50.0, 50.0);
    s.last_pos = Vec2::new(40.0, 50.0);
    let SpeckKind::Blink(b) = &mut s.kind else {
        panic!("speck");
    };
    b.last_blink = 0.2;
    b.blink = 0.6;
    let mut rng = UnityRandom::new(2);
    assert_eq!(s.pick_pebble(&mut rng), None);
    assert_eq!(rng.state(), UnityRandom::new(2).state());
    let sp = *s.draw(0.5, None).sprite(0).expect("sprite");
    assert_eq!(sp.pos, Vec2::new(45.0, 50.0));
    assert!((sp.color.r - 0.4).abs() < 1e-6 && sp.color.r == sp.color.b);
    let SpeckKind::Blink(b) = &mut s.kind else {
        panic!("speck");
    };
    b.room_darkness = 0.5;
    b.last_col = [1.0, 0.0, 0.0, 1.0];
    b.col = [1.0, 0.0, 0.0, 1.0];
    let sp = *s.draw(0.5, None).sprite(0).expect("sprite");
    assert!((sp.color.r - 0.7).abs() < 1e-6 && (sp.color.g - 0.2).abs() < 1e-6);
    s.reset = true;
    assert!(s.draw(0.5, None).items.is_empty());
}

/// A spore draws one cosmetic value (`Range(1, 15)` for its `Pebble`), is a blue
/// disc `Lerp(0.15, 0.35, scale) * sin(yRot * pi)` wide and `Lerp(0.25, 0.55,
/// scale)` tall, rotated by `Lerp(lastRot, rot, t)`.
#[test]
fn spore_sprite() {
    let s = ZeroGSpeck::new(SpeckType::CorruptionSpore, &mut UnityRandom::new(1));
    let mut rng = UnityRandom::new(3);
    let pebble = s.pick_pebble(&mut rng).expect("a pebble");
    let mut e = UnityRandom::new(3);
    assert_eq!(pebble as i32 + 1, e.range_i32(1, 15));
    assert_eq!(rng.state(), e.state());
    let sp = *s.draw(0.0, Some(pebble)).sprite(0).expect("sprite");
    let SpeckKind::CorruptionSpore(st) = s.kind else {
        panic!("spore");
    };
    assert_eq!(sp.element, format!("Pebble{}", pebble + 1));
    assert_eq!(sp.color, Color::rgb(0.0, 0.0, st.col));
    assert_eq!(sp.rotation, st.last_rot);
    assert_eq!(sp.scale.y, 0.25 + 0.3 * st.scale);
}
