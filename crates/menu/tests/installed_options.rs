//! The options menu with the installed game's fonts and illustrations (read-only),
//! reached from the main menu. Set `RW_DIR` to override the default install; tests skip
//! when it is absent. Expected numbers come from `Menu.OptionsMenu` (548249-549535),
//! `HorizontalSlider.Update` (553447) and `SelectOneButton` (540370) of 1.11.8.

use std::path::PathBuf;

use rw_assets::{StreamingAssets, UnityResources};
use rw_menu::menu::{Kind, ObjectId, SceneId, SliderId};
use rw_menu::{
    Initialization, KeyboardKey, MainLoopProcess, MainMenu, MenuContext, MenuShared, OptionsMenu,
    ProcessFactory, ProcessId, ProcessManager, ProcessRequests,
};
use rw_random::UnityRandom;
use rw_save::SaveSlot;
use rw_sim::{FloatExt, Vec2};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

enum Proc {
    Init(Initialization),
    Main(Box<MainMenu>),
    Options(Box<OptionsMenu>),
    Other(ProcessId),
}

struct Ctx {
    shared: MenuShared,
    saved_options: u32,
    loaded_slots: Vec<i32>,
    imports: Vec<SaveSlot>,
}

impl MenuContext for Ctx {
    fn menu(&mut self) -> &mut MenuShared {
        &mut self.shared
    }

    fn load_save_slot(&mut self) {
        self.loaded_slots.push(self.shared.options.save_slot);
    }

    fn save_options(&mut self) {
        self.saved_options += 1;
    }

    fn import_save_slot(&mut self, slot: SaveSlot) -> Result<(), String> {
        self.imports.push(slot);
        Ok(())
    }
}

impl ProcessFactory<Proc> for Ctx {
    fn create_process(&mut self, id: ProcessId, _old: Option<&Proc>) -> Proc {
        match id {
            ProcessId::Initialization => Proc::Init(Initialization::new(ProcessId::MainMenu)),
            ProcessId::MainMenu => Proc::Main(Box::new(MainMenu::new(&mut self.shared))),
            ProcessId::OptionsMenu => Proc::Options(Box::new(OptionsMenu::new(&mut self.shared))),
            other => Proc::Other(other),
        }
    }

    fn pre_switch(&mut self, next: ProcessId) {
        self.shared.pre_switch(next);
    }
}

impl MainLoopProcess<Ctx> for Proc {
    fn id(&self) -> ProcessId {
        match self {
            Self::Init(_) => ProcessId::Initialization,
            Self::Main(_) => ProcessId::MainMenu,
            Self::Options(_) => ProcessId::OptionsMenu,
            Self::Other(id) => *id,
        }
    }

    fn update(&mut self, ctx: &mut Ctx, requests: &mut ProcessRequests) {
        match self {
            Self::Init(p) => p.update(ctx, requests),
            Self::Main(p) => p.update(ctx, requests),
            Self::Options(p) => p.update(ctx, requests),
            Self::Other(_) => {}
        }
    }

    fn graf_update(&mut self, t: f32, ctx: &mut Ctx) {
        match self {
            Self::Main(p) => p.graf_update(t, ctx),
            Self::Options(p) => p.graf_update(t, ctx),
            _ => {}
        }
    }

    fn shut_down(&mut self, ctx: &mut Ctx) {
        match self {
            Self::Main(p) => p.shut_down(ctx),
            Self::Options(p) => p.shut_down(ctx),
            _ => {}
        }
    }
}

struct Harness {
    manager: ProcessManager<Proc>,
    ctx: Ctx,
}

impl Harness {
    /// Boots into the main menu, then OPTIONS (the fifth button: down four times).
    fn options() -> Option<Self> {
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
        shared.random = UnityRandom::new(1);
        let mut ctx = Ctx {
            shared,
            saved_options: 0,
            loaded_slots: Vec::new(),
            imports: Vec::new(),
        };
        let manager = ProcessManager::new([1366.0, 768.0], &mut ctx);
        let mut h = Self { manager, ctx };
        h.settle();
        assert_eq!(h.id(), Some(ProcessId::MainMenu));
        for _ in 0..4 {
            h.tap(KeyboardKey::DownArrow);
        }
        h.tap(KeyboardKey::Return);
        h.settle();
        assert_eq!(h.id(), Some(ProcessId::OptionsMenu));
        Some(h)
    }

