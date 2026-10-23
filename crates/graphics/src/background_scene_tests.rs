//! Scene element sprites: positions, scales and colours as `DrawSprites` of
//! `AboveCloudsView.cs` and `RoofTopView.cs` leave them, worked by hand from the game's
//! formulas for one camera, and the cosmetic draws of the lightning.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;
use rw_sim::{AboveCloudsContext, BackgroundScene, ElementKind, SCREEN_SIZE, SceneElement, Vec2};

use super::{BackgroundSceneSprites, BackgroundSceneView, SceneImageSizes};
use crate::draw::{Color, Layer, Shader};

fn scene(region: &str, room: &str) -> BackgroundScene {
    let mut scene = BackgroundScene::above_clouds_view(&AboveCloudsContext {
        region: Some(region),
        room_name: room,
        room_origin: Vec2::new(1000.0, 2000.0),
        arena: false,
        effect_amount: 1.0,
        clouds: (None, None),
    });
    scene.convergence_point = BackgroundScene::convergence_for(SCREEN_SIZE);
    scene
}

fn sizes() -> SceneImageSizes {
    let mut s = SceneImageSizes::default();
    s.insert("clouds1", Vec2::new(700.0, 150.0));
    s.insert("clouds2", Vec2::new(700.0, 150.0));
    s.insert("clouds3", Vec2::new(700.0, 150.0));
    s.insert("floor", Vec2::new(1355.0, 230.0));
    s
}

/// The camera at which `RoomToWorldPos(camPos)` is the scene's origin, hDisplace 8 (so
/// `DrawPos` adds nothing sideways), on a camera whose position has world height `y`.
fn view(scene: &BackgroundScene, camera_world_y: f32) -> BackgroundSceneView {
    BackgroundSceneView {
        cam_pos: scene.scene_origo - scene.room_origin,
        camera_position: Vec2::new(0.0, camera_world_y - scene.room_origin.y),
        h_displace: 8.0,
        screen: SCREEN_SIZE,
        sky_color: Color::rgb(0.2, 0.4, 0.6),
        time_stacker: 0.5,
    }
}

fn element<'a>(s: &'a BackgroundScene, pick: impl Fn(&ElementKind) -> bool) -> &'a SceneElement {
    s.elements.iter().find(|e| pick(&e.kind)).unwrap()
}

fn u8c(v: f32) -> f32 {
    (v * 255.0).round_ties_even() / 255.0
}

#[test]
fn a_distant_building_sits_at_its_input_with_a_haze_by_depth() {
    let s = scene("UW", "UW_A01");
    let v = view(&s, 20000.0);
    let e = element(&s, |k| {
        matches!(
            k,
            ElementKind::DistantBuilding {
                asset: "AtC_Structure1",
                ..
            }
        )
    });
    let list = BackgroundSceneSprites::draw(&s, e, &v, &sizes(), &mut UnityRandom::new(1));
    let sprite = list.sprite(0).unwrap();
    // The constructor placed it with a zero convergence point (pos = input * depth), so
    // at the neutral camera DrawPos gives input + c - c / depth, c = (683, 512).
    let screen = sprite.pos - v.cam_pos;
    assert!((screen.x - (-520.0 + 683.0 - 683.0 / 160.0)).abs() < 1e-2);
    assert!((screen.y - (-85.0 + 512.0 - 512.0 / 160.0)).abs() < 1e-2);
    assert_eq!(
        (sprite.anchor, sprite.scale),
        (Vec2::new(0.5, 0.0), Vec2::new(1.0, 1.0))
    );
    assert_eq!(
        (sprite.shader, sprite.layer),
        (Shader::DistantBkgObject, Layer::Water)
    );
    // (InverseLerp(0, 600, 160 - 20) ^ 0.3) * 0.9, in 8 bits.
    let red = u8c(((140.0_f64 / 600.0).powf(0.3) * 0.9) as f32);
    assert_eq!(
        (
            sprite.color.r,
            sprite.color.g,
            sprite.color.b,
            sprite.color.a
        ),
        (red, 0.0, 0.0, 1.0)
    );
}

