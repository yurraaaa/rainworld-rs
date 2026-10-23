//! `ProjectedCircle` against `ProjectedCircle.cs`: the constructor's draws in order
//! (connections to the circles already in the update list, off-screen lines, the turn),
//! `Update`'s radius, blink, connection blinks and turn with their draws, `UpdateDepth`
//! and `Rad`.

use rw_random::UnityRandom;

use super::{CircleOwner, ProjectedCircle};
use crate::tests::room;
use crate::{FloatExt, Vec2};

const OWNER: CircleOwner = CircleOwner::Neuron { order: 1 };

/// A 40 x 20 tile room: 800 x 400 px.
fn wide_room() -> rw_assets::Room {
    let rows: Vec<String> = (0..20).map(|_| ".".repeat(40)).collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

/// A circle made from a stream seeded with `seed`, with `existing` circles before it.
fn made(seed: i32, existing: &[u64]) -> (ProjectedCircle, UnityRandom) {
    let mut rng = UnityRandom::new(seed);
    let circle = ProjectedCircle::new(9, OWNER, 1, 0.5, &wide_room(), existing, &mut rng);
    (circle, rng)
}

/// The constructor, replayed: `baseRad = Lerp(60, 180, size)`, `spokes =
/// (int)(2 pi baseRad / 50)`; a `value < 0.5` for each circle already there, until five are
/// connected, making a connection; a `value < 0.5` for none off screen, else `Range(1, 4)`
/// lines each with `value < H / (H + W)`, `value < 0.5` and the position's `value`; then
/// the rotation speed `Lerp(0.2, 1, value)` signed by `value < 0.5`.
#[test]
fn the_constructor_draws_in_order() {
    let many: Vec<u64> = (3..12).collect();
    for existing in [&many[..3], &many[..]] {
        for seed in 1..40 {
            let (c, rng) = made(seed, existing);
            let mut r = UnityRandom::new(seed);
            let mut connected = Vec::new();
            for &other in existing {
                if connected.len() >= 5 {
                    break;
                }
                if r.value() < 0.5 {
                    connected.push(other);
                }
            }
            assert_eq!(c.connected_circles, connected, "seed {seed}");
            let mut lines = Vec::new();
            if r.value() >= 0.5 {
                for _ in 0..r.range_i32(1, 4) {
                    let tall = r.value() < 400.0 / 1200.0;
                    let low = r.value() < 0.5;
                    lines.push(if tall {
                        Vec2::new(if low { -10.0 } else { 810.0 }, 400.0 * r.value())
                    } else {
                        Vec2::new(800.0 * r.value(), if low { -10.0 } else { 410.0 })
                    });
                }
            }
            assert_eq!(c.off_screen_connections, lines, "seed {seed}");
            let speed = 0.2f32.lerp(1.0, r.value());
            let signed = speed * if r.value() < 0.5 { -1.0 } else { 1.0 };
            assert_eq!(c.rotation_speed, signed);
            assert_eq!(rng.state(), r.state());
            assert_eq!(c.base_rad, 120.0);
            assert_eq!(c.spokes, 15);
            assert_eq!(c.connections_blink.len(), connected.len() + lines.len());
        }
    }
}

/// A seed whose first `n` values are all at least `floor`.
fn quiet_seed(n: usize, floor: f32) -> i32 {
    (1..)
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            (0..n).all(|_| r.value() >= floor)
        })
        .expect("a seed")
}

/// `Update` with nothing triggering: the radius moves 0.8 toward `getToRad` then
/// `Lerp(rad, getToRad, 0.02)`, `lastRad` keeps the old one, the turn adds `speed * 140 *
/// speed / rad`, `updateDepth` is set; the draws are one for `getToRad`, one for the
/// blink and one for each connection that is not blinking.
#[test]
fn an_update_moves_and_draws() {
    let seed = quiet_seed(4, 0.01);
    let (mut c, _) = made(1, &[]);
    c.connections_blink = vec![0, 0];
    c.rad = 100.0;
    c.get_to_rad = 120.0;
    c.rotation_speed = 0.5;
    c.depth_offset = 0.25;
    let mut rng = UnityRandom::new(seed);
    c.update(true, &mut rng);
    assert_eq!(c.last_rad, 100.0);
    assert_eq!(c.rad, 100.8f32.lerp(120.0, 0.02));
    assert_eq!(c.rotation, 0.5 * 140.0 * 0.5 / c.rad);
    assert_eq!(c.last_rotation, 0.0);
    assert!(c.update_depth && !c.slated);
    assert_eq!(c.last_depth_offset, 0.25);
    let mut expected = UnityRandom::new(seed);
    for _ in 0..4 {
        expected.value();
    }
    assert_eq!(rng.state(), expected.state());
}

/// A running blink counts down; the blink value rises by 1/30 toward 1 then eases
/// `Lerp(blink, getTo, 0.02)`; with the counter at 0 the target returns to 0.
#[test]
fn the_blink_rises_and_falls() {
    let seed = quiet_seed(2, 0.01);
    let (mut c, _) = made(1, &[]);
    c.connections_blink.clear();
    c.blink_counter = 3;
    c.blink_get_to = 1.0;
    c.update(true, &mut UnityRandom::new(seed));
    assert_eq!(c.blink_counter, 2);
    assert_eq!(c.last_blink, 0.0);
    assert_eq!(c.blink, (1.0f32 / 30.0).lerp(1.0, 0.02));
    // Counter at 0: the target drops to 0 and the blink falls by 1/30.
    c.blink_counter = 0;
    let before = c.blink;
    c.update(true, &mut UnityRandom::new(seed));
    assert_eq!(c.blink_get_to, 0.0);
    assert_eq!(c.last_blink, before);
    assert_eq!(c.blink, (before - 1.0 / 30.0).max(0.0).lerp(0.0, 0.02));
}

/// A connection counts its blink down; at 0 a `value < 1/300` draw may start one of
/// `Range(10, 50)` (2 draws).
#[test]
fn connection_blinks_count_down() {
    let seed = quiet_seed(3, 0.01);
    let (mut c, _) = made(1, &[]);
    c.connections_blink = vec![5, 0];
    let mut rng = UnityRandom::new(seed);
    c.update(true, &mut rng);
    assert_eq!(c.connections_blink, [4, 0]);
    // Draws: getToRad, blink, and the one connection at 0.
    let mut expected = UnityRandom::new(seed);
    for _ in 0..3 {
        expected.value();
    }
    assert_eq!(rng.state(), expected.state());
}

/// An owner that cannot host the circle slates it.
#[test]
fn an_unhosted_circle_is_slated() {
    let (mut c, _) = made(1, &[]);
    c.update(false, &mut UnityRandom::new(quiet_seed(4, 0.01)));
    assert!(c.slated);
}

/// `UpdateDepth`: `depthOffset = Lerp(depthOffset, depth, 0.5)`, run once per `Update`.
/// `Rad(timeStacker)` is the eased radius times `Lerp(1, 0.6, blink)`.
#[test]
fn depth_and_radius_at_a_time() {
    let (mut c, _) = made(1, &[]);
    c.update_depth = true;
    c.apply_depth(1.0);
    assert_eq!(c.depth_offset, 0.5);
    assert!(!c.update_depth);
    c.last_rad = 100.0;
    c.rad = 110.0;
    c.last_blink = 0.0;
    c.blink = 1.0;
    assert_eq!(c.rad_at(0.0), 100.0);
    assert_eq!(c.rad_at(1.0), 110.0 * 0.6);
    assert_eq!(c.rad_at(0.5), 105.0 * 1.0f32.lerp(0.6, 0.5));
}
