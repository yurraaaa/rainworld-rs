//! `SlugcatSelectMenu` (553691) with a saved game: `SlugcatPageContinue` (554289) with
//! the installed game's fonts, strings, region names and scene images (skipped without
//! them), from a save state put in memory as the current one (`MineForSaveData`'s first
//! branch, 555228).

use std::path::PathBuf;

use rw_assets::{SlugcatName, StreamingAssets, UnityResources};
use rw_hud::{SlugcatSelectMenu, SlugcatSelectScreen};
use rw_menu::menu::{Kind, SceneId};
use rw_menu::{MainLoopProcess, MenuColor, MenuContext, MenuShared, ProcessId, ProcessRequests};
use rw_random::UnityRandom;
use rw_save::SaveState;
use rw_sim::Vec2;

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Ctx {
    shared: MenuShared,
}

impl MenuContext for Ctx {
    fn menu(&mut self) -> &mut MenuShared {
        &mut self.shared
    }
}

struct Harness {
    ctx: Ctx,
    select: SlugcatSelectMenu,
}

impl Harness {
    /// The menu with `save` as the current save state.
    fn open(save: SaveState) -> Option<Self> {
        Self::open_seeded(save, 0)
    }

    /// `open` with the random stream seeded `seed` before the constructor.
    fn open_seeded(save: SaveState, seed: i32) -> Option<Self> {
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
        shared.pre_switch(ProcessId::SlugcatSelect);
        shared.progression.red_unlocked = true;
        shared.saves.current_save_state = Some(save);
        shared.random = UnityRandom::new(seed);
        let select = SlugcatSelectMenu::new(&mut shared);
        Some(Self {
            ctx: Ctx { shared },
            select,
        })
    }

    fn tick(&mut self) {
        let mut requests = ProcessRequests::new(None, false, 0.0);
        self.select.update(&mut self.ctx, &mut requests);
        self.select.graf_update(1.0, &mut self.ctx);
    }

    fn screen(&self) -> &SlugcatSelectScreen {
        self.select.screen()
    }

    /// The region label's text, x, alpha and colour.
    fn region_label(&self, page: usize) -> (String, f32, f32, rw_menu::StageColor) {
        let c = self.screen().pages[page]
            .continue_page()
            .expect("a continue page");
        let menu = self.select.menu();
        let Kind::Label(l) = &menu.object(c.region_label).kind else {
            panic!("not a label");
        };
        let label = menu.stage.label(l.label);
        (
            label.text().to_owned(),
            label.transform.x,
            label.transform.alpha,
            label.color,
        )
    }

    fn scene(&self, page: usize) -> SceneId {
        let menu = self.select.menu();
        match &menu.object(self.screen().pages[page].scene).kind {
            Kind::Scene(s) => s.id,
            _ => panic!("not a scene"),
        }
    }
}

fn save(slugcat: SlugcatName, den: &str, cycle: i32, food: i32, karma: (i32, i32)) -> SaveState {
    let mut s = SaveState::new(slugcat, 0);
    s.den_position = den.to_owned();
    s.cycle_number = cycle;
    s.food = food;
    s.death_persistent.karma = karma.0;
    s.death_persistent.karma_cap = karma.1;
    s
}

/// The Monk's save: the only one, so its page is the one shown (`num == 1`); karma and
/// food clamped to the cap and the meter (5 pips, 3 to hibernate).
#[test]
fn the_monks_continue_page() {
    let Some(mut h) = Harness::open(save(SlugcatName::Yellow, "SU_S01", 5, 9, (7, 4))) else {
        return;
    };
    let s = h.screen();
    assert_eq!(s.slugcat_page_index, 0);
    assert!(s.pages[1].new_game_labels().is_some());
    let d = s.save_game_data[0].as_ref().expect("the Monk's data");
    assert_eq!((d.food, d.karma, d.karma_cap), (5, 4, 4));
    let c = s.pages[0].continue_page().expect("a continue page");
    assert_eq!(c.hud.karma_meter.symbol, "smallKarma4");
    assert_eq!(c.hud.food_meter.circles.len(), 5);
    assert_eq!(c.hud.food_meter.show_count, 5);
    // At (-1000, imagePos.y - 249), 200 x 30.
    let menu = h.select.menu();
    assert_eq!(menu.object(c.region_label).pos, Vec2::new(-1000.0, 235.0));
    assert_eq!(menu.object(c.region_label).size, Vec2::new(200.0, 30.0));
    assert_eq!(h.scene(0), SceneId::SlugcatYellow);
    h.tick();
    let (text, x, alpha, color) = h.region_label(0);
    assert_eq!(text, "Outskirts - Cycle 5");
    assert_eq!((x, alpha), (683.01, 1.0));
    assert_eq!(color, MenuColor::MediumGrey.rgb());
    // `KarmaSymbolPos`: centred under the label with the food meter, 290 below the
    // image; the meters are faded in on the centred page.
    let c = h.screen().pages[0].continue_page().unwrap();
    let k = &c.hud.karma_meter;
    let width = c.hud.food_meter.total_width(1.0);
    let radius = c.hud.karma_radius();
    assert_eq!(k.pos.y, 194.01);
    assert!((k.pos.x - (683.0 - width * 0.5 - radius * 0.5 + 0.01)).abs() < 1e-3);
    assert!((c.hud.food_meter.pos.x - (k.pos.x + radius + 22.0)).abs() < 1e-3);
    assert_eq!((k.fade, c.hud.food_meter.fade), (1.0, 1.0));
    // The neighbouring pages' meters would be hidden: one page off is past half.
    assert_eq!(h.screen().page_next_scroll(1), 1.0);
}

