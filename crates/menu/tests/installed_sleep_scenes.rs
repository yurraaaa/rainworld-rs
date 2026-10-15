//! The sleep, death and starve screens' scenes, flat and in depth, built from the
//! installed game's images (read-only). Set `RW_DIR` to override the default install;
//! tests skip when it is absent.
//!
//! Expected values from the decompiled game (`Assembly-CSharp`, 1.11.8):
//! `MenuScene.BuildScene` (542857 sleep, 542963 starve, 544337 new death),
//! `InteractiveMenuScene` (535848: its constructor's three draws, `Update`'s NewDeath
//! alphas over `InverseLerp(0, 160, timer)` (535904) and `timer++` while >= 0; the idle
//! focus pick, `Range` over `idleDepths` then `value`). Positions from the installed
//! `positions.txt` of `scenes/sleep screen - white|yellow|red`, `new death` and
//! `starve screen`.

use std::path::PathBuf;

use rw_assets::{SlugcatName, StreamingAssets, UnityResources};
use rw_menu::menu::{Kind, Menu, MenuCx, MenuScreen, ObjectId, SceneId};
use rw_menu::{MenuShared, ProcessId, ProcessRequests};
use rw_random::UnityRandom;
use rw_save::SaveState;
use rw_sim::{FloatExt, Vec2};

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
fn flat_sleep_scene_follows_the_playing_slugcat() {
    let Some(mut shared) = shared() else { return };
    shared.flat_illustrations = true;
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
fn flat_death_scene_flower_fades_in_with_the_timer() {
    let Some(mut shared) = shared() else { return };
    shared.flat_illustrations = true;
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
fn flat_starve_scene_is_one_image() {
    let Some(mut shared) = shared() else { return };
    shared.flat_illustrations = true;
    let mut menu = Menu::new(ProcessId::StarveScreen, &shared);
    let page = menu.add_page(&shared, "main", 0);
    let scene = menu.add_scene(&mut shared, page, SceneId::StarveScreen);
    let f = flats(&menu, scene);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].0, "Starve Screen - Flat");
    assert!(f[0].1[0] > 100.0);
}

/// The scene's depth layers: file name, position, depth and alpha.
fn depths(menu: &Menu, scene: ObjectId) -> Vec<(String, Vec2, f32, f32)> {
    let Kind::Scene(s) = &menu.object(scene).kind else {
        panic!("not a scene");
    };
    s.depth_illustrations
        .iter()
        .map(|&l| {
            let o = menu.object(l);
            let Kind::Illustration(i) = &o.kind else {
                panic!("not an illustration");
            };
            let d = i.depth.expect("a depth layer");
            (i.file_name.clone(), o.pos, d.depth, i.alpha)
        })
        .collect()
}

impl Rig {
    fn open(mut shared: MenuShared, id: ProcessId, scene: SceneId) -> Self {
        let mut menu = Menu::new(id, &shared);
        let page = menu.add_page(&shared, "main", 0);
        let scene = menu.add_scene(&mut shared, page, scene);
        Self {
            shared,
            menu,
            scene,
            requests: ProcessRequests::default(),
        }
    }

    fn depth_alphas(&self) -> Vec<f32> {
        depths(&self.menu, self.scene).iter().map(|l| l.3).collect()
    }
}

#[test]
fn depth_sleep_scene_per_slugcat() {
    let Some(mut shared) = shared() else { return };
    assert!(!shared.flat_illustrations, "no flatmode.txt installed");
    for (slugcat, slug_layer, slug_pos) in [
        (
            SlugcatName::White,
            "Sleep - 2 - White",
            Vec2::new(677.0, 63.0),
        ),
        (
            SlugcatName::Yellow,
            "Sleep - 2 - Yellow",
            Vec2::new(782.0, 117.0),
        ),
        (SlugcatName::Red, "Sleep - 2 - Red", Vec2::new(817.0, 112.0)),
    ] {
        shared.saves.current_save_state = Some(SaveState::new(slugcat, 0));
        let mut menu = Menu::new(ProcessId::SleepScreen, &shared);
        let page = menu.add_page(&shared, "main", 0);
        let scene = menu.add_scene(&mut shared, page, SceneId::SleepScreen);
        let d = depths(&menu, scene);
        let names: Vec<&str> = d.iter().map(|l| l.0.as_str()).collect();
        assert_eq!(
            names,
            [
                "Sleep - 5",
                "Sleep - 4",
                "Sleep - 3",
                slug_layer,
                "Sleep - 1"
            ]
        );
        assert_eq!(
            d.iter().map(|l| l.2).collect::<Vec<_>>(),
            [3.5, 2.8, 2.2, 1.7, 1.2]
        );
        assert_eq!(d[0].1, Vec2::new(672.0, 236.0));
        assert_eq!(d[3].1, slug_pos);
        assert_eq!(d[4].1, Vec2::new(486.0, -54.0));
        let Kind::Scene(s) = &menu.object(scene).kind else {
            panic!("not a scene");
        };
        assert_eq!(s.idle_depths, [3.3, 2.7, 1.8, 1.7, 1.6, 1.2]);
        assert!(s.flat_illustrations.is_empty());
        // The installed images were found (a missing one is 1 x 1).
        assert!(
            s.depth_illustrations
                .iter()
                .all(|&l| menu.object(l).size.x > 100.0)
        );
        // `Sleep - 4`'s `setAlpha` 0.24 applies on the first update.
        assert_eq!(
            menu.illustration(s.depth_illustrations[1])
                .expect("layer")
                .set_alpha,
            Some(0.24)
        );
    }
}

