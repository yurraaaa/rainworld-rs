//! `Bubble` (`Bubble.cs`): expected values from its constructor and `Update`, draws
//! replayed on a copy of the seeded stream.

use rw_assets::{Room, RoomSettings};
use rw_random::UnityRandom;

use super::{Bubble, BubbleMode};
use crate::tests::room_with_water;
use crate::{FloatExt, Vec2, Water};

/// 20 x 10 tiles, water row 6 (still level 130), a solid ceiling block at tiles
/// (10, 4..=5).
fn pool() -> (Room, Water) {
    let open = ".".repeat(20);
    let block = format!("{}#{}", ".".repeat(10), ".".repeat(9));
    let mut rows = vec![open.as_str(); 10];
    rows[4] = &block;
    rows[5] = &block;
    let room = room_with_water(&rows, 6);
    let water = Water::new(&room, &RoomSettings::ANCESTOR).expect("water");
    (room, water)
}

/// `rng` after `n` more draws from `start`.
fn after(start: &UnityRandom, n: usize) -> [u32; 4] {
    let mut r = start.clone();
    for _ in 0..n {
        r.value();
    }
    r.state()
}

#[test]
fn constructor_draws_size_and_growth() {
    let mut rng = UnityRandom::new(3);
    let mut copy = rng.clone();
    let b = Bubble::new(
        Vec2::new(50.0, 40.0),
        Vec2::new(1.0, 2.0),
        false,
        false,
        &mut rng,
    );
    assert_eq!(rng.state(), after(&copy, 2));
    assert_eq!(b.full_size, 0.5.lerp(1.5, copy.value()));
    assert_eq!(b.growth_speed, 1.0 / 5.0.lerp(120.0, copy.value()));
    assert_eq!((b.mode, b.size, b.init_pop), (BubbleMode::Free, 0.0, true));
    let bottom = Bubble::new(Vec2::ZERO, Vec2::ZERO, true, false, &mut rng);
    assert_eq!((bottom.mode, bottom.init_pop), (BubbleMode::Growing, false));
}

#[test]
fn mode_spelling() {
    for m in [BubbleMode::Growing, BubbleMode::Free, BubbleMode::Trapped] {
        assert_eq!(BubbleMode::from_value(m.value()), Some(m));
    }
    assert_eq!(BubbleMode::from_value("Popped"), None);
}

/// A free bubble pops to full size, slows by 0.8, gets `DegToVec(-90 + 180 a) * b *
/// 1.2` (two draws), then moves.
#[test]
fn free_bubble_rises_with_a_random_push() {
    let (room, water) = pool();
    let mut rng = UnityRandom::new(7);
    let mut b = Bubble::new(
        Vec2::new(50.0, 40.0),
        Vec2::new(0.0, 5.0),
        false,
        false,
        &mut rng,
    );
    let mut copy = rng.clone();
    assert!(!b.update(&room, Some(&water), &mut rng));
    assert_eq!(rng.state(), after(&copy, 2));
    let push = Vec2::from_degrees(-90.0 + 180.0 * copy.value()) * (copy.value() * 1.2);
    let vel = Vec2::new(0.0, 4.0) + push;
    assert_eq!(b.vel, vel);
    assert_eq!(b.pos, Vec2::new(50.0, 40.0) + vel);
    assert_eq!(b.last_pos, Vec2::new(50.0, 40.0));
    assert_eq!((b.size, b.last_size, b.age), (1.0, 0.0, 1));
}

