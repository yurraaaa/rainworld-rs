//! The depth scenes built from the installed game's images and `positions.txt`
//! (read-only). Set `RW_DIR` to override the default install; tests skip when it is
//! absent.
//!
//! Expected values from the decompiled game (`Assembly-CSharp`, 1.11.8):
//! `MenuDepthIllustration` (541924: size `(w, h/2)`, `scaleY` 0.5, `DepthAtPosition`),
//! `MenuScene.BuildScene` (542836 main menu, 543854 Landscape_SU), `RefreshPositions`
//! (542656), `InteractiveMenuScene.Update` (536003: the idle focus, `Range` then
//! `value`; 536014: the depth under a still mouse). Positions from the installed
//! `scenes/landscape - su/positions.txt` and `scenes/main menu/positions.txt`.

use std::path::PathBuf;

use rw_assets::{PngImage, StreamingAssets, UnityResources};
use rw_menu::menu::{Kind, Menu, MenuCx, MenuScreen, MenuShader, ObjectId, SceneId};
use rw_menu::{MenuShared, ProcessId, ProcessRequests};
use rw_random::UnityRandom;
use rw_render::StageShader;
use rw_sim::{FloatExt, Vec2};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct NoScreen;

impl MenuScreen for NoScreen {
    fn signal(&mut self, _: &mut Menu, _: &mut MenuCx<'_>, _: ObjectId, _: &str) {}
}

fn shared() -> Option<MenuShared> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let data = root.join("RainWorld_Data");
    if !data.join("resources.assets").is_file() {
        eprintln!("skipping: no game under {}", root.display());
        return None;
    }
    let assets = StreamingAssets::new(data.join("StreamingAssets"));
    let resources = UnityResources::open(&data).expect("resources");
    let mut shared = MenuShared::load(assets, &resources, [1366.0, 768.0]).expect("fonts");
    shared.random = UnityRandom::new(5);
    Some(shared)
}

fn build(shared: &mut MenuShared, id: SceneId) -> (Menu, ObjectId) {
    let mut menu = Menu::new(ProcessId::MainMenu, shared);
    let page = menu.add_page(shared, "main", 0);
    let scene = menu.add_scene(shared, page, id);
    (menu, scene)
}

fn layers(menu: &Menu, scene: ObjectId) -> Vec<ObjectId> {
    let Kind::Scene(s) = &menu.object(scene).kind else {
        panic!("not a scene");
    };
    s.depth_illustrations.clone()
}

fn tick(shared: &mut MenuShared, menu: &mut Menu, scene: ObjectId) {
    let mut requests = ProcessRequests::default();
    let mut cx = MenuCx {
        shared,
        requests: &mut requests,
    };
    menu.update_object(scene, &mut NoScreen, &mut cx);
}

#[test]
fn landscape_su_is_three_depth_layers_at_their_positions() {
    let Some(mut shared) = shared() else { return };
    assert!(
        !shared.flat_illustrations,
        "the installed game has no flatmode.txt"
    );
    let before = shared.random.state();
    let (menu, scene) = build(&mut shared, SceneId::LandscapeSu);
    // The constructor's `Random.value`, `Custom.RNV()` and `Random.value`; BuildScene
    // draws nothing.
    let mut probe = UnityRandom::from_state(before);
    for _ in 0..3 {
        probe.value();
    }
    assert_eq!(probe.state(), shared.random.state());
    let ids = layers(&menu, scene);
    let expected = [
        ("SU_Landscape - 3", 6.0, Vec2::new(-183.0, 24.0)),
        ("SU_Landscape - 2", 2.5, Vec2::new(-134.0, -68.0)),
        ("SU_Landscape - 1", 1.25, Vec2::new(-222.0, -93.0)),
    ];
    assert_eq!(ids.len(), 3);
    for (&id, (file, depth, pos)) in ids.iter().zip(expected) {
        let o = menu.object(id);
        let Kind::Illustration(i) = &o.kind else {
            panic!("not an illustration");
        };
        assert_eq!(i.file_name, file);
        let d = i.depth.expect("a depth illustration");
        assert_eq!((d.depth, d.shader), (depth, MenuShader::Normal));
        assert_eq!((o.pos, o.last_pos), (pos, pos));
        let path = shared
            .assets
            .resolve_file_path(&format!("Scenes/Landscape - SU/{file}.png"));
        let png = PngImage::load(&path).expect("layer png");
        assert_eq!(
            o.size,
            Vec2::new(png.width() as f32, png.height() as f32 / 2.0)
        );
        let sprite = menu.stage.sprite(i.sprite);
        assert_eq!(sprite.shader, StageShader::SceneBlur);
        assert_eq!(sprite.transform.scale_y, 0.5);
        assert_eq!(sprite.anchor, [0.0, 0.0]);
    }
}

