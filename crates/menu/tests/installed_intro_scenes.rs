//! The intro slideshows' scenes in depth, built from the installed game's images,
//! `positions.txt` and `camera.txt` (read-only). Set `RW_DIR` to override the default
//! install; tests skip when it is absent.
//!
//! Expected values from the decompiled game (`Assembly-CSharp`, 1.11.8):
//! `SlideShowMenuScene` (the constructor's `camPos = CameraPosition(0)` without
//! `/ 300`; `Update`; `ApplySceneSpecificAlphas`), `MenuScene.BuildScene`
//! (the intro tables) and the installed
//! `scenes/intro 1 - tree/camera.txt` (first point `-416, 170, 0.1415929`).

use std::path::PathBuf;

use rw_assets::{StreamingAssets, UnityResources};
use rw_menu::menu::{CameraPath, Kind, Menu, MenuCx, MenuScreen, ObjectId, SceneId};
use rw_menu::{MenuShared, ProcessId, ProcessRequests};
use rw_random::UnityRandom;
use rw_sim::{FloatExt, Vec2};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

/// The intro scenes with a depth table, and their layer counts.
const INTRO_SCENES: [(SceneId, usize); 15] = [
    (SceneId::Intro1Tree, 11),
    (SceneId::Intro2Branch, 6),
    (SceneId::Intro3InTree, 14),
    (SceneId::Intro4Walking, 7),
    (SceneId::Intro5Hunting, 13),
    (SceneId::Intro67RainDrop, 5),
    (SceneId::Intro8Climbing, 12),
    (SceneId::Intro9RainyClimb, 15),
    (SceneId::Intro10Fall, 19),
    (SceneId::Intro105Separation, 6),
    (SceneId::Intro11Drowning, 11),
    (SceneId::Intro12Waking, 3),
    (SceneId::Intro13Alone, 11),
    (SceneId::YellowIntroA, 10),
    (SceneId::YellowIntroB, 5),
];

/// A slideshow at a fixed scene time.
struct Slides {
    time: f32,
}

