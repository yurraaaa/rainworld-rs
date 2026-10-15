//! Checked against the decompiled 1.11.8 `Assembly-CSharp` (`HUD.FoodMeter`,
//! `HUD.KarmaMeter`, `HUD.RainMeter`, `HUD.Map.Update`, `HUD.TextPrompt.Update`,
//! `Menu.TutorialControlsPage`); excerpts in `rainworld-re/notes/hud.md`.

use std::path::PathBuf;

use rw_assets::{StreamingAssets, UnityResources};
use rw_menu::{AtlasManager, StageTree, Translator};
use rw_random::UnityRandom;
use rw_sim::{MapDiscovery, Sounds, Vec2};

use crate::{
    CIRCLE_DISTANCE, FoodMeter, Hud, HudSetup, IllustrationSource, InfoId, KarmaMeter, PageText,
    RainCycleView, StartCondition, Subregions, karma_symbol_sprite,
};
use crate::{CustomInt, OwnerTick};

/// A HUD with its sounds and random stream, ticked by hand.
struct Rig {
    hud: Hud,
    sounds: Sounds,
    rng: UnityRandom,
    translator: Translator,
    discovery: MapDiscovery,
}

impl Rig {
    /// A new game's HUD (cycle 0, not in a shelter) for a slugcat's food meter.
    fn new(max_food: i32, food_to_hibernate: i32) -> Self {
        let setup = HudSetup {
            screen_size: [1366.0, 768.0],
            max_food,
            food_to_hibernate,
            current_food: 0,
            karma: 0,
            karma_cap: 4,
            karma_reinforced: false,
            cycle_number: 0,
            camera_room_shelter: false,
            rain: RainCycleView::STAND_IN,
            subregions: None,
            start: StartCondition::New,
            map: Some(crate::map::tests::test_map_data()),
            cycle_label: None,
        };
        Self {
            hud: Hud::new(setup, &mut AtlasManager::new()),
            sounds: Sounds::default(),
            rng: UnityRandom::new(1),
            translator: Translator::default(),
            discovery: MapDiscovery::default(),
        }
    }

    fn tick(&mut self, map: bool) {
        let owner = OwnerTick {
            reveal_map: map,
            room: Some("SU_C04".to_owned()),
            ..OwnerTick::default()
        };
        self.hud.update(
            &owner,
            map,
            &RainCycleView::STAND_IN,
            &self.translator,
            &mut self.sounds,
            &mut self.rng,
            &mut self.discovery,
        );
    }

    /// (karma, food, rain, map) fades.
    fn fades(&self) -> [f32; 4] {
        [
            self.hud.karma_meter.fade,
            self.hud.food_meter.fade,
            self.hud.rain_meter.fade,
            self.hud.map.fade,
        ]
    }
}

/// The installed game, or `None` (test skipped).
fn installed() -> Option<(StreamingAssets, UnityResources)> {
    let game = std::env::var_os("RW_DIR").map_or_else(
        || PathBuf::from("/home/yura/.local/share/Steam/steamapps/common/Rain World"),
        PathBuf::from,
    );
    let assets = StreamingAssets::new(game.join("RainWorld_Data").join("StreamingAssets"));
    let Ok(resources) = UnityResources::open(assets.data_dir()) else {
        eprintln!("skipped: no game under {}", game.display());
        return None;
    };
    Some((assets, resources))
}

#[test]
fn food_meter_pips_and_hibernation_line_per_slugcat() {
    // `XAdd`: CircleDistance * number + (number >= ShowSurvivalLimit ? CircleDistance / 2 : 0);
    // `CircleDistance` 30 for a player's meter. `SlugcatFoodMeter`: White (7, 4),
    // Yellow (5, 3), Red (9, 6).
    for (max, hib, xs) in [
        (7, 4, vec![0.0, 30.0, 60.0, 90.0, 135.0, 165.0, 195.0]),
        (5, 3, vec![0.0, 30.0, 60.0, 105.0, 135.0]),
        (
            9,
            6,
            vec![0.0, 30.0, 60.0, 90.0, 120.0, 150.0, 195.0, 225.0, 255.0],
        ),
    ] {
        let mut stage = StageTree::new();
        let c = stage.add_container(StageTree::ROOT);
        let meter = FoodMeter::new(&mut stage, c, max, hib, 0);
        assert_eq!(CIRCLE_DISTANCE, 30.0);
        assert_eq!(meter.pos, Vec2::new(50.0, 25.0));
        let got: Vec<f32> = meter.circles.iter().map(|c| c.slow_x_add).collect();
        assert_eq!(got, xs, "slugcat ({max}, {hib})");
        // `lineSprite.x = DrawPos.x + CircleDistance * (Lerp(lastShowSurvLim, showSurvLim,
        // timeCounter) - 0.25)`: half way into the gap before pip `foodToHibernate`.
        meter.draw(&mut stage, 0, 1.0);
        let line = stage.sprite(meter.line_sprite);
        assert_eq!(line.transform.x, 50.0 + 30.0 * (hib as f32 - 0.25));
        assert_eq!(line.transform.y, 25.0);
        assert_eq!(
            (line.transform.scale_x, line.transform.scale_y),
            (2.0, 34.5)
        );
    }
}