#[test]
fn main_menu_is_fifteen_layers_with_their_alphas() {
    let Some(mut shared) = shared() else { return };
    let (menu, scene) = build(&mut shared, SceneId::MainMenu);
    let ids = layers(&menu, scene);
    assert_eq!(ids.len(), 15);
    let o = menu.object(ids[3]);
    let Kind::Illustration(i) = &o.kind else {
        panic!("not an illustration");
    };
    // `MainVines3` comes from `Illustrations`, placed by line 4 of positions.txt.
    assert_eq!(i.file_name, "MainVines3");
    assert_eq!(o.pos, Vec2::new(332.0, 645.0));
    assert!(o.size.x > 100.0);
    let alphas: Vec<Option<f32>> = ids
        .iter()
        .map(|&id| match &menu.object(id).kind {
            Kind::Illustration(i) => i.set_alpha,
            _ => None,
        })
        .collect();
    assert_eq!(alphas[8..11], [Some(0.66), Some(0.21), Some(0.55)]);
    assert!(alphas[..8].iter().chain(&alphas[11..]).all(Option::is_none));
    // Layer 14 (`MainVines1`) from line 15.
    assert_eq!(menu.object(ids[14]).pos, Vec2::new(904.0, 121.0));
}

/// A seed whose first `Random.value` is (or is not) under 0.02.
fn seed_with_first_value(below: bool) -> UnityRandom {
    (0..)
        .map(UnityRandom::new)
        .find(|r| (r.clone().value() < 0.02) == below)
        .expect("a seed")
}

#[test]
fn idle_focus_picks_a_layer_depth_with_two_more_draws() {
    let Some(mut shared) = shared() else { return };
    let (mut menu, scene) = build(&mut shared, SceneId::MainMenu);
    let depths: Vec<f32> = layers(&menu, scene)
        .iter()
        .map(|&id| match &menu.object(id).kind {
            Kind::Illustration(i) => i.depth.expect("depth").depth,
            _ => panic!("not an illustration"),
        })
        .collect();
    if let Some(s) = menu.scene_mut(scene) {
        s.focus = 0.5;
        s.get_to_focus = 0.5;
    }
    shared.random = seed_with_first_value(true);
    let mut probe = shared.random.clone();
    tick(&mut shared, &mut menu, scene);
    probe.value();
    let i = probe.range_i32(0, 15) as usize;
    let jitter = probe.value().mathf_pow(1.5) * 0.5;
    let Kind::Scene(s) = &menu.object(scene).kind else {
        panic!("not a scene");
    };
    assert_eq!(s.get_to_focus, (depths[i] + jitter).inverse_lerp(1.0, 10.0));

    // Without the first draw under 0.02 a depth scene draws as a flat one.
    let start = seed_with_first_value(false);
    let mut ends = Vec::new();
    for flat in [false, true] {
        shared.flat_illustrations = flat;
        let (mut menu, scene) = build(&mut shared, SceneId::MainMenu);
        if let Some(s) = menu.scene_mut(scene) {
            s.focus = 0.5;
            s.get_to_focus = 0.5;
        }
        shared.random = start.clone();
        tick(&mut shared, &mut menu, scene);
        ends.push(shared.random.state());
    }
    assert_eq!(ends[0], ends[1]);
}

#[test]
fn a_still_mouse_focuses_on_the_depth_under_it() {
    let Some(mut shared) = shared() else { return };
    let (mut menu, scene) = build(&mut shared, SceneId::LandscapeSu);
    let near = layers(&menu, scene)[2];
    if let Some(s) = menu.scene_mut(scene) {
        s.cam_pos = Vec2::ZERO;
        s.last_cam_pos = Vec2::ZERO;
        // `num` = 1: the sine of `InverseLerp(10, 200, 105)` is 1.
        s.mouse_inactive_counter = 105;
        s.time_since_mouse_on_button_counter = 100;
    }
    // A pixel of the nearest layer (`SU_Landscape - 1` at (-222, -93), depth 1.25),
    // read from the file: depth map in the lower half, colour (alpha) in the upper.
    let path = shared
        .assets
        .resolve_file_path("Scenes/Landscape - SU/SU_Landscape - 1.png");
    let png = PngImage::load(&path).expect("layer png");
    let half = png.height() / 2;
    let (x, y) = (100..half)
        .step_by(7)
        .flat_map(|y| (100..png.width() - 100).step_by(13).map(move |x| (x, y)))
        .find(|&(x, y)| png.unity_pixel(x, half + y)[3] == 255)
        .expect("an opaque pixel");
    let red = f32::from(png.unity_pixel(x, y)[0]) / 255.0;
    let mouse = Vec2::new(-222.0 + x as f32 + 0.5, -93.0 + y as f32 + 0.5);
    let depth = menu.depth_at_position(&shared.atlases, near, mouse);
    assert_eq!(depth, (1.0 - red) * 2.0 + 1.25);
    assert_eq!(
        menu.depth_at_position(&shared.atlases, near, Vec2::new(-300.0, 0.0)),
        -1.0
    );
    menu.mouse_pos = mouse;
    menu.last_mouse_pos = mouse;
    let min = menu.min_depth_at(&shared.atlases, scene, mouse);
    assert!(min <= depth);
    tick(&mut shared, &mut menu, scene);
    let Kind::Scene(s) = &menu.object(scene).kind else {
        panic!("not a scene");
    };
    assert_eq!(s.get_to_focus, min.inverse_lerp(1.0, 10.0));
}