    fn id(&self) -> Option<ProcessId> {
        self.manager.current().map(MainLoopProcess::id)
    }

    fn frame(&mut self, keys: &[KeyboardKey]) {
        for &k in keys {
            self.ctx.shared.input.set_key(k, true);
        }
        self.manager.update(0.025, &mut self.ctx);
        for &k in keys {
            self.ctx.shared.input.set_key(k, false);
        }
    }

    fn settle(&mut self) {
        for _ in 0..200 {
            if self.manager.upcoming().is_none()
                && self.manager.current().is_some()
                && self.manager.fade_to_black() == 0.0
            {
                return;
            }
            self.frame(&[]);
        }
        panic!("the switch never finished");
    }

    fn tap(&mut self, key: KeyboardKey) {
        self.frame(&[key]);
        self.frame(&[]);
        self.frame(&[]);
    }

    fn options_menu(&self) -> &OptionsMenu {
        match self.manager.current() {
            Some(Proc::Options(p)) => p,
            _ => panic!("not the options menu"),
        }
    }

    /// The page's objects that are buttons, with their signal texts.
    fn button(&self, signal: &str, nth: usize) -> ObjectId {
        let menu = self.options_menu().menu();
        let page = menu.pages[0];
        menu.object(page)
            .sub_objects
            .iter()
            .copied()
            .filter(|&o| menu.button(o).is_some_and(|b| b.signal_text == signal))
            .nth(nth)
            .unwrap_or_else(|| panic!("no button {signal} {nth}"))
    }

    fn pos(&self, o: ObjectId) -> Vec2 {
        self.options_menu().menu().object(o).pos
    }

    fn size(&self, o: ObjectId) -> Vec2 {
        self.options_menu().menu().object(o).size
    }

    fn slider(&self, id: SliderId) -> ObjectId {
        let menu = self.options_menu().menu();
        let page = menu.pages[0];
        menu.object(page)
            .sub_objects
            .iter()
            .copied()
            .find(|&o| menu.slider(o).is_some_and(|s| s.id == id))
            .expect("slider")
    }

    fn select(&mut self, id: ObjectId) {
        match self.manager.current_mut() {
            Some(Proc::Options(p)) => p.menu_mut().selected = Some(id),
            _ => panic!("not the options menu"),
        }
    }
}

