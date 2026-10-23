//! `Water.Surface.Update`'s rain rumble: three draws per surface point a tick in a room
//! whose `DangerType` is not `None`, whatever the shake values are; the height term
//! `Lerp(-1, 1, value) * ScreenShake * RumbleIntensity` after the damping, and the
//! position term `DegToVec(value * 360) * (MicroScreenShake * 4 * RumbleIntensity)` between
//! the sine wave and the roll-back wave. Expected values from `Water.cs` (`Surface.Update`).
//!
//! Mirrors: - (port-only: tests)

use rw_assets::{DangerType, Room, RoomSettings};
use rw_random::UnityRandom;

use crate::tests::room_with_water;
use crate::{FloatExt, Sounds, Vec2, Water};

fn pool(danger: DangerType, rumble: f32) -> (Room, Water) {
    let row = ".".repeat(20);
    let room = room_with_water(&[row.as_str(); 10], 3);
    let settings = RoomSettings {
        danger_type: Some(danger),
        rumble_intensity: rumble,
        ..RoomSettings::ANCESTOR
    };
    let water = Water::new(&room, &settings).expect("water");
    (room, water)
}

/// A seed whose first tick makes no bottom bubble (two draws, no more).
fn seed_without_bubble() -> i32 {
    let (room, mut w) = pool(DangerType::None, 1.0);
    (0..)
        .find(|&s| {
            let made = w.update(
                &room,
                std::iter::empty(),
                &mut UnityRandom::new(s),
                &mut Sounds::default(),
            );
            made.is_empty()
        })
        .unwrap()
}

fn assert_close(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-4, "{a} vs {b}");
}

/// A `None` room draws the bottom bubble's two and one jitter per point; any other
/// `DangerType` adds the height term and the two position terms of each point, with
/// the shake at 0 as well.
#[test]
fn a_danger_room_draws_three_more_per_point() {
    let seed = seed_without_bubble();
    for (danger, per_point) in [
        (DangerType::None, 1),
        (DangerType::Rain, 4),
        (DangerType::Flood, 4),
        (DangerType::FloodAndRain, 4),
        (DangerType::Thunder, 4),
    ] {
        let (room, mut w) = pool(danger, 1.0);
        let mut rng = UnityRandom::new(seed);
        w.update(&room, std::iter::empty(), &mut rng, &mut Sounds::default());
        let mut mirror = UnityRandom::new(seed);
        for _ in 0..2 + per_point * w.point_count() {
            mirror.value();
        }
        assert_eq!(rng, mirror, "{danger:?}");
    }
}

/// The draw order inside a point: the jitter, then the rumble height, then the two rows'
/// position terms; the first point's effects are checked against the same step with the
/// rumble off.
#[test]
fn the_rumble_terms_use_their_own_draws() {
    let seed = seed_without_bubble();
    let (room, mut still) = pool(DangerType::None, 1.0);
    let mut plain = UnityRandom::new(seed);
    still.update(
        &room,
        std::iter::empty(),
        &mut plain,
        &mut Sounds::default(),
    );

    let (_, mut w) = pool(DangerType::Rain, 0.5);
    w.rumble.screen_shake = 2.0;
    w.rumble.micro_screen_shake = 1.5;
    let mut rng = UnityRandom::new(seed);
    w.update(&room, std::iter::empty(), &mut rng, &mut Sounds::default());

    let mut m = UnityRandom::new(seed);
    m.value();
    m.value();
    m.value();
    let height = (-1.0_f32).lerp(1.0, m.value()) * 2.0 * 0.5;
    let front = Vec2::from_degrees(m.value() * 360.0) * (1.5 * 4.0 * 0.5);
    let back = Vec2::from_degrees(m.value() * 360.0) * (1.5 * 4.0 * 0.5);
    assert_close(w.wave_height(0), still.wave_height(0) + height);
    let moved = w.points[0][0].pos - still.points[0][0].pos;
    assert!((moved - front).length() < 1e-3, "{moved:?} vs {front:?}");
    let moved = w.points[0][1].pos - still.points[0][1].pos;
    assert!((moved - back).length() < 1e-3, "{moved:?} vs {back:?}");
}

/// No rumble without rain danger: the shake values are ignored and the point is untouched.
#[test]
fn a_room_without_danger_ignores_the_shake() {
    let seed = seed_without_bubble();
    let (room, mut a) = pool(DangerType::None, 1.0);
    let (_, mut b) = pool(DangerType::None, 1.0);
    b.rumble.screen_shake = 3.0;
    b.rumble.micro_screen_shake = 3.0;
    for w in [&mut a, &mut b] {
        w.update(
            &room,
            std::iter::empty(),
            &mut UnityRandom::new(seed),
            &mut Sounds::default(),
        );
    }
    assert_eq!(a.wave_height(7), b.wave_height(7));
    assert_eq!(a.points[7][0].pos, b.points[7][0].pos);
}
