//! `BorderPushBack`. Expected values from `RoomBorderPushBack.cs` (constructor, `Update`)
//! with `options.ScreenSize.x` 1366: margin `(1366 - 1400) / 2 - 100 = -117`.

use super::BorderPushBack;

const CAMERAS: [[f32; 2]; 3] = [[1400.0, 0.0], [0.0, 0.0], [2800.0, 0.0]];

#[test]
fn the_constructor_reads_the_camera_span_and_margin() {
    let b = BorderPushBack::new(4, &CAMERAS, 1, 1366.0);
    assert_eq!(b.margin, -117.0);
    assert_eq!(
        (b.leftmost_camera_pos, b.rightmost_camera_pos),
        (0.0, 2800.0)
    );
    assert_eq!(b.push_power, vec![0.0]);
    // The left test is `x < 0 + 117`, the right `x > 2800 + 1400 - 117`.
    assert_eq!(b.bounds().to_array(), [117.0, 4083.0]);
}

/// Inside the bounds nothing pushes and `pushPower` stays at 0; outside it ramps
/// `LerpAndTick(p, 1, 0.01, 1/600)` and the push is `3 * InverseLerp(0.3, 1, p)`,
/// toward the room on either side.
#[test]
fn the_push_ramps_up_outside_and_down_inside() {
    let mut b = BorderPushBack::new(4, &CAMERAS, 1, 1366.0);
    assert_eq!(b.update_player(0, Some(500.0)), None);
    assert_eq!(b.push_power[0], 0.0);
    // The first tick out pushes by the old power, 0.
    assert_eq!(b.update_player(0, Some(100.0)), Some(0.0));
    let p1 = 0.0f32 + (1.0 - 0.0) * 0.01 + 1.0 / 600.0;
    assert!((b.push_power[0] - p1).abs() < 1e-7);
    let mut first_push = None;
    for tick in 2..2000 {
        let before = b.push_power[0];
        let push = b.update_player(0, Some(100.0)).expect("outside");
        let want = 3.0 * ((before - 0.3) / 0.7).clamp(0.0, 1.0);
        assert!((push - want).abs() < 1e-6, "tick {tick}: {push} {want}");
        if push > 0.0 && first_push.is_none() {
            first_push = Some(tick);
        }
        if b.push_power[0] == 1.0 {
            break;
        }
    }
    assert_eq!(b.push_power[0], 1.0);
    assert!(first_push.is_some_and(|t| t > 20), "{first_push:?}");
    // Full power on the right side pushes left by 3.
    assert_eq!(b.update_player(0, Some(4100.0)), Some(-3.0));
    // Back inside: it decays by `Lerp(p, 0, 0.01)` then 1/80 a tick.
    let before = b.push_power[0];
    assert_eq!(b.update_player(0, Some(500.0)), None);
    assert!((b.push_power[0] - (before * 0.99 - 1.0 / 80.0)).abs() < 1e-6);
    // A player not there to test (held, or elsewhere) decays too.
    let before = b.push_power[0];
    assert_eq!(b.update_player(0, None), None);
    assert!(b.push_power[0] < before);
}

#[test]
fn power_grows_to_the_player_count() {
    let mut b = BorderPushBack::new(4, &CAMERAS, 1, 1366.0);
    b.push_power[0] = 0.5;
    b.grow_to(1);
    assert_eq!(b.push_power, vec![0.5]);
    b.grow_to(2);
    assert_eq!(b.push_power, vec![0.0, 0.0]);
}

/// `margin = (ScreenSize.x - 1400) / 2 - 100` at the five resolutions.
#[test]
fn the_margin_follows_the_chosen_width() {
    for (width, margin) in [
        (1024.0, -288.0),
        (1366.0, -117.0),
        (1360.0, -120.0),
        (1280.0, -160.0),
        (1229.0, -185.5),
    ] {
        assert_eq!(BorderPushBack::new(4, &CAMERAS, 1, width).margin, margin);
    }
}
