use rw_random::UnityRandom;

use super::{BubbleHead, LizardBubble};
use crate::Vec2;
use crate::creature::CreatureId;

/// `LizardBubble`'s constructor: the lerp, angle and offset values, the four nested
/// ranges (each drawing unless its bound is 0), then the angle, two speed factors and
/// the hollowness.
#[test]
fn the_constructor_draws_in_the_games_order() {
    for seed in [1, 7, 42] {
        let mut rng = UnityRandom::new(seed);
        let mut expected = rng.clone();
        let head = BubbleHead {
            pos: Vec2::new(10.0, 0.0),
            vel: Vec2::new(1.0, 0.0),
            rotation: 90.0,
        };
        let b = LizardBubble::new(
            (CreatureId(0), 1),
            Vec2::ZERO,
            head,
            [1.0, 0.5, 2.0],
            &mut rng,
        );
        for _ in 0..3 {
            expected.value();
        }
        let mut r = expected.range_i32(0, 200);
        r = expected.range_i32(0, r);
        r = expected.range_i32(0, r);
        let life = 10 + expected.range_i32(0, r);
        for _ in 0..4 {
            expected.value();
        }
        assert_eq!(rng, expected);
        assert_eq!(b.life_time, life);
        assert!(b.stuck_to_origin && !b.free_floating);
    }
}

/// No stickiness and a slow head: it floats free from the start.
#[test]
fn a_bubble_without_stickiness_floats_free() {
    let mut rng = UnityRandom::new(3);
    let b = LizardBubble::new(
        (CreatureId(0), 1),
        Vec2::ZERO,
        BubbleHead::default(),
        [1.0, 0.0, 0.0],
        &mut rng,
    );
    assert!(b.free_floating && !b.stuck_to_origin);
}
