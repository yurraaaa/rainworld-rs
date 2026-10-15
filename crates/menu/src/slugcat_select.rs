//! `SlugcatSelectMenu` for a new game in the base game (no saves, Remix, Jolly Co-op
//! and More Slugcats off, flat illustrations): a page per slugcat that scrolls
//! sideways, the hold-to-start button, back, the arrows and the restart check box,
//! which stays hidden without a save.

use rw_sim::{FloatExt, Vec2};

use crate::color::MenuColor;
use crate::menu::{Kind, Menu, MenuCx, MenuScreen, ObjectId, SceneId};
use crate::process::{MainLoopProcess, ProcessId, ProcessRequests};
use crate::shared::{MenuContext, MenuShared, Slugcat, StoryGameInitCondition};
use crate::sound::MenuSoundId;

/// A `SlugcatPageNewGame`.
#[derive(Debug, Clone)]
pub struct SlugcatPage {
    pub page: ObjectId,
    pub slugcat: Slugcat,
    pub scene: ObjectId,
    pub image_pos: Vec2,
    pub scene_offset: Vec2,
    pub difficulty_label: ObjectId,
    pub info_label: ObjectId,
    pub in_position: f32,
    pub last_in_position: f32,
}

/// The `SlugcatSelectMenu` subclass's state.
#[derive(Debug, Clone)]
pub struct SlugcatSelectScreen {
    pub pages: Vec<SlugcatPage>,
    pub slugcat_page_index: usize,
    pub scroll: f32,
    pub last_scroll: f32,
    pub qued_side_input: i32,
    pub start_button: ObjectId,
    pub back_button: ObjectId,
    pub prev_button: ObjectId,
    pub next_button: ObjectId,
    pub restart_checkbox: ObjectId,
    restart_checked: bool,
    restart_available: bool,
    restart_up: f32,
    last_pause_button: bool,
    screen_width: f32,
}

/// The slugcat select process.
pub struct SlugcatSelectMenu {
    menu: Menu,
    screen: SlugcatSelectScreen,
}

impl SlugcatPage {
    /// `ScrollMagnitude`.
    pub const SCROLL_MAGNITUDE: f32 = 500.0;

    /// The game's English text for a page: the name and the blurb with its line
    /// delimiter replaced (`Custom.ReplaceLineDelimeters`).
    #[must_use]
    pub fn texts(slugcat: Slugcat, shared: &MenuShared) -> (String, String) {
        let t = &shared.translator;
        let (name, blurb) = match slugcat {
            Slugcat::WHITE => (
                t.translate("THE SURVIVOR"),
                t.translate(
                    "A nimble omnivore, both predator and prey. Lost in a harsh and indifferent<LINE>land you must make your own way, with wit and caution as your greatest assets.",
                ),
            ),
            Slugcat::YELLOW => (
                t.translate("THE MONK"),
                t.translate(
                    "Weak of body but strong of spirit. In tune with the mysteries of the world and<LINE>empathetic to its creatures, your journey will be a significantly more peaceful one.",
                ),
            ),
            // The Hunter. In English the game replaces the translation with a fixed text.
            _ => (
                t.translate("THE HUNTER"),
                "Strong and quick, with a fierce metabolism requiring a steady diet of meat. But the<LINE>stomach won't be your only concern, as the path of the hunter is one of extreme peril.",
            ),
        };
        (
            name.to_owned(),
            blurb.replace("<LINE>", "\r\n").replace("<WWLINE>", ""),
        )
    }
}

impl SlugcatSelectScreen {
    /// `slugcatColorOrder` of the base game.
    pub const ORDER: [Slugcat; 3] = [Slugcat::YELLOW, Slugcat::WHITE, Slugcat::RED];

    /// `NextScroll`.
    #[must_use]
    pub fn next_scroll(&self) -> f32 {
        self.scroll
            .lerp_and_tick(0.0, 0.07, (self.qued_side_input.abs() + 1) as f32 / 30.0)
    }