/// The constructor's layout with Remix off, English (`GetSaveSlotButtonWidth` 100):
/// the screen column at x 200, the reset button at (503 + 221, 270 + 100 + 207), the
/// slots 200 left of it from y 602 down, the languages from (891 + 55, 620) in two
/// columns 110 apart, the sliders at x 192, y 235/195/155.
#[test]
fn layout() {
    let Some(h) = Harness::options() else { return };
    let menu = h.options_menu().menu();
    let scene = menu.scene.expect("scene");
    assert!(matches!(&menu.object(scene).kind, Kind::Scene(s) if s.id == SceneId::LandscapeSu));
    let fs = h.button("WINDOWED", 0);
    assert_eq!(
        (h.pos(fs), h.size(fs)),
        (Vec2::new(200.0, 620.0), Vec2::new(150.0, 30.0))
    );
    let resolutions = [
        "1024 x 768  [4:3]",
        "1366 x 768  [16:9]",
        "1360 x 768  [16:9]",
        "1280 x 768  [5:3]",
        "1280 x 800  [16:10]",
    ];
    for (i, text) in resolutions.iter().enumerate() {
        assert_eq!(OptionsMenu::resolution_string(i), *text);
        let r = h.button("ScreenRes", i);
        assert_eq!(
            h.pos(r),
            Vec2::new(200.0, 620.0 - 40.0 * (i + 1) as f32),
            "res {i}"
        );
    }
    let wipe = menu
        .object(menu.pages[0])
        .sub_objects
        .iter()
        .copied()
        .find(|&o| menu.hold_button(o).is_some())
        .expect("reset button");
    assert_eq!(h.pos(wipe), Vec2::new(724.0, 577.0));
    assert_eq!(menu.hold_button(wipe).unwrap().fill_time, 400.0);
    for k in 0..3 {
        let s = h.button("SaveSlot", k);
        assert_eq!(h.pos(s), Vec2::new(524.0, 602.0 - 40.0 * k as f32));
        assert_eq!(h.size(s), Vec2::new(100.0, 30.0));
    }
    assert_eq!(h.pos(h.button("BACKUPS", 0)), Vec2::new(524.0, 482.0));
    for j in 0..10 {
        let l = h.button("Language", j);
        let x = if j % 2 == 0 { 946.0 } else { 1056.0 };
        assert_eq!(
            h.pos(l),
            Vec2::new(x, 620.0 - 40.0 * (j / 2) as f32),
            "language {j}"
        );
    }
    assert_eq!(h.pos(h.button("BACK", 0)), Vec2::new(200.0, 50.0));
    assert_eq!(h.pos(h.button("CREDITS", 0)), Vec2::new(1056.0, 50.0));
    let input = h.button("INPUT", 0);
    assert_eq!(
        (h.pos(input), h.size(input)),
        (Vec2::new(1056.0, 169.0), Vec2::new(110.0, 80.0))
    );
    assert_eq!(h.pos(h.button("VIBRATION", 0)), Vec2::new(532.0, 295.0));
    let validation = h.button("VALIDATION", 0);
    assert_eq!(h.pos(validation).y, 259.0);
    assert_eq!(
        menu.slider(h.slider(SliderId::MusicVol))
            .unwrap()
            .anchor_point
            .y,
        235.01
    );
    assert_eq!(
        menu.slider(h.slider(SliderId::ArenaMusicVolume))
            .unwrap()
            .anchor_point
            .y,
        195.01
    );
    assert_eq!(
        menu.slider(h.slider(SliderId::SfxVol))
            .unwrap()
            .anchor_point
            .y,
        155.01
    );
    // BACK is the back object and selected first; the port's import sits between it
    // and CREDITS.
    let back = h.button("BACK", 0);
    assert_eq!(menu.selected, Some(back));
    let import = h.options_menu().import_button();
    assert_eq!(menu.object(back).next_selectable[2], Some(import));
    assert_eq!(
        menu.object(back).next_selectable[1],
        Some(h.slider(SliderId::SfxVol))
    );
    // `nextSelectable` of the slots: up the previous, down the next then BACKUPS,
    // right the reset button, left the screen-mode button.
    let s0 = h.button("SaveSlot", 0);
    assert_eq!(
        menu.object(s0).next_selectable,
        [
            Some(fs),
            Some(s0),
            Some(wipe),
            Some(h.button("SaveSlot", 1))
        ]
    );
}

/// `HorizontalSlider.Update` with the keyboard: `movSpeed` LerpAndTicks to 1 by 0.08
/// and 0.0125 while a direction is held; the value moves `input.x * movSpeed * 0.02` a
/// tick and is clamped to 0..1. The arena slider shows `arena * music` and sets
/// `InverseLerp(0, music, f)`.
#[test]
fn sliders_step_and_clamp() {
    let Some(mut h) = Harness::options() else {
        return;
    };
    let sfx = h.slider(SliderId::SfxVol);
    h.select(sfx);
    let start = h.ctx.shared.options.sound_effects_volume;
    assert_eq!(start, 0.8);
    // Held right: the menu reads the key the tick it is held.
    let mut speed = 0.0_f32;
    let mut expect = start;
    for _ in 0..5 {
        h.frame(&[KeyboardKey::RightArrow]);
        speed = speed.lerp_and_tick(1.0, 0.08, 0.0125);
        expect = (expect + speed * 0.02).clamp(0.0, 1.0);
        assert!((h.ctx.shared.options.sound_effects_volume - expect).abs() < 1e-6);
    }
    for _ in 0..200 {
        h.frame(&[KeyboardKey::RightArrow]);
    }
    assert_eq!(h.ctx.shared.options.sound_effects_volume, 1.0);
    let menu = h.options_menu().menu();
    assert_eq!(
        menu.selected,
        Some(sfx),
        "left and right do not leave a slider"
    );
    assert_eq!(menu.info_text(), "Sound effects volume: 100%");
    // The knob sits at the value along the 340-long line.
    let s = menu.slider(sfx).unwrap();
    assert!((menu.screen_pos(sfx).x - (s.anchor_point.x + 340.0 + 1.0 / 3.0)).abs() < 0.02);

    let arena = h.slider(SliderId::ArenaMusicVolume);
    h.select(arena);
    for _ in 0..300 {
        h.frame(&[KeyboardKey::LeftArrow]);
    }
    let o = &h.ctx.shared.options;
    assert_eq!((o.arena_music_volume, o.music_volume), (0.0, 0.8));
}