#[test]
fn karma_symbols() {
    // `KarmaSymbolSprite(small, k)`: k.x < 5 -> "smallKarma" + Clamp(k.x, 0, 4), else
    // "smallKarma" + Clamp(k.x, 5, 9) + "-" + Clamp(k.y, k.x, 9).
    for (karma, cap, name) in [
        (0, 4, "smallKarma0"),
        (2, 4, "smallKarma2"),
        (4, 4, "smallKarma4"),
        (-1, 4, "smallKarma0"),
        (5, 5, "smallKarma5-5"),
        (5, 7, "smallKarma5-7"),
        (6, 5, "smallKarma6-6"),
        (9, 9, "smallKarma9-9"),
        (10, 12, "smallKarma9-9"),
        (10, 8, "smallKarma9-10"),
    ] {
        assert_eq!(
            karma_symbol_sprite(true, karma, cap),
            name,
            "({karma}, {cap})"
        );
    }
    assert_eq!(karma_symbol_sprite(false, 7, 9), "karma7-9");
    assert_eq!(12.int_clamp(10, 9), 9);
    assert_eq!(8.int_clamp(10, 9), 10);
    // The constructor clamps the karma it shows to 0..cap (`Custom.IntClamp`): new
    // games start at karma 0, cap 4 (Hunter at karma 2).
    let mut stage = StageTree::new();
    let c = stage.add_container(StageTree::ROOT);
    assert_eq!(
        KarmaMeter::new(&mut stage, c, 0, 4, false).symbol,
        "smallKarma0"
    );
    assert_eq!(
        KarmaMeter::new(&mut stage, c, 2, 4, false).symbol,
        "smallKarma2"
    );
    assert_eq!(
        KarmaMeter::new(&mut stage, c, 6, 4, false).symbol,
        "smallKarma4"
    );
}

#[test]
fn karma_meter_rests_in_the_corner_and_shrinks_hidden() {
    let mut rig = Rig::new(7, 4);
    // `pos = (Max(55.01, SafeScreenOffset.x + 22.51), Max(45.01, ...))`, rad 22.5.
    assert_eq!(rig.hud.karma_meter.pos, Vec2::new(55.01, 45.01));
    assert_eq!(rig.hud.karma_meter.rad, 22.5);
    // Hidden: `rad = LerpAndTick(rad, LerpMap(fade, 0, 0.15, 17, 22.5, 1.3), 0.12, 0.1)`.
    rig.tick(false);
    let rad = rig.hud.karma_meter.rad;
    assert!(
        (rad - (22.5 + (17.0 - 22.5) * 0.12 - 0.1)).abs() < 1e-4,
        "{rad}"
    );
    for _ in 0..200 {
        rig.tick(false);
    }
    assert_eq!(rig.hud.karma_meter.rad, 17.0);
    assert_eq!(rig.hud.karma_meter.pos, KarmaMeter::REST);
    assert_eq!(rig.fades(), [0.0; 4]);
}