    /// `SlugcatPage.Scroll(timeStacker)` of page `i`: its offset from the centre in
    /// pages, wrapped into -1..1.
    #[must_use]
    pub fn page_scroll(&self, i: usize, t: f32) -> f32 {
        let count = self.pages.len() as f32;
        let mut num =
            (i as f32 - self.slugcat_page_index as f32) - self.last_scroll.lerp(self.scroll, t);
        if num < -1.0 {
            num += count;
        } else if num > 1.0 {
            num -= count;
        }
        num
    }

    /// `UseAlpha`.
    #[must_use]
    pub fn use_alpha(&self, i: usize, t: f32) -> f32 {
        Menu::s_curve((1.0 - self.page_scroll(i, t).abs()).clamp(0.0, 1.0), 0.5)
    }

    /// `SlugcatSelectMenu.SlugcatUnlocked` with no save: `SlugcatStats.SlugcatUnlocked`.
    #[must_use]
    pub fn slugcat_unlocked(slugcat: Slugcat, shared: &MenuShared) -> bool {
        slugcat != Slugcat::RED || shared.progression.red_unlocked
    }

    fn page_index(&self, page: ObjectId) -> Option<usize> {
        self.pages.iter().position(|p| p.page == page)
    }

    /// `UpdateStartButtonText` with no save: "NEW GAME", held for 40 ticks (120 with
    /// restart checked).
    fn update_start_button_text(&self, menu: &mut Menu, shared: &MenuShared) {
        if let Some(h) = menu.hold_button_mut(self.start_button) {
            h.fill_time = if self.restart_checked { 120.0 } else { 40.0 };
        }
        let label = menu.hold_button(self.start_button).map(|h| h.menu_label);
        if let Some(label) = label {
            menu.set_label_text(label, shared.translator.translate("NEW GAME"));
        }
    }

    /// `UpdateSelectedSlugcatInMiscProg`.
    fn update_selected_slugcat(&self, shared: &mut MenuShared) {
        if let Some(page) = self.pages.get(self.slugcat_page_index) {
            shared.progression.selected_slugcat = page.slugcat;
        }
    }

    /// `StartGame` for a new game: the Hunter needs unlocking; the Survivor and the Monk
    /// would get their intro slideshow first (not ported: D13).
    fn start_game(&mut self, slugcat: Slugcat, cx: &mut MenuCx<'_>) {
        if slugcat == Slugcat::RED && !cx.shared.progression.red_unlocked {
            return;
        }
        cx.shared.in_game_slugcat = Some(slugcat);
        cx.shared.progression.selected_slugcat = slugcat;
        cx.shared.start_game_condition = Some(StoryGameInitCondition::New);
        cx.requests.switch_to(ProcessId::Game);
        cx.play_sound(MenuSoundId::StartNewGame);
        if let Some(song) = cx.shared.intro_roll_song() {
            song.fade_out(20.0);
        }
    }

    /// `SlugcatSelectMenu.Update` after the base.
    fn update(&mut self, menu: &mut Menu, cx: &mut MenuCx<'_>) {
        if self.restart_available {
            if let Some(b) = menu.behavior_mut(self.restart_checkbox) {
                b.greyed_out = false;
            }
            menu.set_check_box_selectable(self.restart_checkbox, true);
        } else {
            if let Some(b) = menu.behavior_mut(self.restart_checkbox) {
                b.greyed_out = true;
            }
            menu.set_check_box_selectable(self.restart_checkbox, false);
            // `restartCheckbox.Checked = false` reports to `SetChecked`.
            self.set_restart_checked(menu, cx.shared, false);
            self.restart_checked = false;
            // No save: the check box stays down.
            self.restart_up = 0.0;
            self.restart_available = false;
        }
        if let Some(h) = menu.hold_button_mut(self.start_button) {
            h.warning_mode = self.restart_checked;
        }
        menu.object_mut(self.restart_checkbox).pos.y = (-50.0_f32).lerp(30.0, self.restart_up);
        let pause = cx.shared.input.pause_button();
        if pause && !self.last_pause_button {
            cx.requests.switch_to(ProcessId::MainMenu);
            cx.play_sound(MenuSoundId::SwitchPageOut);
        }
        self.last_pause_button = pause;
        self.last_scroll = self.scroll;
        self.scroll = self.next_scroll();
        let slugcat = Self::ORDER[self.slugcat_page_index];
        let greyed = self.scroll.abs() > 0.1 || !Self::slugcat_unlocked(slugcat, cx.shared);
        if let Some(b) = menu.behavior_mut(self.start_button) {
            b.greyed_out = greyed;
        }
        if self.last_scroll.abs() > 0.5 && self.scroll.abs() <= 0.5 {
            self.update_start_button_text(menu, cx.shared);
        }
        if self.scroll != 0.0 || self.last_scroll != 0.0 {
            return;
        }
        let count = self.pages.len();
        if self.qued_side_input < 0 {
            self.qued_side_input += 1;
            self.slugcat_page_index = (self.slugcat_page_index + count - 1) % count;
            self.update_selected_slugcat(cx.shared);
            self.scroll = 1.0;
            self.last_scroll = 1.0;
            self.restart_available = false;
        } else if self.qued_side_input > 0 {
            self.qued_side_input -= 1;
            self.slugcat_page_index = (self.slugcat_page_index + 1) % count;
            self.update_selected_slugcat(cx.shared);
            self.scroll = -1.0;
            self.last_scroll = -1.0;
            self.restart_available = false;
        }
    }

