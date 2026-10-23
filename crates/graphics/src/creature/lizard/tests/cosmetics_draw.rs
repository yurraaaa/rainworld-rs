use rw_assets::CreatureType;
use rw_random::UnityRandom;
use rw_sim::Vec2;

use crate::creature::lizard::cosmetics::{
    CosmeticCx, CosmeticShape, LizardCosmetic, LizardScale, SpritesOverlap,
};

/// Each cosmetic's sprites as its constructor counts them (`SpineSpikes`: a spike
/// per bump, doubled when coloured; `BumpHawk`: a circle per bump; the long scales:
/// one per scale, doubled when coloured; `ShortBodyScales`: one per scale), chained
/// from the body's 20 (`AddCosmetic`); all behind the head but the head scales.
#[test]
fn the_green_cosmetics_chain_their_sprites() {
    let cx = CosmeticCx {
        kind: CreatureType::GreenLizard,
        body_length: 60.0,
        body_and_tail_length: 140.0,
        head_size: 1.0,
        tail_color: 0.0,
    };
    let mut kinds = Vec::new();
    for seed in 0..300 {
        let mut rng = UnityRandom::new(seed);
        let list = LizardCosmetic::choose_green(cx, 20, &mut rng);
        let mut next = 20;
        for c in &list {
            assert_eq!(c.start_sprite, next);
            next += c.number_of_sprites;
            let (count, doubled, overlap) = match &c.shape {
                CosmeticShape::SpineSpikes(b) => {
                    (b.bumps as usize, b.colored > 0, SpritesOverlap::BehindHead)
                }
                CosmeticShape::BumpHawk(b) => (b.bumps as usize, false, SpritesOverlap::BehindHead),
                CosmeticShape::ShortBodyScales(s) => {
                    (s.positions.len(), false, SpritesOverlap::BehindHead)
                }
                CosmeticShape::LongShoulderScales(s) | CosmeticShape::TailTuft(s) => {
                    (s.positions.len(), s.colored, SpritesOverlap::BehindHead)
                }
                CosmeticShape::LongHeadScales(s) => {
                    (s.positions.len(), s.colored, SpritesOverlap::InFront)
                }
            };
            assert_eq!(c.number_of_sprites, if doubled { count * 2 } else { count });
            assert_eq!(c.sprites_overlap, overlap);
            kinds.push(c.shape.name());
        }
    }
    for k in [
        "SpineSpikes",
        "ShortBodyScales",
        "TailTuft",
        "LongHeadScales",
    ] {
        assert!(kinds.contains(&k), "never chosen: {k}");
    }
}

/// A scale's pull draws only while the lizard shows dominance (one angle).
#[test]
fn a_scale_draws_only_showing_dominance() {
    let mut rng = UnityRandom::new(3);
    let expected = rng.clone();
    let mut s = LizardScale::new(20.0, 1.0);
    s.pull_toward(Vec2::new(100.0, 0.0), 0.0, 0.5, &mut rng);
    assert_eq!(rng, expected);
    let mut e = rng.clone();
    s.pull_toward(Vec2::new(100.0, 0.0), 0.5, 0.5, &mut rng);
    e.value();
    assert_eq!(rng, e);
}

/// From the origin, a tip 100 px off is first moved to half its length (10 px) of
/// the target, then sped toward it by `ClampMagnitude(10 px, 10) / Lerp(5, 1.5, 0)`
/// and slowed by `Lerp(1, 0.8, 0)`: position (90, 0), velocity (92, 0).
#[test]
fn a_scale_is_pulled_within_half_its_length() {
    let mut rng = UnityRandom::new(3);
    let mut s = LizardScale::new(20.0, 1.0);
    s.pull_toward(Vec2::new(100.0, 0.0), 0.0, 0.0, &mut rng);
    assert!((s.part.pos - Vec2::new(90.0, 0.0)).length() < 1e-4);
    assert!((s.part.vel - Vec2::new(92.0, 0.0)).length() < 1e-4);
}