#[test]
fn meters_fade_in_on_the_map_button_and_out_after() {
    let mut rig = Rig::new(7, 4);
    let mut trace = Vec::new();
    for _ in 0..60 {
        rig.tick(true);
        trace.push(rig.fades());
    }
    // Tick 1: `showKarmaFoodRain` is still false; the karma meter's `Show` target is
    // Max(0, 0.25 + 0.75 * map.fade) = 0.25, +0.1. The food meter (RevealMap) +0.1. The
    // rain meter reads the karma meter's fade from before its update: 0.
    assert_eq!(trace[0], [0.1, 0.1, 0.0, 0.0]);
    // Then the karma and food meters +0.1 a tick to 1 (tick 10), the rain meter +1/30
    // a tick from tick 2.
    for (i, f) in trace.iter().enumerate().take(10) {
        let expected = (0.1 * (i + 1) as f32).min(1.0);
        assert!((f[0] - expected).abs() < 1e-5, "tick {}: {f:?}", i + 1);
        assert!((f[1] - expected).abs() < 1e-5, "tick {}: {f:?}", i + 1);
        assert!(
            (f[2] - i as f32 / 30.0).abs() < 1e-5,
            "tick {}: {f:?}",
            i + 1
        );
    }
    assert_eq!(trace[9][..2], [1.0, 1.0]);
    let rain_full = trace.iter().position(|f| f[2] == 1.0).unwrap() + 1;
    assert!((31..=32).contains(&rain_full), "rain full at {rain_full}");
    // `Map.Update`: fadeCounter++; only past 30 does the fade rise:
    // fade = Min(fade + 1/30, 1); fade = Lerp(fade, 1, 0.1).
    assert!(trace[..30].iter().all(|f| f[3] == 0.0));
    let first = (1.0_f32 / 30.0) + (1.0 - 1.0 / 30.0) * 0.1;
    assert!((trace[30][3] - first).abs() < 1e-6, "{:?}", trace[30]);
    assert_eq!(rig.hud.map.fade_counter, 60);

    // Released: on the first tick `showKarmaFoodRain` is still set from the last one.
    let mut out = Vec::new();
    for _ in 0..100 {
        rig.tick(false);
        out.push(rig.fades());
    }
    assert_eq!(out[0][..2], [1.0, 1.0]);
    // Map: fade = Max(fade - 0.05, 0); fade = Lerp(fade, 0, 0.1).
    let map = trace[59][3];
    assert!((out[0][3] - (map - 0.05) * 0.9).abs() < 1e-6);
    assert_eq!(rig.hud.map.fade_counter, 0);
    // Food: Max(0, fade - 0.0125) a tick, 80 ticks to 0. Karma: -0.0125, then
    // Min(fade + 0.1, foodMeter.fade) while both show: it follows the food meter's
    // fade of the tick before.
    for i in 1..out.len() {
        let food = (1.0 - 0.0125 * i as f32).max(0.0);
        assert!((out[i][1] - food).abs() < 1e-4, "tick {i}: {:?}", out[i]);
        if out[i - 1][1] > 0.0 && out[i][0] > 0.0 {
            assert_eq!(out[i][0], out[i - 1][1], "tick {i}");
        }
    }
    let food_gone = out.iter().position(|f| f[1] == 0.0).unwrap();
    assert!(
        (79..=81).contains(&food_gone),
        "food hidden after {food_gone}"
    );
    // Rain: Max(0, fade - 0.1) once not shown.
    assert!((out[1][2] - 0.9).abs() < 1e-5);
    assert_eq!(out[10][2], 0.0);
    assert_eq!(out.last().unwrap(), &[0.0; 4]);
}

#[test]
fn meters_show_for_200_ticks_after_cycle_0() {
    // `FireUpSinglePlayerHUD`: cycleNumber > 0 sets the food, karma and rain meters'
    // visible counters to 200.
    let setup = HudSetup {
        screen_size: [1366.0, 768.0],
        max_food: 7,
        food_to_hibernate: 4,
        current_food: 0,
        karma: 0,
        karma_cap: 4,
        karma_reinforced: false,
        cycle_number: 1,
        camera_room_shelter: false,
        rain: RainCycleView::STAND_IN,
        subregions: None,
        start: StartCondition::Load,
        map: None,
        cycle_label: None,
    };
    let mut rig = Rig::new(7, 4);
    rig.hud = Hud::new(setup, &mut AtlasManager::new());
    assert_eq!(rig.hud.food_meter.visible_counter, 200);
    assert_eq!(rig.hud.karma_meter.force_visible_counter, 200);
    assert_eq!(rig.hud.rain_meter.remain_visible_counter, 200);
    for _ in 0..10 {
        rig.tick(false);
    }
    assert_eq!(rig.hud.food_meter.fade, 1.0);
    for _ in 0..190 {
        rig.tick(false);
    }
    assert_eq!(rig.hud.food_meter.visible_counter, 0);
    assert_eq!(rig.hud.food_meter.fade, 1.0);
    rig.tick(false);
    assert_eq!(rig.hud.food_meter.fade, 1.0 - 0.0125);
}