#[test]
fn the_fog_fades_with_altitude_and_is_the_skys_colour() {
    let s = scene("UW", "UW_A01");
    // World height 20500: InverseLerp(22000, 18000, 20500) = 0.375, times 0.6.
    let mut v = view(&s, 20000.0);
    v.cam_pos = Vec2::new(0.0, 20500.0 - s.room_origin.y);
    let e = element(&s, |k| matches!(k, ElementKind::Fog));
    let list = BackgroundSceneSprites::draw(&s, e, &v, &sizes(), &mut UnityRandom::new(1));
    let sprite = list.sprite(0).unwrap();
    assert_eq!(sprite.element, "pixel");
    assert_eq!(sprite.scale, Vec2::new(1386.0, 788.0));
    assert_eq!(sprite.pos - v.cam_pos, Vec2::new(683.0, 384.0));
    assert_eq!(sprite.color.a, u8c(0.375 * 0.6));
    assert_eq!(sprite.color.r, u8c(0.2));
    // The Sky Islands use InverseLerp(16000, 9000, y): 0 above 16000.
    let si = scene("SI", "SI_A06");
    let list =
        BackgroundSceneSprites::draw(&si, &si.elements[0], &v, &sizes(), &mut UnityRandom::new(1));
    assert_eq!(list.sprite(0).unwrap().color.a, 0.0);
}

#[test]
fn the_lowest_close_cloud_at_ground_height() {
    let s = scene("UW", "UW_A01");
    // The camera at world height 2000: InverseLerp(20000, 31400, 2000) = 0.
    let v = view(&s, 2000.0);
    let e = element(
        &s,
        |k| matches!(k, ElementKind::CloseCloud { cloud, .. } if cloud.index == 0),
    );
    let ElementKind::CloseCloud { cloud, .. } = &e.kind else {
        unreachable!()
    };
    let list = BackgroundSceneSprites::draw(&s, e, &v, &sizes(), &mut UnityRandom::new(1));
    let (fill, cloud_sprite) = (list.sprite(0).unwrap(), list.sprite(1).unwrap());
    // cloudDepth 0: depth 5, scale 10, num4 1; y2 = DrawPos(...).y - 100.
    let y2 = s.draw_pos(e.pos, 5.0, v.cam_pos, 8.0).y - 100.0;
    assert_eq!(cloud_sprite.element, "clouds1");
    assert_eq!(cloud_sprite.scale, Vec2::new(10.0, 10.0));
    assert_eq!(cloud_sprite.shader, Shader::Cloud);
    assert_eq!(cloud_sprite.anchor, Vec2::new(0.5, 0.0));
    let y = (y2 - 150.0 * 10.0).floor();
    assert_eq!(cloud_sprite.pos - v.cam_pos, Vec2::new(683.0, y));
    assert_eq!(
        (
            cloud_sprite.color.r,
            cloud_sprite.color.g,
            cloud_sprite.color.b
        ),
        (0.0, u8c(cloud.random_offset), 1.0)
    );
    // The fill under it: a pixel 1400 wide up to the cloud's y + 1, the sky blended 0 to
    // the atmosphere.
    assert_eq!(
        (fill.element, fill.shader, fill.scale),
        ("pixel", Shader::Background, Vec2::new(1400.0, y + 1.0))
    );
    assert_eq!(fill.color.r, u8c(0.2));
    assert_eq!(fill.pos - v.cam_pos, Vec2::new(683.0, 0.0));
}