    fn set_restart_checked(&mut self, menu: &mut Menu, shared: &MenuShared, checked: bool) {
        self.restart_checked = checked;
        self.update_start_button_text(menu, shared);
    }
}

impl MenuScreen for SlugcatSelectScreen {
    fn signal(&mut self, _menu: &mut Menu, cx: &mut MenuCx<'_>, _sender: ObjectId, message: &str) {
        match message {
            "BACK" => {
                cx.requests.switch_to(ProcessId::MainMenu);
                cx.play_sound(MenuSoundId::SwitchPageOut);
            }
            "START" => {
                let slugcat = self.pages[self.slugcat_page_index].slugcat;
                self.start_game(slugcat, cx);
            }
            "PREV" => {
                self.qued_side_input = (self.qued_side_input - 1).max(-3);
                cx.play_sound(MenuSoundId::NextSlugcat);
            }
            "NEXT" => {
                self.qued_side_input = (self.qued_side_input + 1).min(3);
                cx.play_sound(MenuSoundId::NextSlugcat);
            }
            _ => {}
        }
    }

    fn init(&mut self, menu: &mut Menu) {
        menu.selected = Some(self.start_button);
    }

    /// `SlugcatPage.Update` (no mark or glow on a new-game page).
    fn page_update(&mut self, menu: &mut Menu, page: ObjectId, _cx: &mut MenuCx<'_>) {
        let Some(i) = self.page_index(page) else {
            return;
        };
        let alpha = self.use_alpha(i, 1.0);
        let scroll = self.page_scroll(i, 1.0);
        if let Some(s) = menu.scene_mut(self.pages[i].scene) {
            s.camera_range = 0.5 * alpha;
        }
        let target = (1.0 - scroll.abs()).inverse_lerp(0.5, 1.0).powi(5);
        let p = &mut self.pages[i];
        p.last_in_position = p.in_position;
        if target < p.in_position {
            p.in_position = target;
        } else {
            p.in_position = p.in_position.lerp_and_tick(target, 0.01, 1.0 / 300.0);
        }
    }