/// `SetCurrentlySelectedOfSeries`: a save slot reloads the progression; a resolution
/// sets `resolutionDirty`, and `Update` then sets the screen windowed at that size, shows
/// the cursor and switches to a new options menu, which after ten ticks of a wrong mode
/// puts the screen back to full screen with the cursor hidden.
#[test]
fn slots_and_resolutions() {
    let Some(mut h) = Harness::options() else {
        return;
    };
    let slot3 = h.button("SaveSlot", 2);
    h.select(slot3);
    h.tap(KeyboardKey::Return);
    assert_eq!(h.ctx.shared.options.save_slot, 2);
    assert_eq!(h.ctx.loaded_slots, [2]);
    assert_eq!(h.options_menu().menu().info_text(), "Select save slot 3");

    let res = h.button("ScreenRes", 0);
    h.select(res);
    h.tap(KeyboardKey::Return);
    assert_eq!(h.ctx.shared.options.resolution, 0);
    assert_eq!(h.ctx.shared.screen.size, [1024.0, 768.0]);
    assert!(!h.ctx.shared.screen.full_screen && h.ctx.shared.screen.cursor_visible);
    assert_eq!(h.ctx.shared.screen_size, [1024.0, 768.0]);
    assert_eq!(h.manager.upcoming(), Some(ProcessId::OptionsMenu));
    h.settle();
    for _ in 0..12 {
        h.frame(&[]);
    }
    assert!(h.ctx.shared.screen.full_screen && !h.ctx.shared.screen.cursor_visible);

    // The screen-mode button: windowed, and the new menu offers FULLSCREEN.
    let fs = h.button("WINDOWED", 0);
    h.select(fs);
    h.tap(KeyboardKey::Return);
    assert!(h.ctx.shared.options.windowed);
    h.settle();
    h.button("FULLSCREEN", 0);
    // BACK saves the options and goes to the main menu.
    let back = h.button("BACK", 0);
    h.select(back);
    h.tap(KeyboardKey::Return);
    h.settle();
    assert_eq!(h.id(), Some(ProcessId::MainMenu));
    assert_eq!(h.ctx.saved_options, 1);
}

/// The port's importer: a confirmation over the frozen menu, then the import of the
/// selected slot and a report.
#[test]
fn import_confirms_then_reports() {
    let Some(mut h) = Harness::options() else {
        return;
    };
    h.tap(KeyboardKey::RightArrow);
    assert_eq!(
        h.options_menu().menu().selected,
        Some(h.options_menu().import_button())
    );
    assert_eq!(
        h.options_menu().menu().info_text(),
        "Import save slot 1 from Rain World"
    );
    h.tap(KeyboardKey::Return);
    let text = h
        .options_menu()
        .dialog()
        .expect("a dialog")
        .text()
        .to_owned();
    assert!(text.starts_with("Replace save slot 1 with Rain World's save slot 1?"));
    // CANCEL (right of OK) closes it without importing.
    h.tap(KeyboardKey::RightArrow);
    h.tap(KeyboardKey::Return);
    assert!(h.options_menu().dialog().is_none());
    assert!(h.ctx.imports.is_empty());
    // Escape is ignored while a dialog is open; OK imports.
    h.tap(KeyboardKey::Return);
    assert!(h.options_menu().dialog().is_some());
    h.tap(KeyboardKey::Escape);
    assert_eq!(h.id(), Some(ProcessId::OptionsMenu));
    assert!(h.manager.upcoming().is_none());
    h.tap(KeyboardKey::Return);
    assert_eq!(h.ctx.imports, [SaveSlot(0)]);
    let report = h
        .options_menu()
        .dialog()
        .expect("a report")
        .text()
        .to_owned();
    assert_eq!(report, "Imported save slot 1 from Rain World.");
    h.tap(KeyboardKey::Return);
    assert!(h.options_menu().dialog().is_none());
}