#[test]
fn distant_and_flying_clouds_hide_below_a_third_of_the_altitude_range() {
    let s = scene("UW", "UW_A01");
    let distant = element(&s, |k| matches!(k, ElementKind::DistantCloud { .. }));
    let flying = element(&s, |k| matches!(k, ElementKind::FlyingCloud { .. }));
    // 33% of 20000..31400 is 23762.
    let low = view(&s, 23700.0);
    for e in [distant, flying] {
        let list = BackgroundSceneSprites::draw(&s, e, &low, &sizes(), &mut UnityRandom::new(1));
        assert!(list.items.is_empty());
    }
    let high = view(&s, 23800.0);
    let list = BackgroundSceneSprites::draw(&s, distant, &high, &sizes(), &mut UnityRandom::new(1));
    assert_eq!(list.items.len(), 2);
    let list = BackgroundSceneSprites::draw(&s, flying, &high, &sizes(), &mut UnityRandom::new(1));
    let f = list.sprite(0).unwrap();
    // The first flying cloud: scaleY flattened * 2, anchorY 1, alpha 0.5 in the colour.
    assert_eq!(
        (f.scale, f.anchor, f.shader),
        (
            Vec2::new(2.0, 0.7),
            Vec2::new(0.5, 1.0),
            Shader::CloudDistant
        )
    );
    assert_eq!((f.color.r, f.color.a), (u8c(0.9), u8c(0.5)));
    assert_eq!(f.color.b, u8c(0.35_f32 + (1.0 - 0.35) * 0.5));
}

#[test]
fn a_lightning_draws_its_brightness_on_the_cosmetic_stream() {
    let s = scene("UW", "UW_A01");
    let e = element(&s, |k| matches!(k, ElementKind::DistantLightning(_)));
    // Low altitude: the alpha is zero, but the brightness still draws (1 or 3 values).
    let v = view(&s, 2000.0);
    let mut rng = UnityRandom::new(4);
    let mut probe = rng.clone();
    let first = probe.value();
    let list = BackgroundSceneSprites::draw(&s, e, &v, &sizes(), &mut rng);
    let drawn = if first < 1.0 / 3.0 { 3 } else { 1 };
    let mut reference = UnityRandom::new(4);
    for _ in 0..drawn {
        reference.next_u32();
    }
    assert_eq!(rng.state(), reference.state());
    let sprite = list.sprite(0).unwrap();
    assert_eq!(sprite.color.a, 0.0);
    assert_eq!(sprite.shader, Shader::Background);
    assert_eq!(sprite.anchor.y, 0.0);
}

#[test]
fn the_roof_top_floor_spans_its_depths() {
    let mut rng = UnityRandom::new(3);
    let mut s = BackgroundScene::roof_top_view(Vec2::new(500.0, 600.0), (400, 80), &mut rng);
    s.convergence_point = BackgroundScene::convergence_for(SCREEN_SIZE);
    let v = BackgroundSceneView {
        cam_pos: Vec2::new(100.0, 50.0),
        camera_position: Vec2::ZERO,
        h_displace: 8.0,
        screen: SCREEN_SIZE,
        sky_color: Color::BLACK,
        time_stacker: 0.0,
    };
    let e = element(&s, |k| matches!(k, ElementKind::Floor { .. }));
    let list = BackgroundSceneSprites::draw(&s, e, &v, &sizes(), &mut rng);
    let f = list.sprite(0).unwrap();
    let near = s.draw_pos(e.pos, 12.0, v.cam_pos, 8.0);
    let far = s.draw_pos(e.pos, 1.0, v.cam_pos, 8.0);
    assert_eq!(f.shader, Shader::BkgFloor);
    assert_eq!(f.anchor, Vec2::new(0.5, 1.0));
    assert!((f.scale.x - 1400.0 / 1355.0).abs() < 1e-6);
    assert!((f.scale.y - (near.y - far.y) / 230.0).abs() < 1e-4);
    assert_eq!(f.pos - v.cam_pos, Vec2::new(683.0, near.y));
    assert_eq!(
        (f.color.r, f.color.g, f.color.b),
        (1.0, u8c(1.0 / 12.0), 0.0)
    );
}
