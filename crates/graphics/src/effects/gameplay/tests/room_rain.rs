//! A `RoomRain`'s sprites. Expected values from `RoomRain.InitiateSprites`, `DrawSprites`,
//! `AddToContainer` and `SplashSize`.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::DangerType;

use super::*;
use crate::tests::{flat_room, sprites};

/// 30 columns of ground (splash tiles (x, 1), `splashes` = 60), one camera.
fn rain(danger: DangerType, intensity: f32) -> RoomRain {
    let mut r = RoomRain::new(1, &flat_room(), danger, false);
    r.intensity = intensity;
    r
}

fn view(camera_position: Vec2) -> RainDrawView<'static> {
    RainDrawView {
        cam_pos: Vec2::new(10.0, 20.0),
        camera_position,
        screen_width: 1366.0,
        tiles: (30, 20),
        water: None,
        fog_color: Color::rgb(0.2, 0.4, 0.6),
        arena: false,
        direction: (-0.5, 0.5),
        time_stacker: 0.25,
    }
}

/// The 59 splashes `AddToContainer` adds are drawn; sprite 60 (the 61st) is created but
/// never added or updated (the user's decision, R3).
#[test]
fn the_last_splash_sprite_is_never_drawn() {
    let r = rain(DangerType::Rain, 1.0);
    assert_eq!((r.splashes, r.sprite_count()), (60, 61));
    let (_, list) = r.draw(&view(Vec2::new(-188.0, 0.0)), &mut UnityRandom::new(5));
    let drawn = sprites(&list.items);
    assert_eq!(drawn.len(), 60, "the rain quad and sprites 1 to 59");
    let indices: Vec<usize> = drawn.iter().map(|s| s.index).collect();
    assert_eq!(indices, (0..60).collect::<Vec<_>>());
}

/// Sprite 0: `Futile_White` scaled to the screen width by 768, anchored at the stage
/// origin (the camera position), on the `DeathRain` shader in `GrabShaders`. Splashes
/// below `splashes / 2` are `90DegreeSplash` (anchor y 0.2), the rest `TallSplash` (0.1),
/// in `Items`.
#[test]
fn sprites_and_containers() {
    let r = rain(DangerType::Rain, 1.0);
    let (_, list) = r.draw(&view(Vec2::new(-188.0, 0.0)), &mut UnityRandom::new(5));
    let drawn = sprites(&list.items);
    let quad = drawn[0];
    assert_eq!(quad.pos, Vec2::new(10.0, 20.0));
    assert_eq!(quad.scale, Vec2::new(1366.0 / 16.0, 48.0));
    assert_eq!(quad.anchor, Vec2::new(0.0, 0.0));
    assert_eq!(
        (quad.shader, quad.layer),
        (Shader::DeathRain, Layer::GrabShaders)
    );
    for s in &drawn[1..] {
        assert_eq!((s.shader, s.layer), (Shader::Basic, Layer::Items));
        let near = s.index < 30;
        assert_eq!(
            s.element,
            if near { "90DegreeSplash" } else { "TallSplash" }
        );
        assert_eq!(s.anchor, Vec2::new(0.5, if near { 0.2 } else { 0.1 }));
        assert_eq!(s.scale.y, 1.0);
        assert!(s.pos.y == 40.0, "tile (x, 1) middle 30 + 10: {:?}", s.pos);
    }
}

/// Per splash with a visible first pick: one `Range(0, count)`, one `Random.value` for
/// the x jitter, the rotation, the x scale and the colour: 5 draws.
#[test]
fn a_visible_splash_makes_five_draws() {
    let r = rain(DangerType::Rain, 0.25);
    let mut rng = UnityRandom::new(9);
    r.draw(&view(Vec2::new(-188.0, 0.0)), &mut rng);
    let mut expect = UnityRandom::new(9);
    for _ in 1..60 {
        let _ = expect.range_i32(0, 30);
        for _ in 0..4 {
            let _ = expect.value();
        }
    }
    assert_eq!(rng, expect);
}