#[test]
fn rain_meter_has_a_circle_per_1200_ticks() {
    // `cycleLength / 1200` circles (`timePerCircle`), at most 30.
    let rig = Rig::new(7, 4);
    assert_eq!(rig.hud.rain_meter.circles.len(), 32_000 / 1200);
    assert_eq!(rig.hud.rain_meter.time_per_circle, 1200);
}

#[test]
fn text_prompt_timing() {
    let mut rig = Rig::new(7, 4);
    rig.hud.add_message("Hello", 10, 40, true, true);
    let mut showing = Vec::new();
    for _ in 0..200 {
        rig.tick(false);
        let p = &rig.hud.text_prompt;
        showing.push((p.show, p.currently_showing, p.messages.len()));
    }
    // `infoID` is Nothing while `wait > 0` (wait-- a tick): ticks 1..=10.
    assert!(
        showing[..10]
            .iter()
            .all(|s| s.1 == InfoId::Nothing && s.0 == 0.0)
    );
    // Tick 11: infoID Message != currentlyShowing; show stays 0 and, show and lastShow
    // being 0, currentlyShowing becomes Message.
    assert_eq!(showing[10], (0.0, InfoId::Message, 1));
    // Then show = Min(1, show + 1/30) a tick.
    assert!((showing[11].0 - 1.0 / 30.0).abs() < 1e-6);
    let full = showing.iter().position(|s| s.0 == 1.0).unwrap();
    assert!((40..=41).contains(&full), "full at tick {}", full + 1);
    // While show == 1, time-- a tick (40 ticks), then fadeOut and infoID Nothing:
    // show - 1/30 a tick to 0, and the message is removed on the tick after.
    let leaving = showing.iter().rposition(|s| s.0 == 1.0).unwrap();
    assert!(
        (leaving - full) >= 39 && (leaving - full) <= 41,
        "{full} {leaving}"
    );
    let gone = showing.iter().position(|s| s.2 == 0).unwrap();
    assert!(
        (gone - leaving) >= 30 && (gone - leaving) <= 32,
        "{leaving} {gone}"
    );
    assert_eq!(showing[gone].0, 0.0);
    // `HideGeneralHud` (hideHud && show > 0) zeroes the meters' fades while it shows.
    let mut rig = Rig::new(7, 4);
    rig.hud.add_message("Hidden", 0, 100, false, true);
    for _ in 0..20 {
        rig.tick(true);
    }
    assert!(rig.hud.hide_general_hud());
    assert_eq!(rig.fades(), [0.0; 4]);
}

#[test]
fn text_prompt_map_button_shortens_the_message() {
    // `if (RevealMap) time = IntClamp(time - 4, 0, 20)` while showing.
    let mut rig = Rig::new(7, 4);
    rig.hud.add_message("Hello", 0, 400, false, false);
    while rig.hud.text_prompt.show < 1.0 {
        rig.tick(false);
    }
    rig.tick(true);
    assert_eq!(rig.hud.text_prompt.messages[0].time, 20);
    for _ in 0..4 {
        rig.tick(true);
    }
    assert_eq!(rig.hud.text_prompt.messages[0].time, 0);
    assert!(rig.hud.text_prompt.messages[0].fade_out);
}

#[test]
fn controls_page_fades_in_and_out() {
    let mut rig = Rig::new(7, 4);
    let assets = StreamingAssets::new("/nonexistent");
    let mut atlases = AtlasManager::new();
    let translator = Translator::default();
    rig.hud.open_controls_page(
        &mut IllustrationSource {
            assets: &assets,
            atlases: &mut atlases,
        },
        &PageText {
            font: None,
            translator: &translator,
        },
    );
    // `blackFade = Min(1, blackFade + 0.0625)` a tick: 16 ticks.
    for i in 1..=16 {
        rig.hud.update_controls_page(false);
        let page = rig.hud.controls_page.as_ref().unwrap();
        assert_eq!(page.counter, i);
        assert_eq!(page.black_fade, 0.0625 * i as f32);
        // `ControlMap`: fade = blackFade * (1 - map.fade).
        assert_eq!(page.control_map.fade, page.black_fade);
    }
    rig.hud.update_controls_page(false);
    assert_eq!(rig.hud.controls_page.as_ref().unwrap().black_fade, 1.0);
    // `wantToContinue`: Max(0, blackFade - 0.125) a tick, then `ShutDownProcess`.
    for i in 1..8 {
        rig.hud.update_controls_page(true);
        let page = rig.hud.controls_page.as_ref().unwrap();
        assert_eq!(page.black_fade, 1.0 - 0.125 * i as f32);
    }
    rig.hud.update_controls_page(false);
    assert!(rig.hud.controls_page.is_none());
}

