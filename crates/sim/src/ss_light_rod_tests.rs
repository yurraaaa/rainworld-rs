//! `SSLightRod`: the light count, the constructor's draws, a vessel's turn.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::SsLightRodData;
use rw_random::UnityRandom;

use super::{ROD_COLOR, RodCamera, SsLightRod};
use crate::tests::room_with_water;
use crate::{CameraView, FloatExt, LightSource, LightTie, Vec2};

fn data(depth: f32, rotation: f32, length: f32, brightness: f32) -> SsLightRodData {
    SsLightRodData {
        panel_pos: [0.0, 0.0],
        depth,
        rotation,
        length,
        brightness,
    }
}

fn lights(ids: &[u64]) -> Vec<LightSource> {
    ids.iter()
        .map(|&id| SsLightRod::light_source(id, 0, 9, Vec2::new(50.0, 60.0)))
        .collect()
}

/// `UpdateLightAmount`: `IntClamp((int)(length / 45), 2, 30)`.
#[test]
fn light_count_is_a_light_per_45_px_between_2_and_30() {
    assert_eq!(SsLightRod::light_count(40.0), 2);
    assert_eq!(SsLightRod::light_count(80.0), 2);
    assert_eq!(SsLightRod::light_count(110.5898), 2);
    assert_eq!(SsLightRod::light_count(135.0), 3);
    assert_eq!(SsLightRod::light_count(800.0), 17);
    assert_eq!(SsLightRod::light_count(5000.0), 30);
}

/// The vessel's light is `new LightSource(placedObject.pos, false, rod.color, rod)`:
/// not environmental, `(0.4, 1, 0.8)`, tied to the rod.
#[test]
fn a_vessels_light_is_the_rods_colour_and_tied_to_it() {
    let l = SsLightRod::light_source(4, 2, 9, Vec2::new(50.0, 60.0));
    assert!(!l.environmental_light);
    assert_eq!(ROD_COLOR, [0.4, 1.0, 0.8]);
    assert_eq!((l.color, l.color_alpha), ([0.4 / 1.0, 1.0, 0.8 / 1.0], 1.0));
    assert_eq!(l.tied_to, Some(LightTie::Fixture(9)));
    assert_eq!(l.pos, Vec2::new(50.0, 60.0));
}

/// The constructor draws three a vessel: `Reset`'s speed and size, then the progression.
#[test]
fn the_constructor_draws_three_a_light() {
    let mut rng = UnityRandom::new(7);
    let mut e = UnityRandom::new(7);
    let rod = SsLightRod::new(5, Vec2::ZERO, data(0.0, 0.0, 200.0, 0.5), &[1, 2, 3], &mut rng);
    for v in &rod.lights {
        let (speed, size, progression) = (e.value(), e.value(), e.value());
        assert_eq!(v.speed, 0.5f32.lerp(2.0, speed));
        assert_eq!(v.size, size);
        assert_eq!(v.progression, progression);
        assert_eq!((v.strength, v.visible), (0.0, 0.0));
    }
    assert_eq!(rod.lights.iter().map(|v| v.light).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(rng.state(), e.state());
}

fn camera_room() -> rw_assets::Room {
    room_with_water(&["....................", "...................."], -1)
}

/// A vessel at half way, unseen by any camera (`b` 0.7): `strength` is
/// `InverseLerp(0.1, 1, sin(pi / 2)^0.5)` = 1, `visible` moves toward 0.7 by
/// `Lerp(0.2, 0.05, brightness)`, the light's alpha is `Lerp(1, 0.5, size) * strength *
/// visible * power`, its rad `Lerp(100, 400, size) * visible * Lerp(0.2, 1.5, brightness)`
/// and its place a rod length along the rotation; the progression advances by `speed /
/// length` and no draw is made.
#[test]
fn an_unseen_vessel_fades_toward_seven_tenths() {
    let room = camera_room();
    let cam = RodCamera {
        view: CameraView {
            room: 3,
            current: 0,
            next: 0,
        },
        room_index: 0,
        room: &room,
        image: None,
    };
    let pos = Vec2::new(50.0, 60.0);
    let d = data(0.0, 90.0, 100.0, 0.5);
    let mut rod = SsLightRod::new(9, pos, d, &[1, 2], &mut UnityRandom::new(1));
    let mut ls = lights(&[1, 2]);
    rod.lights[0].progression = 0.5;
    rod.lights[0].speed = 1.0;
    rod.lights[0].size = 0.25;
    let mut rng = UnityRandom::new(3);
    let before = rng.state();
    rod.update(&mut ls, 0.8, &cam, &mut rng);
    let v = &rod.lights[0];
    let visible = 0.0f32.lerp(0.7, 0.2f32.lerp(0.05, 0.5));
    assert_eq!(v.visible, visible);
    assert_eq!(v.strength, 1.0 * visible);
    let l = &ls[0];
    assert_eq!(l.set_alpha, Some(1.0f32.lerp(0.5, 0.25) * visible * 0.8));
    assert_eq!(
        l.set_rad,
        Some(100.0f32.lerp(400.0, 0.25) * visible * 0.2f32.lerp(1.5, 0.5))
    );
    // `DegToVec(90)` is (1, 0) up to the float sine.
    let at = l.set_pos.expect("set");
    assert!((at.x - 100.0).abs() < 1e-3 && (at.y - 60.0).abs() < 1e-3, "{at:?}");
    assert_eq!(v.progression, 0.5 + 1.0 / 100.0);
    assert_eq!(rng.state(), before);
}

/// A seen vessel counts the 7 samples whose depth reaches the rod's: with no picture the
/// sky (1) reaches any depth, so `visible` heads for 1 at the same rate.
#[test]
fn a_seen_vessel_counts_the_samples_at_its_depth() {
    let room = camera_room();
    let cam = RodCamera {
        view: CameraView {
            room: 0,
            current: 0,
            next: 0,
        },
        room_index: 0,
        room: &room,
        image: None,
    };
    let d = data(0.6, 0.0, 100.0, 1.0);
    let mut rod = SsLightRod::new(9, Vec2::new(50.0, 10.0), d, &[1, 2], &mut UnityRandom::new(1));
    let mut ls = lights(&[1, 2]);
    rod.lights[1].progression = 0.25;
    rod.update(&mut ls, 1.0, &cam, &mut UnityRandom::new(3));
    assert_eq!(rod.lights[1].visible, 0.0f32.lerp(1.0, 0.2f32.lerp(0.05, 1.0)));
}

/// A vessel that reaches the end resets: speed and size are two new draws and the
/// progression is 0 (a draw-free update otherwise).
#[test]
fn a_vessel_at_the_end_resets_with_two_draws() {
    let room = camera_room();
    let cam = RodCamera {
        view: CameraView {
            room: 3,
            current: 0,
            next: 0,
        },
        room_index: 0,
        room: &room,
        image: None,
    };
    let d = data(0.0, 0.0, 100.0, 0.5);
    let mut rod = SsLightRod::new(9, Vec2::ZERO, d, &[1, 2], &mut UnityRandom::new(1));
    let mut ls = lights(&[1, 2]);
    rod.lights[0].progression = 0.995;
    rod.lights[0].speed = 1.0;
    let mut rng = UnityRandom::new(11);
    let mut e = UnityRandom::new(11);
    rod.update(&mut ls, 1.0, &cam, &mut rng);
    let v = &rod.lights[0];
    assert_eq!(v.progression, 0.0);
    assert_eq!(v.speed, 0.5f32.lerp(2.0, e.value()));
    assert_eq!(v.size, e.value());
    assert_eq!(rng.state(), e.state());
}
