//! The main menu and the slugcat select menu driven through the process manager with
//! the installed game's fonts, illustrations and `sounds.txt` (read-only). Set `RW_DIR`
//! to override the default install; tests skip when it is absent.
//!
//! Expected numbers come from the decompiled game (`Assembly-CSharp`, 1.11.8):
//! `MainMenu.AddMainMenuButton`, `Menu.Update`/`SelectNewObject`, `HoldButton.Update`,
//! `SlugcatSelectMenu.Update`/`NextScroll`, `SlugcatPage.Scroll` and `sounds.txt`;
//! each test names the lines it follows.

use std::path::PathBuf;
use std::sync::Arc;

use rw_assets::unity::AssetBundle;
use rw_assets::{SoundTable, StreamingAssets, UnityResources};
use rw_menu::menu::{Kind, ObjectId};
use rw_menu::{
    Initialization, KeyboardKey, MainLoopProcess, MainMenu, MenuContext, MenuMic, MenuShared,
    MenuSoundEvent, MenuSoundId, ProcessFactory, ProcessId, ProcessManager, ProcessRequests,
    Slugcat, SlugcatSelectMenu,
};
use rw_random::UnityRandom;
use rw_sim::{FloatExt, Vec2};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";
const DT: f32 = 0.025;

enum Proc {
    Init(Initialization),
    Main(Box<MainMenu>),
    Select(Box<SlugcatSelectMenu>),
    Other(ProcessId),
}

struct Ctx {
    shared: MenuShared,
    /// Every sound event the menu microphone produced, by frame.
    sounds: Vec<(u64, MenuSoundEvent)>,
    frame: u64,
}

impl MenuContext for Ctx {
    fn menu(&mut self) -> &mut MenuShared {
        &mut self.shared
    }
}

impl ProcessFactory<Proc> for Ctx {
    fn create_process(&mut self, id: ProcessId, _old: Option<&Proc>) -> Proc {
        match id {
            ProcessId::Initialization => Proc::Init(Initialization::new(ProcessId::MainMenu)),
            ProcessId::MainMenu => Proc::Main(Box::new(MainMenu::new(&mut self.shared))),
            ProcessId::SlugcatSelect => {
                Proc::Select(Box::new(SlugcatSelectMenu::new(&mut self.shared)))
            }
            other => Proc::Other(other),
        }
    }

    fn update_side_processes(&mut self, current: Option<&Proc>, _dt: f32) {
        let menu = matches!(current, Some(Proc::Main(_) | Proc::Select(_)));
        self.shared.mic.graf_update(menu);
        let frame = self.frame;
        self.sounds.extend(
            self.shared
                .mic
                .take_events()
                .into_iter()
                .map(|e| (frame, e)),
        );
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
            Self::Select(_) => ProcessId::SlugcatSelect,
            Self::Other(id) => *id,
        }
    }

    fn update(&mut self, ctx: &mut Ctx, requests: &mut ProcessRequests) {
        match self {
            Self::Init(p) => p.update(ctx, requests),
            Self::Main(p) => p.update(ctx, requests),
            Self::Select(p) => p.update(ctx, requests),
            Self::Other(_) => {}
        }
    }

    fn graf_update(&mut self, t: f32, ctx: &mut Ctx) {
        match self {
            Self::Main(p) => p.graf_update(t, ctx),
            Self::Select(p) => p.graf_update(t, ctx),
            _ => {}
        }
    }

    fn shut_down(&mut self, ctx: &mut Ctx) {
        match self {
            Self::Main(p) => p.shut_down(ctx),
            Self::Select(p) => p.shut_down(ctx),
            _ => {}
        }
    }

    fn communicate_with_upcoming(&mut self, next: &mut Self, ctx: &mut Ctx) {
        let next_menu = match next {
            Self::Main(p) => Some(p.menu_mut()),
            Self::Select(p) => Some(p.menu_mut()),
            _ => None,
        };
        let menu = match self {
            Self::Main(p) => p.menu_mut(),
            Self::Select(p) => p.menu_mut(),
            _ => return,
        };
        menu.communicate_with_upcoming(next_menu, &mut ctx.shared);
    }
}