impl MenuScreen for Slides {
    fn signal(&mut self, _: &mut Menu, _: &mut MenuCx<'_>, _: ObjectId, _: &str) {}

    fn scene_update(&mut self, menu: &mut Menu, scene: ObjectId) {
        menu.update_slide_show_scene(scene, self.time);
    }
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
    shared.random = UnityRandom::new(7);
    Some(shared)
}

fn build(shared: &mut MenuShared, id: SceneId) -> (Menu, ObjectId) {
    let mut menu = Menu::new(ProcessId::SlideShow, shared);
    let page = menu.add_page(shared, "main", 0);
    let scene = menu.add_slide_show_scene(shared, page, id);
    menu.add_sub(page, scene);
    (menu, scene)
}

fn tick(shared: &mut MenuShared, menu: &mut Menu, scene: ObjectId, time: f32) {
    let mut requests = ProcessRequests::default();
    let mut cx = MenuCx {
        shared,
        requests: &mut requests,
    };
    menu.update_object(scene, &mut Slides { time }, &mut cx);
}

fn scene(menu: &Menu, scene: ObjectId) -> &rw_menu::menu::SceneData {
    let Kind::Scene(s) = &menu.object(scene).kind else {
        panic!("not a scene");
    };
    s
}

fn alpha(menu: &Menu, id: ObjectId) -> f32 {
    match &menu.object(id).kind {
        Kind::Illustration(i) => i.alpha,
        _ => panic!("not an illustration"),
    }
}

/// Every intro scene builds its depth table with each layer's image found (not the
/// 1 x 1 stand-in), every layer placed from `positions.txt` (a missing line keeps the
/// table's place), and
/// its camera path from `camera.txt`.
#[test]
fn intro_scenes_build_in_depth_from_their_files() {
    let Some(mut shared) = shared() else { return };
    assert!(!shared.flat_illustrations);
    for (id, n) in INTRO_SCENES {
        let (menu, s) = build(&mut shared, id);
        let data = scene(&menu, s);
        assert!(!data.flat_mode && data.flat_illustrations.is_empty());
        assert_eq!(data.depth_illustrations.len(), n, "{id:?}");
        for &layer in &data.depth_illustrations {
            let size = menu.object(layer).size;
            assert!(size.x > 1.0 && size.y > 1.0, "{id:?}: an image missing");
        }
        let folder = id.slide_show_folder();
        let positions = rw_menu::menu::DepthScene::read_positions(&shared.assets, folder, false);
        // Intro 10's `positions.txt` has 18 lines: `WhiteFade` keeps its (0, 0).
        let lines = if id == SceneId::Intro10Fall { 18 } else { n };
        assert_eq!(positions.len().min(n), lines, "{id:?}");
        let table = id.slide_show_depth_scene().expect("a table");
        for (i, &layer) in data.depth_illustrations.iter().enumerate() {
            let want = positions.get(i).copied().unwrap_or(table.layers[i].pos);
            assert_eq!(menu.object(layer).pos, want, "{id:?} layer {i}");
        }
        let path = CameraPath::read(&shared.assets, folder);
        assert!(path.points.len() > 1, "{id:?}: camera.txt");
        assert_eq!(data.camera_path, path);
    }
    // `Empty` and the title have no folder: one camera point at zero.
    for id in [SceneId::Empty, SceneId::Intro14Title] {
        let (menu, s) = build(&mut shared, id);
        assert_eq!(scene(&menu, s).camera_path.points.len(), 1);
        assert_eq!(scene(&menu, s).cam_pos, Vec2::ZERO);
    }
}

/// The constructor puts the camera at the first point undivided; the first update
/// divides by 300 (and `lastCamPos` keeps the undivided value for that tick's frames).
/// The focus is the third column, -1 points interpolated.
#[test]
fn the_first_camera_position_is_not_divided() {
    let Some(mut shared) = shared() else { return };
    let (mut menu, s) = build(&mut shared, SceneId::Intro1Tree);
    let first = Vec2::new(-416.0, 170.0);
    let data = scene(&menu, s);
    assert_eq!((data.cam_pos, data.last_cam_pos), (first, first));
    assert_eq!(data.focus, 0.141_592_9);
    tick(&mut shared, &mut menu, s, 0.0);
    let data = scene(&menu, s);
    assert_eq!(data.last_cam_pos, first);
    assert_eq!(data.cam_pos, first / 300.0);
    // 9 points: f = 1/16 is halfway along segment 0, between (z 0.1415929) and point
    // 2 (z 0.3539823) past the interpolated point 1.
    tick(&mut shared, &mut menu, s, 1.0 / 16.0);
    let data = scene(&menu, s);
    let want = 0.141_592_9_f32.lerp(0.353_982_3, 0.25);
    assert!((data.focus - want).abs() < 1e-6, "{} vs {want}", data.focus);
    assert_eq!(data.last_cam_pos, first / 300.0);
    // At the end the camera is the last point.
    tick(&mut shared, &mut menu, s, 1.0);
    let data = scene(&menu, s);
    assert_eq!(data.cam_pos, Vec2::new(326.0, -23.0) / 300.0);
    assert_eq!(data.focus, 0.306_784_7);
}

/// `SlideShowMenuScene` makes no random draws: building every intro scene (and the
/// two without a table) and running each for 80 ticks across its time leaves the
/// stream where it was.
#[test]
fn slideshow_scenes_draw_nothing() {
    let Some(mut shared) = shared() else { return };
    let before = shared.random.state();
    let ids = INTRO_SCENES
        .iter()
        .map(|&(id, _)| id)
        .chain([SceneId::Empty, SceneId::Intro14Title]);
    for id in ids {
        let (mut menu, s) = build(&mut shared, id);
        for k in 0..80 {
            tick(&mut shared, &mut menu, s, k as f32 / 79.0);
        }
    }
    assert_eq!(shared.random.state(), before);
}

/// The depth rules of `ApplySceneSpecificAlphas`: the rain drop crossfades layers 0 to
/// 4 by `setAlpha` (taken on the next tick), Yellow A's last two and Yellow B's layers
/// 1 and 2 have `alpha` itself set (this tick), Intro 9 and 10 fade their `WhiteFade`.
#[test]
fn depth_alphas_follow_the_display_time() {
    let Some(mut shared) = shared() else { return };
    let (mut menu, s) = build(&mut shared, SceneId::Intro67RainDrop);
    let layers = scene(&menu, s).depth_illustrations.clone();
    let t = 0.55;
    tick(&mut shared, &mut menu, s, t);
    // The constructor's `setAlpha`s (display time 0) were taken by this tick.
    assert_eq!(alpha(&menu, layers[0]), 1.0);
    assert_eq!(alpha(&menu, layers[1]), 0.0);
    tick(&mut shared, &mut menu, s, t);
    let num = t.inverse_lerp(0.52, 0.58);
    let (off, on) = ((1.0 - num).powf(0.5), num.powf(0.5));
    for (i, want) in [off, on, off, on, on].into_iter().enumerate() {
        assert!((alpha(&menu, layers[i]) - want).abs() < 1e-6, "layer {i}");
    }

    let (mut menu, s) = build(&mut shared, SceneId::YellowIntroA);
    let layers = scene(&menu, s).depth_illustrations.clone();
    // Set at once by the constructor (display time 0), and its `lastAlpha` copied.
    assert_eq!(alpha(&menu, layers[9]), 0.0);
    let t = 0.53;
    tick(&mut shared, &mut menu, s, t);
    let num = t.inverse_lerp(0.51, 0.55);
    assert!((alpha(&menu, layers[8]) - num.powf(1.5) * 0.65).abs() < 1e-6);
    assert!((alpha(&menu, layers[9]) - num.powf(0.5)).abs() < 1e-6);

    let (mut menu, s) = build(&mut shared, SceneId::YellowIntroB);
    let layers = scene(&menu, s).depth_illustrations.clone();
    let t = 0.3;
    tick(&mut shared, &mut menu, s, t);
    let first = 1.0 + (0.2 - 1.0) * t.inverse_lerp(0.1, 0.9).powf(0.7);
    let second = 0.5 + (1.0 - 0.5) * t.inverse_lerp(0.1, 0.5).powf(1.2);
    assert!((alpha(&menu, layers[1]) - first).abs() < 1e-6);
    assert!((alpha(&menu, layers[2]) - second).abs() < 1e-6);

    for (id, t, want) in [
        (SceneId::Intro9RainyClimb, 0.975, 0.5),
        (SceneId::Intro10Fall, 0.025, 0.5),
    ] {
        let (mut menu, s) = build(&mut shared, id);
        let fade = *scene(&menu, s).depth_illustrations.last().unwrap();
        tick(&mut shared, &mut menu, s, t);
        tick(&mut shared, &mut menu, s, t);
        assert!((alpha(&menu, fade) - want).abs() < 1e-5, "{id:?}");
    }
}