    /// `SlugcatPage.GrafUpdate` and `SlugcatPageNewGame.GrafUpdate`: the flat image and
    /// the labels slide with the scroll and fade away from the centre.
    fn page_graf_update(&self, menu: &mut Menu, page: ObjectId, t: f32) {
        let Some(i) = self.page_index(page) else {
            return;
        };
        let p = &self.pages[i];
        let scroll = self.page_scroll(i, t);
        let alpha = self.use_alpha(i, t);
        let flats = match &menu.object(p.scene).kind {
            Kind::Scene(s) => s.flat_illustrations.clone(),
            _ => Vec::new(),
        };
        for flat in flats {
            let pos = menu.object(flat).pos + p.scene_offset;
            let Kind::Illustration(il) = &menu.object(flat).kind else {
                continue;
            };
            let (node, il_alpha) = (il.sprite, il.alpha);
            let tr = menu.stage.transform_mut(node);
            tr.x = pos.x + scroll * SlugcatPage::SCROLL_MAGNITUDE;
            tr.y = pos.y;
            tr.alpha = il_alpha * alpha;
        }
        let x = self.screen_width / 2.0 + scroll * SlugcatPage::SCROLL_MAGNITUDE + 0.01;
        for label in [p.difficulty_label, p.info_label] {
            if let Kind::Label(l) = &menu.object(label).kind {
                let tr = menu.stage.transform_mut(l.label);
                tr.alpha = alpha;
                tr.x = x;
            }
        }
    }

    fn checked(&self, _menu: &Menu, _check_box: ObjectId) -> bool {
        self.restart_checked
    }

    fn set_checked(&mut self, menu: &mut Menu, _check_box: ObjectId, checked: bool) {
        self.restart_checked = checked;
        if let Some(h) = menu.hold_button_mut(self.start_button) {
            h.fill_time = if checked { 120.0 } else { 40.0 };
        }
    }
}

impl SlugcatSelectMenu {
    /// `new SlugcatSelectMenu(manager)` with no saves.
    #[must_use]
    pub fn new(shared: &mut MenuShared) -> Self {
        let mut menu = Menu::new(ProcessId::SlugcatSelect, shared);
        let page0 = menu.add_page(shared, "main", 0);
        if shared.unlock_red_file() {
            shared.progression.red_unlocked = true;
        }
        let order = SlugcatSelectScreen::ORDER;
        // `SetSlugcatColorOrder`: the selected slugcat's page, else the Survivor's.
        let mut slugcat_page_index = 0;
        for (i, slugcat) in order.iter().enumerate() {
            if *slugcat == shared.progression.selected_slugcat {
                slugcat_page_index = i;
                break;
            }
            if *slugcat == Slugcat::WHITE {
                slugcat_page_index = i;
            }
        }
        let screen_width = shared.screen_size[0];
        let pages = order
            .iter()
            .enumerate()
            .map(|(j, &slugcat)| Self::add_new_game_page(&mut menu, shared, 1 + j, slugcat))
            .collect();
        let start_button =
            menu.add_hold_button(shared, page0, "", "START", Vec2::new(683.0, 85.0), 40.0);
        menu.add_sub(page0, start_button);
        let back_text = shared.translator.translate("BACK").to_owned();
        let back_button = menu.add_simple_button(
            shared,
            page0,
            &back_text,
            "BACK",
            Vec2::new(200.0, 668.0),
            Vec2::new(110.0, 30.0),
        );
        menu.add_sub(page0, back_button);
        menu.back_object = Some(back_button);
        let prev_button = menu.add_big_arrow_button(page0, "PREV", Vec2::new(200.0, 50.0), -1);
        menu.add_sub(page0, prev_button);
        let next_button = menu.add_big_arrow_button(page0, "NEXT", Vec2::new(1116.0, 50.0), 1);
        menu.add_sub(page0, next_button);
        // `GetRestartTextWidth` / `GetRestartTextOffset` for English; the safe screen
        // offset is zero.
        let (text_width, text_offset) = (85.0, 0.0);
        let restart_text = shared.translator.translate("Restart game").to_owned();
        let restart_checkbox = menu.add_check_box(
            shared,
            page0,
            Vec2::new(683.0 + 200.0 + text_offset, 30.0_f32.max(0.0)),
            text_width,
            &restart_text,
            "RESTART",
            false,
        );
        let label = menu.check_box_label(restart_checkbox);
        if let Some(label) = label {
            let width = menu.label_text_width(label);
            menu.object_mut(label).pos.x += text_width - width - 5.0;
        }
        menu.add_sub(page0, restart_checkbox);
        let screen = SlugcatSelectScreen {
            pages,
            slugcat_page_index,
            scroll: 0.0,
            last_scroll: 0.0,
            qued_side_input: 0,
            start_button,
            back_button,
            prev_button,
            next_button,
            restart_checkbox,
            restart_checked: false,
            restart_available: false,
            restart_up: 0.0,
            last_pause_button: false,
            screen_width,
        };
        screen.update_start_button_text(&mut menu, shared);
        screen.update_selected_slugcat(shared);
        menu.my_sound_loop = Some(MenuSoundId::MainMenuLoop);
        Self { menu, screen }
    }