/// With no tile visible each splash draws 5 picks, then rotation, x scale and colour: 8.
#[test]
fn an_unseen_splash_makes_eight_draws_and_no_sprite() {
    let r = rain(DangerType::Rain, 0.25);
    let mut rng = UnityRandom::new(9);
    let (_, list) = r.draw(&view(Vec2::new(5000.0, 0.0)), &mut rng);
    assert_eq!(list.items.len(), 1, "only the rain quad");
    let mut expect = UnityRandom::new(9);
    for _ in 1..60 {
        for _ in 0..5 {
            let _ = expect.range_i32(0, 30);
        }
        for _ in 0..3 {
            let _ = expect.value();
        }
    }
    assert_eq!(rng, expect);
}

/// At intensity 0 the sprites are hidden and nothing is drawn (no random draws); the
/// globals are set all the same.
#[test]
fn no_intensity_draws_nothing_but_sets_the_globals() {
    let r = rain(DangerType::Rain, 0.0);
    let mut rng = UnityRandom::new(9);
    let (g, list) = r.draw(&view(Vec2::new(-188.0, 0.0)), &mut rng);
    assert!(list.items.is_empty());
    assert_eq!(rng, UnityRandom::new(9));
    assert_eq!(g.intensity, 0.0);
    assert_eq!(g.everywhere, 0.1, "a Rain room outside the arena keeps 0.1");
}

/// The splash's `SplashSize` scale, rotation range and colour from the fog colour.
#[test]
fn a_splash_is_scaled_by_the_splash_size() {
    let r = rain(DangerType::Rain, 0.25);
    let (_, list) = r.draw(&view(Vec2::new(-188.0, 0.0)), &mut UnityRandom::new(9));
    let drawn = sprites(&list.items);
    let size = 0.25_f32;
    for s in &drawn[1..] {
        assert!((s.scale.y - size).abs() < 1e-6);
        assert!(s.scale.x.abs() <= size + 1e-6);
        let limit = if s.index < 30 { 45.0 } else { 25.0 };
        assert!(s.rotation.abs() <= limit);
        assert!(s.color.r >= 0.2 && s.color.r <= 1.0);
        assert!(
            s.color.r - 0.2 <= (1.0 - 0.2) * 0.25 + 1e-6,
            "value * intensity"
        );
    }
}

/// `_rainDirection` is the lerp of the last and current direction; `_RainSpriteRect`
/// is the camera position over the room and 1366 x 768 over it; `_rainIntensity` is
/// `InverseLerp(0, 0.5, I)`; `_rainEverywhere` is `RainUnderCeilings` (`Max(0.1, ..)` for
/// a `Rain` room outside the arena).
#[test]
fn the_shader_globals() {
    let (g, _) = rain(DangerType::Rain, 0.75).draw(&view(Vec2::ZERO), &mut UnityRandom::new(1));
    assert!((g.direction - (-0.25)).abs() < 1e-6);
    let w = 20.0 * 30.0;
    let h = 20.0 * 20.0;
    assert_eq!(g.sprite_rect, [10.0 / w, 20.0 / h, 1366.0 / w, 768.0 / h]);
    assert_eq!(g.intensity, 1.0);
    assert!((g.everywhere - 0.5).abs() < 1e-6);
    let (g, _) =
        rain(DangerType::FloodAndRain, 0.3).draw(&view(Vec2::ZERO), &mut UnityRandom::new(1));
    assert!((g.intensity - 0.6).abs() < 1e-6);
    assert_eq!(g.everywhere, 0.0, "no floor of 0.1 outside a Rain room");
    let mut arena = view(Vec2::ZERO);
    arena.arena = true;
    let (g, _) = rain(DangerType::Rain, 0.3).draw(&arena, &mut UnityRandom::new(1));
    assert_eq!(g.everywhere, 0.0, "nor in the arena");
}