/// The Hunter's label counts the cycles left: `RedsCycles(false) - cycle` = 19 - 7.
#[test]
fn the_hunters_cycles_left() {
    let Some(mut h) = Harness::open(save(SlugcatName::Red, "LF_S02", 7, 2, (1, 4))) else {
        return;
    };
    assert_eq!(h.screen().slugcat_page_index, 2);
    assert!(!h.screen().red_is_dead);
    assert_eq!(h.scene(2), SceneId::SlugcatRed);
    h.tick();
    assert_eq!(h.region_label(2).0, "Farm Arrays - Cycle 12");
}

/// `redIsDead` after the last cycle's death: the dead Hunter's scene, STATISTICS.
#[test]
fn the_dead_hunter() {
    let mut s = save(SlugcatName::Red, "LF_S02", 19, 0, (1, 4));
    s.death_persistent.reds_death = true;
    let Some(mut h) = Harness::open(s) else {
        return;
    };
    assert!(h.screen().red_is_dead);
    assert_eq!(h.scene(2), SceneId::SlugcatDeadRed);
    h.tick();
    assert_eq!(h.region_label(2).0, "Farm Arrays - Cycle 0");
}

/// An ascended Survivor shows its ghost scene.
#[test]
fn the_ascended_survivor() {
    let mut s = save(SlugcatName::White, "SU_S01", 30, 0, (9, 9));
    s.death_persistent.ascended = true;
    let Some(h) = Harness::open(s) else {
        return;
    };
    assert_eq!(h.scene(1), SceneId::GhostWhite);
}

/// Depth mode: `new RainEffect` first (2508 draws), then each page's
/// `InteractiveMenuScene` (`getToFocus`, `RNV`, `value`); `BuildScene`, the mark, the
/// glow and the HUD draw nothing.
#[test]
fn the_constructor_draws_the_rain_then_the_scenes() {
    let mut s = save(SlugcatName::White, "SU_S01", 3, 2, (1, 4));
    s.the_glow = true;
    s.death_persistent.the_mark = true;
    let Some(h) = Harness::open_seeded(s, 11) else {
        return;
    };
    let mut probe = UnityRandom::new(11);
    for _ in 0..2508 + 3 * 3 {
        probe.value();
    }
    assert_eq!(h.ctx.shared.random, probe);
    let screen = h.screen();
    assert!(screen.rain_effect.is_some());
    // The Survivor's continue page has both; the others are new-game pages.
    let lights = &screen.pages[1].lights;
    assert!(lights.has_mark && lights.has_glow);
    assert!(lights.mark.is_some() && lights.glow.is_some());
    assert!(screen.pages[0].lights.glow.is_none());
}

/// `SlugcatPage.GrafUpdate` places each layer at `pos - CamPos * 80 / depth +
/// sceneOffset`, slid by the scroll, at its alpha times `UseAlpha`; `positions.txt`
/// gives `pos` (the Monk's first layer at (432, 173)).
#[test]
fn depth_layers_follow_the_page() {
    let Some(mut h) = Harness::open(save(SlugcatName::Yellow, "SU_S01", 5, 2, (1, 4))) else {
        return;
    };
    h.tick();
    let menu = h.select.menu();
    let page = &h.screen().pages[0];
    let Kind::Scene(scene) = &menu.object(page.scene).kind else {
        panic!("not a scene");
    };
    assert_eq!(scene.depth_illustrations.len(), 6);
    assert_eq!(scene.idle_depths, [3.6, 2.8, 2.6, 2.5, 1.5]);
    // The centred page: `cameraRange` 0.5.
    assert_eq!(scene.camera_range, 0.5);
    let first = scene.depth_illustrations[0];
    assert_eq!(menu.object(first).pos, Vec2::new(432.0, 173.0));
    let Kind::Illustration(il) = &menu.object(first).kind else {
        panic!("not an illustration");
    };
    let expected =
        Vec2::new(432.0, 173.0) - scene.cam_pos_at(1.0) * 80.0 / 3.6 + Vec2::new(10.0, 75.0);
    let tr = menu.stage.sprite(il.sprite).transform;
    assert!((tr.x - expected.x).abs() < 1e-3 && (tr.y - expected.y).abs() < 1e-3);
    assert_eq!(tr.alpha, il.alpha);
    // The rain fades in by 0.006 a tick.
    let rain = menu.rain_effect(h.screen().rain_effect.unwrap()).unwrap();
    assert!((rain.rain_fade - 0.006).abs() < 1e-7);
}