    /// `new SlugcatPageNewGame(menu, null, pageIndex, slugcat)`.
    fn add_new_game_page(
        menu: &mut Menu,
        shared: &mut MenuShared,
        index: usize,
        slugcat: Slugcat,
    ) -> SlugcatPage {
        let name = format!("Slugcat_Page_{}", slugcat.value());
        let page = menu.add_page(shared, &name, index);
        // `AddImage(ascended: false)`.
        let mut image_pos = Vec2::new(683.0, 484.0);
        let (scene_id, mut scene_offset) = match slugcat {
            Slugcat::WHITE => (SceneId::SlugcatWhite, Vec2::new(-10.0, 100.0)),
            Slugcat::YELLOW => (SceneId::SlugcatYellow, Vec2::new(10.0, 75.0)),
            // The Hunter.
            _ => (SceneId::SlugcatRed, Vec2::new(10.0, 45.0)),
        };
        scene_offset.x -= (1366.0 - shared.screen_size[0]) / 2.0;
        let scene = menu.add_scene(shared, page, scene_id);
        menu.add_sub(page, scene);
        let (text, info) = SlugcatPage::texts(slugcat, shared);
        let lines = info.matches('\n').count();
        let extra = if lines > 1 { 30.0 } else { 0.0 };
        let difficulty_label = menu.add_label(
            shared,
            page,
            &text,
            Vec2::new(-1000.0, image_pos.y - 249.0 + extra),
            Vec2::new(200.0, 30.0),
            true,
        );
        menu.add_sub(page, difficulty_label);
        let info_label = menu.add_label(
            shared,
            page,
            &info,
            Vec2::new(-1000.0, image_pos.y - 249.0 - 60.0 + extra / 2.0),
            Vec2::new(400.0, 60.0),
            true,
        );
        menu.add_sub(page, info_label);
        if lines > 1 {
            image_pos.y += 30.0;
            scene_offset.y += 30.0;
        }
        let (name_color, info_color) = if SlugcatSelectScreen::slugcat_unlocked(slugcat, shared) {
            (MenuColor::MediumGrey, MenuColor::DarkGrey)
        } else {
            (MenuColor::VeryDarkGrey, MenuColor::VeryDarkGrey)
        };
        for (label, color) in [(difficulty_label, name_color), (info_label, info_color)] {
            if let Kind::Label(l) = &menu.object(label).kind {
                let node = l.label;
                menu.stage.label_mut(node).color = color.rgb();
            }
        }
        SlugcatPage {
            page,
            slugcat,
            scene,
            image_pos,
            scene_offset,
            difficulty_label,
            info_label,
            in_position: 0.0,
            last_in_position: 0.0,
        }
    }

    #[must_use]
    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    pub fn menu_mut(&mut self) -> &mut Menu {
        &mut self.menu
    }

    #[must_use]
    pub fn screen(&self) -> &SlugcatSelectScreen {
        &self.screen
    }
}

impl<C: MenuContext> MainLoopProcess<C> for SlugcatSelectMenu {
    fn id(&self) -> ProcessId {
        ProcessId::SlugcatSelect
    }

    fn update(&mut self, ctx: &mut C, requests: &mut ProcessRequests) {
        let mut cx = MenuCx {
            shared: ctx.menu(),
            requests,
        };
        self.menu.update(&mut self.screen, &mut cx);
        self.screen.update(&mut self.menu, &mut cx);
    }

    fn graf_update(&mut self, time_stacker: f32, _ctx: &mut C) {
        self.menu.graf_update(&self.screen, time_stacker);
    }

    fn shut_down(&mut self, ctx: &mut C) {
        self.menu.shut_down(ctx.menu());
    }
}