#[test]
fn controls_page_layout() {
    // `ControlMap` at ((int)(1366 / 3) + 170.2 + (int)((1366 - 1366) / 2), 450) on a page
    // at (1/3, 1/3).
    let mut rig = Rig::new(7, 4);
    let assets = StreamingAssets::new("/nonexistent");
    let mut atlases = AtlasManager::new();
    let translator = Translator::default();
    rig.hud.open_controls_page(
        &mut IllustrationSource {
            assets: &assets,
            atlases: &mut atlases,
        },
        &PageText {
            font: None,
            translator: &translator,
        },
    );
    let page = rig.hud.controls_page.as_ref().unwrap();
    let pos = page.control_map.pos;
    assert!(
        (pos.x - (455.0 + 170.2 + 1.0 / 3.0)).abs() < 1e-3,
        "{pos:?}"
    );
    assert!((pos.y - (450.0 + 1.0 / 3.0)).abs() < 1e-3, "{pos:?}");
}

#[test]
fn controls_page_labels_installed() {
    let Some((assets, resources)) = installed() else {
        return;
    };
    let mut atlases = AtlasManager::new();
    atlases.load_english_fonts(&assets, &resources).unwrap();
    let translator = Translator::load(&assets).unwrap();
    let mut rig = Rig::new(7, 4);
    let font = atlases.font(rw_menu::Font::FONT).cloned();
    rig.hud.open_controls_page(
        &mut IllustrationSource {
            assets: &assets,
            atlases: &mut atlases,
        },
        &PageText {
            font: font.as_ref(),
            translator: &translator,
        },
    );
    // `ControlMap` for `KeyboardSinglePlayer` with the default bindings.
    assert_eq!(
        rig.hud.controls_page_texts().unwrap(),
        [
            "Arrows - Move",
            "Hold Space - Map",
            "Esc - Pause",
            "Shift - Pick up / Eat",
            "Z - Jump",
            "X - Throw",
            "",
        ]
    );
    // The keyboard pictures load from `Illustrations`.
    for name in ["keyboard", "keyboard2", "keyboard3"] {
        assert!(atlases.element(name).is_some(), "{name}");
    }
}

#[test]
fn hud_atlas_elements_installed() {
    let Some((assets, resources)) = installed() else {
        return;
    };
    let mut atlases = AtlasManager::new();
    for atlas in ["Atlases/rainWorld", "Atlases/uiSprites"] {
        atlases.load_atlas(&assets, &resources, atlas).unwrap();
    }
    let mut names: Vec<String> = [
        "FoodCircleA",
        "FoodCircleB",
        "deerEyeB",
        "smallEmptyCircle",
        "karmaRing",
        "Circle4",
        "QuarterPips1",
        "QuarterPips2",
        "QuarterPips3",
        "smallKarmaRingReinforced",
        "pixel",
        AtlasManager::WHITE,
        // The map's markers.
        "ShelterMarker",
        "smallKarmaNoRing-1",
        "smallKarmaNoRing0",
        "smallKarmaNoRing4",
    ]
    .map(str::to_owned)
    .to_vec();
    for karma in 0..=9 {
        for cap in 4..=9 {
            names.push(karma_symbol_sprite(true, karma.int_clamp(0, cap), cap));
        }
    }
    for name in &names {
        assert!(atlases.element(name).is_some(), "missing {name}");
    }
}

#[test]
fn subregions_of_outskirts_installed() {
    let Some((assets, _)) = installed() else {
        return;
    };
    let su = Subregions::load(&assets, "SU", "White");
    // `properties.txt`: "Subregion: Outskirts"; index 0 is the unnamed one.
    assert_eq!(su.names, ["", "Outskirts"]);
    assert_eq!(su.index_of("SU_B13"), 1);
    assert_eq!(su.index_of("su_a33"), 1);
    // SU_C04's map line leaves the field empty.
    assert_eq!(su.index_of("SU_C04"), 0);
    assert_eq!(su.index_of("NOT_A_ROOM"), 0);
}
