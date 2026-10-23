use rw_random::UnityRandom;
use rw_sim::creature::CreatureId;
use rw_sim::creature::lizard::{BubbleHead, LizardBubble};
use rw_sim::{StationaryEffect, Vec2};

use super::{BubbleSprite, FlashSprite};
use crate::core::draw::{Color, DrawItem};
use crate::tests::{MODULE, OneModule};

/// A bubble of module 7 stuck at `(10, 0)` in the head's frame, at the origin, its
/// last read head far off.
trait Stuck {
    fn stuck(life: f32) -> Self;
}

impl Stuck for LizardBubble {
    fn stuck(life: f32) -> Self {
        let mut b = Self::new(
            (CreatureId(0), 7),
            Vec2::ZERO,
            BubbleHead::default(),
            [1.0, 1.0, 0.0],
            &mut UnityRandom::new(1),
        );
        b.head = BubbleHead {
            pos: Vec2::new(100.0, 100.0),
            vel: Vec2::ZERO,
            rotation: 0.0,
        };
        b.pos = Vec2::ZERO;
        b.last_pos = Vec2::ZERO;
        b.origin_point = Vec2::new(10.0, 0.0);
        b.stuck_to_origin = true;
        b.free_floating = false;
        b.hollow = false;
        b.life = life;
        b.last_life = life;
        b
    }
}

/// Stuck, it stretches toward `head + RotateAroundOrigo(originPoint, rotation)`
/// of its module at `t`: (20, 10) + (0, -10) = (20, 0), 20 px off, so the stretch
/// is 1.25: `scaleY = 1.25 - 0.3125`, `anchorY = InverseLerp(1.25, 0.3125, 1.25)
/// * 0.5 = 0`, aimed at 90 degrees.
#[test]
fn a_stuck_bubble_stretches_toward_its_modules_head() {
    let b = LizardBubble::stuck(1.0);
    let DrawItem::Sprite(sp) = b.sprite(0.5, &OneModule(MODULE)) else {
        panic!("a sprite");
    };
    assert!((sp.rotation - 90.0).abs() < 1e-4);
    assert!((sp.scale.y - 0.9375).abs() < 1e-5);
    assert!(sp.anchor.y.abs() < 1e-6);
    assert_eq!(sp.color, MODULE.head_color);
}

/// Its colour runs from the module's head colour to the module's own black by
/// `1 - Clamp(life * 2)`: halfway at a quarter of its life. Without a module the
/// camera's black stands in for both.
#[test]
fn a_bubble_fades_to_its_modules_black() {
    let b = LizardBubble::stuck(0.25);
    let DrawItem::Sprite(sp) = b.sprite(0.0, &OneModule(MODULE)) else {
        panic!("a sprite");
    };
    assert_eq!(sp.color, Color::rgb(0.5, 0.0, 0.5));
    let mut orphan = LizardBubble::stuck(0.25);
    orphan.module = 8;
    let DrawItem::Sprite(sp) = orphan.sprite(0.0, &OneModule(MODULE)) else {
        panic!("a sprite");
    };
    assert_eq!(sp.color, Color::WHITE);
}

/// A head-shield flash takes `HeadColor(t)` of the module it was given (three
/// draws, no colour draw), white when this view lacks it.
#[test]
fn a_flash_shows_its_modules_head_colour() {
    let lizards = OneModule(MODULE);
    let mut rng = UnityRandom::new(5);
    let flash = StationaryEffect::flashing_orb(Vec2::ZERO, Some((CreatureId(0), 7)));
    let DrawItem::Sprite(sp) = flash.sprite(0.5, &lizards, &mut rng) else {
        panic!("a sprite");
    };
    assert_eq!(sp.color, MODULE.head_color);
    let mut expect = UnityRandom::new(5);
    for _ in 0..3 {
        expect.value();
    }
    assert_eq!(rng.state(), expect.state());
    let orphan = StationaryEffect::flashing_orb(Vec2::ZERO, Some((CreatureId(0), 8)));
    let DrawItem::Sprite(sp) = orphan.sprite(0.5, &lizards, &mut rng) else {
        panic!("a sprite");
    };
    assert_eq!(sp.color, Color::WHITE);
}