struct Harness {
    manager: ProcessManager<Proc>,
    ctx: Ctx,
}

impl Harness {
    /// Boots into the main menu and runs until its fade-in is over (the menu is frozen
    /// while `fadeToBlack > 0.5`).
    fn boot() -> Option<Self> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let data = root.join("RainWorld_Data");
        if !data.join("resources.assets").is_file() {
            eprintln!("skipping: no game under {}", root.display());
            return None;
        }
        let assets = StreamingAssets::new(data.join("StreamingAssets"));
        let resources = UnityResources::open(&data).expect("resources");
        let bundle = AssetBundle::open(&assets.asset_bundle("loadedsoundeffects")).expect("bundle");
        let table = Arc::new(SoundTable::load(&assets, &bundle).expect("sounds.txt"));
        let mut shared = MenuShared::load(assets, &resources, [1366.0, 768.0]).expect("fonts");
        shared.random = UnityRandom::new(1);
        shared.mic = MenuMic::new(table);
        let mut ctx = Ctx {
            shared,
            sounds: Vec::new(),
            frame: 0,
        };
        let manager = ProcessManager::new([1366.0, 768.0], &mut ctx);
        let mut h = Self { manager, ctx };
        h.settle();
        assert_eq!(h.id(), Some(ProcessId::MainMenu));
        Some(h)
    }

    fn id(&self) -> Option<ProcessId> {
        self.manager.current().map(MainLoopProcess::id)
    }

    /// One frame of 1/40 s (one tick once a process runs) with `keys` held.
    fn frame(&mut self, keys: &[KeyboardKey]) {
        for &k in keys {
            self.ctx.shared.input.set_key(k, true);
        }
        self.ctx.frame += 1;
        self.manager.update(DT, &mut self.ctx);
        for &k in keys {
            self.ctx.shared.input.set_key(k, false);
        }
    }

    /// Runs until a switch is over and the fade from black has finished.
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

    fn main(&self) -> &MainMenu {
        match self.manager.current() {
            Some(Proc::Main(p)) => p,
            _ => panic!("not the main menu"),
        }
    }

    fn select(&self) -> &SlugcatSelectMenu {
        match self.manager.current() {
            Some(Proc::Select(p)) => p,
            _ => panic!("not the slugcat select menu"),
        }
    }

    fn selected(&self) -> Option<ObjectId> {
        match self.manager.current() {
            Some(Proc::Main(p)) => p.menu().selected,
            Some(Proc::Select(p)) => p.menu().selected,
            _ => None,
        }
    }

    /// The one-shots played since `from` (frame number), with `soundData.vol` and
    /// `soundData.pitch`.
    fn plays_since(&self, from: u64) -> Vec<(MenuSoundId, f32, f32)> {
        self.ctx
            .sounds
            .iter()
            .filter(|(f, _)| *f >= from)
            .filter_map(|(_, e)| match e {
                MenuSoundEvent::Play(s) => Some((s.id, s.volume, s.pitch)),
                _ => None,
            })
            .collect()
    }

    /// Taps a key: held for one frame, then two frames released. `Menu.Update` updates
    /// the pages before it reads the input, so the buttons see a press a tick late
    /// and a release another tick later.
    fn tap(&mut self, key: KeyboardKey) {
        self.frame(&[key]);
        self.frame(&[]);
        self.frame(&[]);
    }
}

