//! The sleep, death and starve screens' flat scenes built from the installed game's
//! images (read-only). Set `RW_DIR` to override the default install; tests skip when
//! it is absent.
//!
//! Expected values from the decompiled game (`Assembly-CSharp`, 1.11.8):
//! `MenuScene.BuildScene` (542857 sleep, 542963 starve, 544337 new death),
//! `InteractiveMenuScene` (535848: its constructor's three draws, `Update`'s NewDeath
//! flat alpha `InverseLerp(0, 160, timer)` and `timer++` while >= 0).

use std::path::PathBuf;

use rw_assets::{SlugcatName, StreamingAssets, UnityResources};
use rw_menu::menu::{Kind, Menu, MenuCx, MenuScreen, ObjectId, SceneId};
use rw_menu::{MenuShared, ProcessId, ProcessRequests};
use rw_random::UnityRandom;
use rw_save::SaveState;

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct NoScreen;

/// A menu with one scene, updated by hand.
struct Rig {
    shared: MenuShared,
    menu: Menu,
    scene: ObjectId,
    requests: ProcessRequests,
}

impl Rig {
    fn tick(&mut self) {
        let mut cx = MenuCx {
            shared: &mut self.shared,
            requests: &mut self.requests,
        };
        self.menu.update_object(self.scene, &mut NoScreen, &mut cx);
    }

    fn flower_alpha(&self) -> f32 {
        flats(&self.menu, self.scene)[1].2
    }
}

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

/// The scene's flat illustrations: file name, size and alpha.
fn flats(menu: &Menu, scene: ObjectId) -> Vec<(String, [f32; 2], f32)> {
    let Kind::Scene(s) = &menu.object(scene).kind else {
        panic!("not a scene");
    };
    s.flat_illustrations
        .iter()
        .map(|&f| {
            let o = menu.object(f);
            let Kind::Illustration(i) = &o.kind else {
                panic!("not an illustration");
            };
            (i.file_name.clone(), o.size.to_array(), i.alpha)
        })
        .collect()
}

#[test]
fn sleep_scene_follows_the_playing_slugcat() {
    let Some(mut shared) = shared() else { return };
    for (slugcat, file) in [
        (SlugcatName::White, "Sleep Screen - White - Flat"),
        (SlugcatName::Yellow, "Sleep Screen - Yellow - Flat"),
        (SlugcatName::Red, "Sleep Screen - Red - Flat"),
    ] {
        // `PlayingAsSlugcat`: the current save state's slugcat.
        shared.saves.current_save_state = Some(SaveState::new(slugcat, 0));
        let mut menu = Menu::new(ProcessId::SleepScreen, &shared);
        let page = menu.add_page(&shared, "main", 0);
        let scene = menu.add_scene(&mut shared, page, SceneId::SleepScreen);
        let f = flats(&menu, scene);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].0, file);
        // The installed image was found (a missing one is 1 x 1).
        assert!(f[0].1[0] > 100.0, "{file}: {:?}", f[0].1);
    }
}

#[test]
fn death_scene_flower_fades_in_with_the_timer() {
    let Some(mut shared) = shared() else { return };
    let mut menu = Menu::new(ProcessId::DeathScreen, &shared);
    let page = menu.add_page(&shared, "main", 0);
    let before = shared.random.state();
    let scene = menu.add_scene(&mut shared, page, SceneId::NewDeath);
    // The constructor: `Random.value`, `Custom.RNV()` (one draw) and `Random.value`.
    let mut probe = UnityRandom::from_state(before);
    for _ in 0..3 {
        probe.value();
    }
    assert_eq!(probe.state(), shared.random.state());
    let f = flats(&menu, scene);
    let names: Vec<&str> = f.iter().map(|(n, _, _)| n.as_str()).collect();
    assert_eq!(names, ["New Death - Flat", "New Death Flower - Flat"]);
    assert!(f.iter().all(|(_, size, _)| size[0] > 100.0));
    let mut rig = Rig {
        shared,
        menu,
        scene,
        requests: ProcessRequests::default(),
    };
    // The flower starts at alpha 1; its `setAlpha` of 0 (timer -1) applies a tick later.
    assert_eq!(rig.flower_alpha(), 1.0);
    rig.tick();
    assert_eq!(rig.flower_alpha(), 1.0);
    rig.tick();
    assert_eq!(rig.flower_alpha(), 0.0);
    // `FoodCountDownDone` with `showFlower`: the timer starts at 0 and counts each tick.
    rig.menu.start_scene_timer(scene);
    for _ in 0..41 {
        rig.tick();
    }
    // Set from timer 39 (after 40 updates) and applied on the 41st.
    assert!((rig.flower_alpha() - 39.0 / 160.0).abs() < 1e-6);
    assert_eq!(flats(&rig.menu, scene)[0].2, 1.0);
}

#[test]
fn starve_scene_is_one_flat_image() {
    let Some(mut shared) = shared() else { return };
    let mut menu = Menu::new(ProcessId::StarveScreen, &shared);
    let page = menu.add_page(&shared, "main", 0);
    let scene = menu.add_scene(&mut shared, page, SceneId::StarveScreen);
    let f = flats(&menu, scene);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].0, "Starve Screen - Flat");
    assert!(f[0].1[0] > 100.0);
}