/// Above `FloatWaterLevel - 10` (120) the size drops to 0; the next tick it goes.
#[test]
fn bubble_at_the_surface_shrinks_then_goes() {
    let (room, water) = pool();
    let mut rng = UnityRandom::new(7);
    let mut b = Bubble::new(Vec2::new(50.0, 121.0), Vec2::ZERO, false, false, &mut rng);
    assert!(!b.update(&room, Some(&water), &mut rng));
    assert_eq!(b.size, 0.0);
    b.pos.y = 121.0;
    assert!(b.update(&room, Some(&water), &mut rng));
    // Without water the level is `float.MinValue`: always above it.
    let mut dry = Bubble::new(Vec2::new(50.0, 40.0), Vec2::ZERO, false, false, &mut rng);
    assert!(!dry.update(&room, None, &mut rng));
    assert!(dry.update(&room, None, &mut rng));
    // A fake water bubble ignores the surface.
    let mut fake = Bubble::new(Vec2::new(50.0, 121.0), Vec2::ZERO, false, true, &mut rng);
    assert!(!fake.update(&room, Some(&water), &mut rng));
    assert_eq!(fake.size, 1.0);
}

/// Growing: still, one draw `> 0.0125` to grow by `growthSpeed` and rise `fullSize *
/// growthSpeed * 10`; once full a second chance `< 0.05` frees it.
#[test]
fn bottom_bubble_grows_in_place() {
    let (room, water) = pool();
    let mut rng = UnityRandom::new(11);
    let mut b = Bubble::new(
        Vec2::new(50.0, 10.0),
        Vec2::new(3.0, 3.0),
        true,
        false,
        &mut rng,
    );
    let mut copy = rng.clone();
    b.update(&room, Some(&water), &mut rng);
    let grew = copy.value() > 0.0125;
    if !grew {
        copy.value();
    }
    assert_eq!(rng.state(), copy.state());
    assert_eq!(b.vel, Vec2::ZERO);
    if grew {
        assert_eq!(b.size, b.growth_speed);
        assert_eq!(b.pos.y, 10.0 + b.full_size * b.growth_speed * 10.0);
    }
    // Full: one draw against 0.05.
    b.size = 1.0;
    b.mode = BubbleMode::Growing;
    let mut copy = rng.clone();
    b.update(&room, Some(&water), &mut rng);
    assert_eq!(rng.state(), after(&copy, 1));
    let freed = copy.value() < 0.05;
    assert_eq!(b.mode == BubbleMode::Free, freed);
}

/// Rising from tile (10, 3) into the solid tile (10, 4): trapped 11 px under the middle
/// of the solid tile; a trapped bubble sits 9 px over its tile's middle and shrinks by
/// `growthSpeed` a tick (two draws).
#[test]
fn bubble_rising_into_a_ceiling_is_trapped() {
    let (room, water) = pool();
    let mut rng = UnityRandom::new(5);
    let mut b = Bubble::new(Vec2::new(210.0, 79.0), Vec2::ZERO, false, false, &mut rng);
    b.update(&room, Some(&water), &mut rng);
    // Placed inside the tile: lastPos below it.
    b.pos = Vec2::new(210.0, 81.0);
    b.last_pos = Vec2::new(210.0, 79.0);
    b.vel = Vec2::ZERO;
    b.age = 1;
    b.do_not_slow = true;
    b.update(&room, Some(&water), &mut rng);
    assert_eq!(b.mode, BubbleMode::Trapped);
    // pos.y = middle (90) - 11, then moved by vel.y 0.
    assert_eq!(b.pos.y, 79.0);
    let copy = rng.clone();
    let size = b.size;
    b.update(&room, Some(&water), &mut rng);
    assert_eq!(rng.state(), after(&copy, 2));
    assert_eq!(b.size, size - b.growth_speed);
    // Tile (10, 3)'s middle is 70: held at 79 while the tile above is solid.
    assert_eq!(b.mode, BubbleMode::Trapped);
}

/// A free bubble goes on its 601st update (`age > 600`).
#[test]
fn free_bubble_goes_after_600_ticks() {
    let (room, water) = pool();
    let mut rng = UnityRandom::new(5);
    let mut b = Bubble::new(Vec2::new(50.0, 40.0), Vec2::ZERO, false, false, &mut rng);
    b.age = 599;
    assert!(!b.update(&room, Some(&water), &mut rng));
    b.pos = Vec2::new(50.0, 40.0);
    assert!(b.update(&room, Some(&water), &mut rng));
    assert_eq!(b.age, 601);
}