/// `MainMenu.AddMainMenuButton` (538889): with six buttons of 110 x 30, `num6 = 290 +
/// 5 * 40 / 2 - (6 - 5) * 10 = 380`, `num7 = 683 - 110 / 2 = 628`, each button 40 lower;
/// `nextSelectable[1]`/`[3]` are the previous/next with wrap, `[0]`/`[2]` itself.
#[test]
fn main_menu_buttons_order_layout_and_links() {
    let Some(h) = Harness::boot() else { return };
    let menu = h.main().menu();
    let buttons = h.main().buttons();
    let texts: Vec<String> = buttons
        .iter()
        .map(|&b| menu.button(b).expect("button").signal_text.clone())
        .collect();
    assert_eq!(
        texts,
        ["STORY", "REGIONS", "ARENA", "REMIX", "OPTIONS", "EXIT"]
    );
    for (i, &b) in buttons.iter().enumerate() {
        let o = menu.object(b);
        assert_eq!(
            o.pos,
            Vec2::new(628.0, 380.0 - 40.0 * i as f32),
            "button {i}"
        );
        assert_eq!(o.size, Vec2::new(110.0, 30.0));
        // The page sits at (1/3, 1/3).
        assert_eq!(
            menu.screen_pos(b),
            Vec2::new(628.0, 380.0 - 40.0 * i as f32) + Vec2::new(1.0 / 3.0, 1.0 / 3.0)
        );
        let up = buttons[(i + 5) % 6];
        let down = buttons[(i + 1) % 6];
        assert_eq!(o.next_selectable, [Some(b), Some(up), Some(b), Some(down)]);
    }
    // `MainMenu.Update`: Regions is greyed with no discovered shelter.
    assert!(menu.behavior(buttons[1]).unwrap().greyed_out);
    assert!(!menu.behavior(buttons[0]).unwrap().greyed_out);
    // The title: the shadow, then the bevel drawn with MenuText, both at (378, 440).
    let page = menu.pages[0];
    let titles: Vec<&str> = menu
        .object(page)
        .sub_objects
        .iter()
        .filter_map(|&s| match &menu.object(s).kind {
            Kind::Illustration(i) if menu.object(s).pos == Vec2::new(378.0, 440.0) => {
                Some(i.file_name.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(titles, ["MainTitleShadow", "MainTitleBevel"]);
}

/// `Menu.Update` (539312): a new direction selects at once (`SelectNewObject`); held, it
/// repeats once `scrollInitDelay > 20` every time `scrollDelay > 6`. `SelectNewObject`
/// (539661) plays `MENU_Button_Select_Gamepad_Or_Keyboard` (UIMetal3/vol=0.3) or, onto
/// a greyed button, `MENU_Greyed_Out_Button_Select_Gamepad_Or_Keyboard`
/// (UIMetal3/vol=0.2/pitch=0.3).
#[test]
fn keyboard_selection_wraps_repeats_and_sounds() {
    let Some(mut h) = Harness::boot() else { return };
    let buttons = h.main().buttons();
    // `Init`: no back object, so the page's first selectable.
    assert_eq!(h.selected(), Some(buttons[0]));
    let from = h.ctx.frame + 1;
    h.tap(KeyboardKey::DownArrow);
    assert_eq!(h.selected(), Some(buttons[1]));
    assert_eq!(
        h.plays_since(from),
        [(
            MenuSoundId::GreyedOutButtonSelectGamepadOrKeyboard,
            0.2,
            0.3
        )]
    );
    let from = h.ctx.frame + 1;
    h.tap(KeyboardKey::DownArrow);
    assert_eq!(h.selected(), Some(buttons[2]));
    assert_eq!(
        h.plays_since(from),
        [(MenuSoundId::ButtonSelectGamepadOrKeyboard, 0.3, 1.0)]
    );
    h.tap(KeyboardKey::UpArrow);
    h.tap(KeyboardKey::UpArrow);
    h.tap(KeyboardKey::UpArrow);
    assert_eq!(
        h.selected(),
        Some(buttons[5]),
        "up from STORY wraps to EXIT"
    );
    h.tap(KeyboardKey::LeftArrow);
    assert_eq!(h.selected(), Some(buttons[5]), "left links to itself");

    // Held down for 50 ticks from EXIT: moves on ticks 1, 28, 35, 42 and 49.
    let mut moves = Vec::new();
    let mut last = h.selected();
    for tick in 1..=50 {
        h.frame(&[KeyboardKey::DownArrow]);
        if h.selected() != last {
            moves.push(tick);
            last = h.selected();
        }
    }
    assert_eq!(moves, [1, 28, 35, 42, 49]);
    assert_eq!(h.selected(), Some(buttons[(5 + 5) % 6]));
}

/// The background loop starts on the menu's first update (`mySoundLoopID`
/// MENU_Main_Menu_LOOP: MenuLOOP/vol=0.4) and the slugcat select menu, with the same
/// loop, takes it over (`Menu.CommunicateWithUpcomingProcess`, 539621). STORY plays
/// `MENU_Switch_Page_In` (UIArp/vol=0.4) and requests `SlugcatSelect` (538932); a press
/// plays `MENU_Button_Press_Init` (UIMetal3/vol=0.1/pitch=0.5) and the click comes on
/// release (`ButtonBehavior.Update`, 540463).
#[test]
fn story_goes_to_slugcat_select_keeping_the_loop() {
    let Some(mut h) = Harness::boot() else { return };
    let loops: Vec<_> = h
        .ctx
        .sounds
        .iter()
        .filter_map(|(_, e)| match e {
            MenuSoundEvent::StartLoop(handle, s) => Some((*handle, s.id, s.volume)),
            _ => None,
        })
        .collect();
    assert_eq!(loops.len(), 1);
    let (handle, id, volume) = loops[0];
    assert_eq!((id, volume), (MenuSoundId::MainMenuLoop, 0.4));
    let from = h.ctx.frame + 1;
    // Tick 1 reads the key; tick 2's buttons see the press; tick 3's the release.
    h.frame(&[KeyboardKey::Z]);
    assert_eq!(h.plays_since(from), []);
    h.frame(&[]);
    assert_eq!(
        h.plays_since(from),
        [(MenuSoundId::ButtonPressInit, 0.1, 0.5)]
    );
    assert_eq!(h.manager.upcoming(), None);
    h.frame(&[]);
    assert_eq!(h.manager.upcoming(), Some(ProcessId::SlugcatSelect));
    assert_eq!(
        h.plays_since(from + 2),
        [(MenuSoundId::SwitchPageIn, 0.4, 1.0)]
    );
    h.settle();
    assert_eq!(h.id(), Some(ProcessId::SlugcatSelect));
    assert_eq!(h.select().menu().sound_loop, Some(handle));
    assert!(
        !h.ctx
            .sounds
            .iter()
            .any(|(f, e)| *f > from && matches!(e, MenuSoundEvent::StartLoop(..))),
        "no second loop"
    );
    // `SlugcatSelectMenu.Init` (554926) selects the start button.
    assert_eq!(h.selected(), Some(h.select().screen().start_button));
}

fn to_slugcat_select() -> Option<Harness> {
    let mut h = Harness::boot()?;
    h.tap(KeyboardKey::Z);
    h.settle();
    assert_eq!(h.id(), Some(ProcessId::SlugcatSelect));
    Some(h)
}

/// `SlugcatSelectMenu` constructor (554724) and `SlugcatPageNewGame` (554172): pages
/// Monk, Survivor, Hunter with the Survivor's shown; START a 55-radius hold button at
/// (683, 85) filling in 40 ticks; BACK 110 x 30 at (200, 668); arrows 50 x 50 at
/// (200, 50) and (1116, 50); the labels at `imagePos.y - 249` = 235 (200 x 30) and
/// 175 (400 x 60), the Hunter's very dark grey while locked.
#[test]
fn slugcat_select_layout() {
    let Some(h) = to_slugcat_select() else { return };
    let s = h.select().screen();
    let menu = h.select().menu();
    let order: Vec<Slugcat> = s.pages.iter().map(|p| p.slugcat).collect();
    assert_eq!(order, [Slugcat::Yellow, Slugcat::White, Slugcat::Red]);
    assert_eq!(s.slugcat_page_index, 1);
    let start = menu.object(s.start_button);
    assert_eq!((start.pos, start.rad), (Vec2::new(683.0, 85.0), 55.0));
    assert_eq!(menu.hold_button(s.start_button).unwrap().fill_time, 40.0);
    for (id, pos, size) in [
        (s.back_button, (200.0, 668.0), (110.0, 30.0)),
        (s.prev_button, (200.0, 50.0), (50.0, 50.0)),
        (s.next_button, (1116.0, 50.0), (50.0, 50.0)),
    ] {
        assert_eq!(menu.object(id).pos, Vec2::new(pos.0, pos.1));
        assert_eq!(menu.object(id).size, Vec2::new(size.0, size.1));
    }
    assert_eq!(menu.back_object, Some(s.back_button));
    // No save: the restart check box went down to y = -50 on the first update.
    assert_eq!(menu.object(s.restart_checkbox).pos.y, -50.0);
    for p in &s.pages {
        assert_eq!(
            menu.object(p.difficulty_label).pos,
            Vec2::new(-1000.0, 235.0)
        );
        assert_eq!(menu.object(p.difficulty_label).size, Vec2::new(200.0, 30.0));
        assert_eq!(menu.object(p.info_label).pos, Vec2::new(-1000.0, 175.0));
        assert_eq!(menu.object(p.info_label).size, Vec2::new(400.0, 60.0));
    }
}

/// `HoldButton.Update` (540567): held, `filled = LerpAndTick(filled, 1, 0.007, 1 /
/// fillTime)` (`Custom.LerpAndTick`, 467748) each tick, and the signal once `filled >= 1`;
/// the first held tick plays `MENU_Security_Button_Init` (UIMetal2/vol=0.3/pitch=0.5).
/// START then requests the game with `MENU_Start_New_Game` (UIImpact2/vol=0.4) for a new
/// Survivor game (`StartGame`, 555445).
#[test]
fn hold_start_fills_then_starts_a_new_game() {
    let Some(mut h) = to_slugcat_select() else {
        return;
    };
    let start = h.select().screen().start_button;
    let mut want = 0.0_f32;
    let mut want_ticks = 0;
    while want < 1.0 {
        want = want.lerp_and_tick(1.0, 0.007, 1.0 / 40.0);
        want_ticks += 1;
    }
    let from = h.ctx.frame + 1;
    let mut filled = Vec::new();
    let mut ticks = 0;
    while h.manager.upcoming().is_none() {
        h.frame(&[KeyboardKey::Z]);
        ticks += 1;
        filled.push(h.select().menu().hold_button(start).unwrap().filled);
        assert!(ticks <= 100, "never signalled");
    }
    // The first tick only reads the key (the button sees `holdButton` a tick later).
    assert_eq!(filled[0], 0.0);
    assert_eq!(ticks, want_ticks + 1);
    assert_eq!(want_ticks, 36, "0.9 s of holding for a 40-tick fill time");
    let mut f = 0.0_f32;
    for (i, got) in filled.iter().enumerate().skip(1) {
        f = f.lerp_and_tick(1.0, 0.007, 1.0 / 40.0);
        assert_eq!(*got, f, "tick {}", i + 1);
    }
    assert_eq!(h.manager.upcoming(), Some(ProcessId::Game));
    let plays = h.plays_since(from);
    assert_eq!(plays[0], (MenuSoundId::ButtonPressInit, 0.1, 0.5));
    assert_eq!(plays[1], (MenuSoundId::SecurityButtonInit, 0.3, 0.5));
    assert_eq!(
        plays.last().copied(),
        Some((MenuSoundId::StartNewGame, 0.4, 1.0))
    );
    assert_eq!(h.ctx.shared.in_game_slugcat, Some(Slugcat::White));
}

/// NEXT (`Singal`, 555162) queues a side input and plays `MENU_Next_Slugcat`
/// (PLAYALL: UIMetal1/vol=0.4/pitch=0.9 and UIArp/vol=0.2). The next `Update` (554932)
/// moves to the next page with `scroll = -1`, which then follows `NextScroll =
/// LerpAndTick(scroll, 0, 0.07, (|quedSideInput| + 1) / 30)` back to 0; a page sits
/// `((index - slugcatPageIndex) - scroll)` pages (wrapped to -1..1) from the centre,
/// 500 px each (`SlugcatPage.Scroll`, 554007). START is greyed while `|scroll| > 0.1`
/// and on the locked Hunter.
#[test]
fn next_scrolls_to_the_hunter() {
    let Some(mut h) = to_slugcat_select() else {
        return;
    };
    let s = h.select().screen().clone();
    h.tap(KeyboardKey::RightArrow);
    assert_eq!(h.selected(), Some(s.next_button));
    let from = h.ctx.frame + 1;
    h.tap(KeyboardKey::Z);
    let next: Vec<_> = h
        .plays_since(from)
        .into_iter()
        .filter(|p| p.0 == MenuSoundId::NextSlugcat)
        .collect();
    assert_eq!(
        next,
        [
            (MenuSoundId::NextSlugcat, 0.4, 0.9),
            (MenuSoundId::NextSlugcat, 0.2, 1.0)
        ]
    );
    let sc = h.select().screen();
    assert_eq!(
        (sc.slugcat_page_index, sc.scroll, sc.last_scroll),
        (2, -1.0, -1.0)
    );
    assert_eq!(h.ctx.shared.progression.selected_slugcat, Slugcat::Red);
    let mut scroll = -1.0_f32;
    let mut ticks = 0;
    while scroll != 0.0 {
        scroll = scroll.lerp_and_tick(0.0, 0.07, 1.0 / 30.0);
        h.frame(&[]);
        ticks += 1;
        let sc = h.select().screen();
        assert_eq!(sc.scroll, scroll, "tick {ticks}");
        let menu = h.select().menu();
        let start_greyed = menu.behavior(sc.start_button).unwrap().greyed_out;
        assert!(
            start_greyed,
            "greyed while scrolling and on the locked Hunter"
        );
        for i in 0..3 {
            let mut want = (i as f32 - 2.0) - scroll;
            if want < -1.0 {
                want += 3.0;
            } else if want > 1.0 {
                want -= 3.0;
            }
            assert_eq!(sc.page_scroll(i, 1.0), want);
        }
    }
    assert_eq!(ticks, 16);
    // The Hunter's labels at the centre once drawn at t = 1.
    if let Some(Proc::Select(p)) = h.manager.current_mut() {
        p.graf_update(1.0, &mut h.ctx);
    }
    let menu = h.select().menu();
    let red = &h.select().screen().pages[2];
    if let Kind::Label(l) = &menu.object(red.difficulty_label).kind {
        let label = menu.stage.label(l.label);
        assert_eq!(label.transform.x, 683.01);
        assert_eq!(label.transform.y, 235.0 + 15.0 + 1.0 / 3.0);
        assert_eq!(label.text(), "THE HUNTER");
    }
}

/// `SlugcatSelectMenu.Update`: the pause button (Escape) goes back to the main menu with
/// `MENU_Switch_Page_Out` (UIMetalHit/vol=0.3).
#[test]
fn escape_goes_back_to_the_main_menu() {
    let Some(mut h) = to_slugcat_select() else {
        return;
    };
    let from = h.ctx.frame + 1;
    h.frame(&[KeyboardKey::Escape]);
    assert_eq!(h.manager.upcoming(), Some(ProcessId::MainMenu));
    assert!(
        h.plays_since(from)
            .contains(&(MenuSoundId::SwitchPageOut, 0.3, 1.0))
    );
    h.frame(&[]);
    h.settle();
    assert_eq!(h.id(), Some(ProcessId::MainMenu));
}

/// EXIT (`ExitButtonPressed`, 538972) calls `Application.Quit`.
#[test]
fn exit_quits() {
    let Some(mut h) = Harness::boot() else { return };
    h.tap(KeyboardKey::UpArrow);
    assert_eq!(h.selected(), Some(h.main().buttons()[5]));
    assert!(!h.ctx.shared.quit);
    h.tap(KeyboardKey::Z);
    assert!(h.ctx.shared.quit);
}