#[test]
fn depth_starve_scene_layers() {
    let Some(shared) = shared() else { return };
    let mut rig = Rig::open(shared, ProcessId::StarveScreen, SceneId::StarveScreen);
    rig.tick();
    let d = depths(&rig.menu, rig.scene);
    let names: Vec<&str> = d.iter().map(|l| l.0.as_str()).collect();
    assert_eq!(
        names,
        [
            "Starve - 5",
            "Starve - 4",
            "Starve - 3",
            "Starve - 2",
            "Starve - 1"
        ]
    );
    assert_eq!(d[0].1, Vec2::new(543.0, 19.0));
    assert_eq!(d[4].1, Vec2::new(398.0, -86.0));
    assert_eq!(d[2].3, 0.59);
    assert_eq!(d[3].3, 1.0);
}

#[test]
fn depth_death_scene_flowers_follow_the_timer() {
    let Some(shared) = shared() else { return };
    let mut rig = Rig::open(shared, ProcessId::DeathScreen, SceneId::NewDeath);
    let d = depths(&rig.menu, rig.scene);
    assert_eq!(d.len(), 13);
    assert_eq!(d[3].0, "New Death - 4");
    assert_eq!(d[7].0, "FlowerA");
    assert_eq!(d[12].0, "FlowerC2");
    assert_eq!(d[0].1, Vec2::new(716.0, 155.0));
    assert_eq!(d[12].1, Vec2::new(883.0, 250.0));
    rig.tick();
    rig.tick();
    let a = rig.depth_alphas();
    assert!(a[..7].iter().all(|&x| x == 1.0));
    assert!(a[7..].iter().all(|&x| x == 0.0));
    // The timer starts; set from timer t on the update after it, applied on the next.
    rig.menu.start_scene_timer(rig.scene);
    for _ in 0..81 {
        rig.tick();
    }
    let want = SceneId::new_death_flower_alphas(79.0_f32.inverse_lerp(0.0, 160.0));
    assert_eq!(rig.depth_alphas()[7..], want);
    // Near halfway the first flower fades out as the second fades in.
    assert!(want[0] > 0.0 && want[2] > 0.0 && want[4] == 0.0);
}

#[test]
fn depth_sleep_idle_focus_picks_from_its_idle_depths() {
    let Some(mut shared) = shared() else { return };
    shared.saves.current_save_state = Some(SaveState::new(SlugcatName::White, 0));
    let mut rig = Rig::open(shared, ProcessId::SleepScreen, SceneId::SleepScreen);
    if let Some(s) = rig.menu.scene_mut(rig.scene) {
        s.focus = 0.5;
        s.get_to_focus = 0.5;
    }
    // A seed whose first `value` is under 0.02: the pick draws `Range(0, 6)` and `value`.
    rig.shared.random = (0..)
        .map(UnityRandom::new)
        .find(|r| r.clone().value() < 0.02)
        .expect("a seed");
    let mut probe = rig.shared.random.clone();
    rig.tick();
    probe.value();
    let i = probe.range_i32(0, 6) as usize;
    let jitter = probe.value().mathf_pow(1.5) * 0.5;
    let depths = [3.3, 2.7, 1.8, 1.7, 1.6, 1.2];
    let Kind::Scene(s) = &rig.menu.object(rig.scene).kind else {
        panic!("not a scene");
    };
    assert_eq!(s.get_to_focus, (depths[i] + jitter).inverse_lerp(1.0, 10.0));
}
