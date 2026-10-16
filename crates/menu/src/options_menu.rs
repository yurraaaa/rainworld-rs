//! `OptionsMenu` of the base game with Remix off (not a console): screen mode and
//! resolution, language, save slot, reset progress, the volume sliders, vibration,
//! speedrun validation, the input settings and credits buttons, BACK; and the port's
//! save importer (docs/DIVERGENCES.md D13). Behaviour: `rainworld-re/notes/menu.md`.

use rw_save::{Language, Options, SaveSlot};
use rw_sim::{FloatExt, Vec2};

use crate::dialog::{Dialog, DialogAnswer};
use crate::menu::{Kind, Menu, MenuCx, MenuScreen, ObjectId, SceneId, SimpleKind, SliderId};
use crate::process::{MainLoopProcess, ProcessId, ProcessRequests};
use crate::shared::{MenuContext, MenuShared};
use crate::sound::MenuSoundId;
use crate::stage::{Sprite, StageColor};

/// The `SelectOneButton` series (their signal texts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Series {
    ScreenRes,
    Language,
    SaveSlot,
}

impl Series {
    pub const ALL: [Self; 3] = [Self::ScreenRes, Self::Language, Self::SaveSlot];

    #[must_use]
    pub const fn value(self) -> &'static str {
        match self {
            Self::ScreenRes => "ScreenRes",
            Self::Language => "Language",
            Self::SaveSlot => "SaveSlot",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.value() == value)
    }
}

/// What a tick asks of the app (its context), after the menu's update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextRequest {
    LoadSaveSlot,
    SaveOptions,
    Import(SaveSlot),
}

/// What the open dialog is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogPurpose {
    ConfirmImport(SaveSlot),
    Report,
}

/// The `OptionsMenu` subclass's state.
struct OptionsScreen {
    resolution_dirty: bool,
    language_dirty: bool,
    back_button: ObjectId,
    credits_button: ObjectId,
    save_backups_button: ObjectId,
    import_button: ObjectId,
    wipe_save_button: ObjectId,
    controls_button: ObjectId,
    reset_warning_text: ObjectId,
    validation_checkbox: ObjectId,
    vibration_checkbox: ObjectId,
    /// `options.vibration` and `options.validation`, which the check boxes toggle.
    vibration: bool,
    validation: bool,
    arena_music_slider: ObjectId,
    reset_warning_text_counter: i32,
    reset_warning_text_alpha: f32,
    wrong_fullscreen_setting_counter: i32,
    last_pause_button: bool,
    exiting: bool,
    requests: Vec<ContextRequest>,
    dialog: Option<(Dialog, DialogPurpose)>,
}

/// The options menu process.
pub struct OptionsMenu {
    menu: Menu,
    screen: OptionsScreen,
}

impl OptionsMenu {
    /// `languageOrder`.
    pub const LANGUAGE_ORDER: [Language; 10] = [
        Language::English,
        Language::French,
        Language::Italian,
        Language::German,
        Language::Spanish,
        Language::Portuguese,
        Language::Japanese,
        Language::Korean,
        Language::Russian,
        Language::Chinese,
    ];
    /// The port's import button (not the game's).
    const IMPORT: &'static str = "IMPORT";

    /// `GetSaveSlotButtonWidth`.
    #[must_use]
    pub fn save_slot_button_width(language: Language) -> f32 {
        match language {
            Language::French => 220.0,
            Language::Italian | Language::Japanese => 160.0,
            Language::German => 130.0,
            Language::Spanish | Language::Russian | Language::Thai => 170.0,
            Language::Portuguese => 150.0,
            _ => 100.0,
        }
    }

    /// `ResolutionString`.
    #[must_use]
    pub fn resolution_string(index: usize) -> String {
        let mut size = Options::SCREEN_RESOLUTIONS[index];
        if Options::is_steam_deck_resolution(size) {
            size = [1280.0, 800.0];
        }
        format!(
            "{} x {}  [{}]",
            size[0] as i32,
            size[1] as i32,
            Options::ASPECT_RATIOS[index]
        )
    }

    /// `new OptionsMenu(manager)`.
    #[must_use]
    pub fn new(shared: &mut MenuShared) -> Self {
        let mut menu = Menu::new(ProcessId::OptionsMenu, shared);
        let page = menu.add_page(shared, "main", 0);
        let scene = menu.add_scene(shared, page, SceneId::LandscapeSu);
        menu.add_sub(page, scene);
        menu.scene = Some(scene);
        let mut dark = Sprite::new("pixel");
        dark.color = StageColor::BLACK;
        dark.anchor = [0.0, 0.0];
        dark.transform.scale_x = 1368.0;
        dark.transform.scale_y = 770.0;
        dark.transform.x = -1.0;
        dark.transform.y = -1.0;
        dark.transform.alpha = 0.85;
        let container = menu.container_of(page);
        menu.stage.add_sprite(container, dark);
        let mut b = Builder { menu, page };
        let screen = b.build(shared);
        let mut menu = b.menu;
        menu.my_sound_loop = Some(MenuSoundId::MainMenuLoop);
        Self { menu, screen }
    }

    #[must_use]
    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    pub fn menu_mut(&mut self) -> &mut Menu {
        &mut self.menu
    }

    /// The dialog over the menu, if one is open.
    #[must_use]
    pub fn dialog(&self) -> Option<&Dialog> {
        self.screen.dialog.as_ref().map(|(d, _)| d)
    }

    /// The port's import button.
    #[must_use]
    pub fn import_button(&self) -> ObjectId {
        self.screen.import_button
    }
}

/// Builds the menu's objects in the constructor's order.
struct Builder {
    menu: Menu,
    page: ObjectId,
}

impl Builder {
    fn tr(shared: &MenuShared, text: &str) -> String {
        shared.translator.translate(text).to_owned()
    }

    fn label(&mut self, shared: &MenuShared, text: &str, pos: Vec2, size: Vec2) -> ObjectId {
        let l = self
            .menu
            .add_label(shared, self.page, text, pos, size, false);
        self.menu.add_sub(self.page, l);
        l
    }

    fn simple(&mut self, shared: &MenuShared, text: &str, signal: &str, pos: Vec2) -> ObjectId {
        let b = self.menu.add_simple_button(
            shared,
            self.page,
            &Self::tr(shared, text),
            signal,
            pos,
            Vec2::new(110.0, 30.0),
        );
        self.menu.add_sub(self.page, b);
        b
    }

    #[allow(clippy::too_many_arguments)]
    fn select_one(
        &mut self,
        shared: &MenuShared,
        text: &str,
        series: Series,
        pos: Vec2,
        size: Vec2,
        index: usize,
    ) -> ObjectId {
        let b = self.menu.add_select_one_button(
            shared,
            self.page,
            text,
            series.value(),
            pos,
            size,
            index as i32,
        );
        self.menu.add_sub(self.page, b);
        b
    }

    fn slider(&mut self, shared: &MenuShared, text: &str, y: f32, id: SliderId) -> ObjectId {
        let s = self.menu.add_horizontal_slider(
            shared,
            self.page,
            &Self::tr(shared, text),
            Vec2::new(192.0, y),
            Vec2::new(340.0, 30.0),
            id,
        );
        self.menu.add_sub(self.page, s);
        s
    }

    fn link(&mut self, id: ObjectId, dir: usize, to: ObjectId) {
        self.menu.object_mut(id).next_selectable[dir] = Some(to);
    }

    #[allow(clippy::too_many_lines)]
    fn build(&mut self, shared: &mut MenuShared) -> OptionsScreen {
        const LEFT: usize = 0;
        const UP: usize = 1;
        const RIGHT: usize = 2;
        const DOWN: usize = 3;
        let language = shared.options.language;
        let vector = Vec2::new(200.0, 620.0);
        let zero = Vec2::new(0.0, 100.0);
        let vector2 = Vec2::new(-60.0, -30.0);
        let slot_width = OptionsMenu::save_slot_button_width(language);
        self.label(
            shared,
            &Self::tr(shared, "SCREEN"),
            vector + Vec2::new(20.0, 40.0),
            Vec2::new(100.0, 30.0),
        );
        let fs_text = if shared.options.full_screen() {
            "WINDOWED"
        } else {
            "FULLSCREEN"
        };
        let full_screen = self.menu.add_custom_message_button(
            shared,
            self.page,
            &Self::tr(shared, fs_text),
            fs_text,
            vector,
            Vec2::new(150.0, 30.0),
            "Toggle Fullscreen",
        );
        self.menu.add_sub(self.page, full_screen);
        let res: Vec<ObjectId> = (0..Options::SCREEN_RESOLUTIONS.len())
            .map(|i| {
                self.select_one(
                    shared,
                    &OptionsMenu::resolution_string(i),
                    Series::ScreenRes,
                    vector - Vec2::new(0.0, 40.0 * (i + 1) as f32),
                    Vec2::new(150.0, 30.0),
                    i,
                )
            })
            .collect();
        let wipe_pos = Vec2::new(503.0 + zero.x, 270.0 + zero.y)
            + Vec2::new(221.0 + (slot_width - 100.0) / 2.0, 207.0);
        let wipe_text = Self::tr(shared, "RESET PROGRESS").replace("<LINE>", "\r\n");
        let wipe = self.menu.add_hold_button(
            shared,
            self.page,
            &wipe_text,
            "RESET PROGRESS",
            wipe_pos,
            400.0,
        );
        self.menu.add_sub(self.page, wipe);
        let warning = Self::tr(
            shared,
            "WARNING!<LINE>This will reset all progress in the selected save slot,<LINE>including map exploration. Unlocked arenas and<LINE>sandbox items are retained.",
        )
        .replace("<LINE>", "\r\n");
        let reset_warning_text = self.label(
            shared,
            &warning,
            wipe_pos + Vec2::new(0.0, -140.0) + vector2,
            Vec2::new(200.0, 30.0),
        );
        let back = self.simple(shared, "BACK", "BACK", Vec2::new(200.0, 50.0));
        let credits = self.simple(shared, "CREDITS", "CREDITS", Vec2::new(1056.0, 50.0));
        self.menu.back_object = Some(back);
        let mut vector3 = wipe_pos + Vec2::new(-200.0, 40.0 * 2.0 / 2.0 - 15.0);
        let vector4 = Vec2::new(891.0, vector.y);
        self.label(
            shared,
            &Self::tr(shared, "LANGUAGE"),
            vector4 + Vec2::new(0.0, 40.0),
            Vec2::new(310.0, 30.0),
        );
        let languages: Vec<ObjectId> = OptionsMenu::LANGUAGE_ORDER
            .iter()
            .enumerate()
            .map(|(j, l)| {
                let n = j as f32 * 0.5;
                let text = Self::tr(shared, &l.value().to_uppercase());
                self.select_one(
                    shared,
                    &text,
                    Series::Language,
                    vector4 + Vec2::new(55.0 + n.fract() * 220.0, n.floor() * -40.0),
                    Vec2::new(100.0, 30.0),
                    j,
                )
            })
            .collect();
        self.label(
            shared,
            &Self::tr(shared, "SAVE FILES"),
            vector3 + Vec2::new(65.0 - (slot_width - 100.0) / 2.0, 58.0),
            Vec2::new(122.0, 30.0),
        );
        vector3.x -= slot_width - 100.0;
        let slot_text = Self::tr(shared, "SAVE SLOT");
        let slots: Vec<ObjectId> = (0..3)
            .map(|k| {
                self.select_one(
                    shared,
                    &format!("{slot_text} {}", k + 1),
                    Series::SaveSlot,
                    vector3 + Vec2::new(0.0, -(k as f32) * 40.0),
                    Vec2::new(slot_width, 30.0),
                    k,
                )
            })
            .collect();
        let backups = self.menu.add_simple_button(
            shared,
            self.page,
            &Self::tr(shared, "BACKUPS"),
            "BACKUPS",
            vector3 + Vec2::new(0.0, -3.0 * 40.0),
            Vec2::new(slot_width, 30.0),
        );
        self.menu.add_sub(self.page, backups);
        let music = self.slider(shared, "Music", 235.0, SliderId::MusicVol);
        let arena = self.slider(shared, "Arena Music", 195.0, SliderId::ArenaMusicVolume);
        let sound = self.slider(shared, "Sound Effects", 155.0, SliderId::SfxVol);
        let controls = self.menu.add_controls_button(
            shared,
            self.page,
            Vec2::new(1056.0, 169.0),
            &Self::tr(shared, "Input Settings"),
        );
        self.menu.add_sub(self.page, controls);
        let vector5 = Vec2::new(192.0, 235.0) + Vec2::new(0.0, 60.0);
        let vibration = self.menu.add_check_box(
            shared,
            self.page,
            Vec2::new(192.0 + 340.0, vector5.y),
            65.0,
            &Self::tr(shared, "vibration"),
            "VIBRATION",
            true,
        );
        self.menu.add_sub(self.page, vibration);
        // `InitMiscCheckboxes` with More Slugcats off: speedrun validation only.
        let validation = self.menu.add_check_box(
            shared,
            self.page,
            Vec2::ZERO,
            -40.0,
            &Self::tr(shared, "Speedrun Validation"),
            "VALIDATION",
            false,
        );
        let width = self
            .menu
            .check_box_label(validation)
            .map_or(0.0, |l| self.menu.label_text_width(l));
        let o = self.menu.object_mut(validation);
        o.pos = Vec2::new(1126.0 - width, 259.0);
        o.last_pos = o.pos;
        self.menu.add_sub(self.page, validation);

        // The port's importer, between BACK and CREDITS.
        let import_text = Self::tr(shared, "IMPORT SAVE FROM RAIN WORLD");
        let import = self.menu.add_simple_button(
            shared,
            self.page,
            &import_text,
            OptionsMenu::IMPORT,
            Vec2::new(683.0 - 110.0, 50.0),
            Vec2::new(220.0, 30.0),
        );
        self.menu.add_sub(self.page, import);

        let last_res = *res.last().expect("five resolutions");
        // Left.
        self.link(full_screen, LEFT, full_screen);
        for &r in &res {
            self.link(r, LEFT, r);
        }
        self.link(vibration, LEFT, last_res);
        self.link(back, LEFT, back);
        for &s in &slots {
            self.link(s, LEFT, full_screen);
        }
        self.link(backups, LEFT, full_screen);
        self.link(wipe, LEFT, slots[0]);
        for (j, &l) in languages.iter().enumerate() {
            let to = if j % 2 == 0 { wipe } else { languages[j - 1] };
            self.link(l, LEFT, to);
        }
        self.link(validation, LEFT, music);
        self.link(controls, LEFT, music);
        self.link(credits, LEFT, import);
        self.link(import, LEFT, back);
        // Right.
        self.link(full_screen, RIGHT, slots[0]);
        for (i, &r) in res.iter().enumerate() {
            self.link(
                r,
                RIGHT,
                if i >= res.len() - 2 {
                    vibration
                } else {
                    slots[0]
                },
            );
        }
        self.link(vibration, RIGHT, validation);
        self.link(back, RIGHT, import);
        self.link(import, RIGHT, credits);
        for &s in &slots {
            self.link(s, RIGHT, wipe);
        }
        self.link(backups, RIGHT, wipe);
        self.link(wipe, RIGHT, languages[0]);
        for (j, &l) in languages.iter().enumerate() {
            let to = if j % 2 == 1 || j == languages.len() - 1 {
                l
            } else {
                languages[j + 1]
            };
            self.link(l, RIGHT, to);
        }
        self.link(validation, RIGHT, validation);
        self.link(controls, RIGHT, controls);
        self.link(credits, RIGHT, credits);
        // Up.
        self.link(full_screen, UP, full_screen);
        for (i, &r) in res.iter().enumerate() {
            self.link(r, UP, if i == 0 { full_screen } else { res[i - 1] });
        }
        self.link(vibration, UP, backups);
        self.link(music, UP, vibration);
        self.link(arena, UP, music);
        self.link(sound, UP, arena);
        self.link(back, UP, sound);
        self.link(import, UP, sound);
        for (k, &s) in slots.iter().enumerate() {
            self.link(s, UP, if k == 0 { s } else { slots[k - 1] });
        }
        self.link(backups, UP, slots[2]);
        self.link(wipe, UP, wipe);
        for (j, &l) in languages.iter().enumerate() {
            self.link(l, UP, if j < 2 { l } else { languages[j - 2] });
        }
        self.link(validation, UP, *languages.last().expect("ten languages"));
        self.link(controls, UP, validation);
        self.link(credits, UP, controls);
        // Down.
        self.link(full_screen, DOWN, res[0]);
        for (i, &r) in res.iter().enumerate() {
            self.link(r, DOWN, res.get(i + 1).copied().unwrap_or(music));
        }
        self.link(vibration, DOWN, music);
        self.link(music, DOWN, arena);
        self.link(arena, DOWN, sound);
        self.link(sound, DOWN, back);
        self.link(back, DOWN, back);
        self.link(import, DOWN, import);
        for (k, &s) in slots.iter().enumerate() {
            self.link(s, DOWN, slots.get(k + 1).copied().unwrap_or(backups));
        }
        self.link(backups, DOWN, vibration);
        self.link(wipe, DOWN, vibration);
        for (j, &l) in languages.iter().enumerate() {
            self.link(l, DOWN, languages.get(j + 2).copied().unwrap_or(validation));
        }
        self.link(validation, DOWN, controls);
        self.link(controls, DOWN, credits);
        self.link(credits, DOWN, credits);

        let [w, h] = shared.screen_size;
        let version = self.label(
            shared,
            "v1.11.8",
            Vec2::new((1366.0 - w) / 2.0 + 20.0, h - 30.0),
            Vec2::new(200.0, 20.0),
        );
        let width = self.menu.label_text_width(version);
        self.menu.object_mut(version).size.x = width;
        OptionsScreen {
            resolution_dirty: false,
            language_dirty: false,
            back_button: back,
            credits_button: credits,
            save_backups_button: backups,
            import_button: import,
            wipe_save_button: wipe,
            controls_button: controls,
            reset_warning_text,
            validation_checkbox: validation,
            vibration_checkbox: vibration,
            vibration: shared.options.vibration,
            validation: shared.options.validation,
            arena_music_slider: arena,
            reset_warning_text_counter: 0,
            reset_warning_text_alpha: 0.0,
            wrong_fullscreen_setting_counter: 0,
            last_pause_button: false,
            exiting: false,
            requests: Vec::new(),
            dialog: None,
        }
    }
}

impl OptionsScreen {
    fn tr(cx: &MenuCx<'_>, text: &str) -> String {
        cx.shared.translator.translate(text).to_owned()
    }

    /// `OptionsMenu.Update` after the base.
    fn update(&mut self, menu: &mut Menu, cx: &mut MenuCx<'_>) {
        cx.shared.options.vibration = self.vibration;
        cx.shared.options.validation = self.validation;
        let options = &cx.shared.options;
        if let Some(b) = menu.behavior_mut(self.arena_music_slider) {
            b.greyed_out = options.music_volume == 0.0;
        }
        let no_switch = cx.requests.upcoming().is_none();
        if self.language_dirty {
            if no_switch {
                cx.requests.switch_to(ProcessId::OptionsMenu);
            }
        } else if self.resolution_dirty {
            self.resolution_dirty = false;
            let size = cx.shared.options.screen_size();
            cx.shared.screen.size = size;
            cx.shared.screen.full_screen = false;
            cx.shared.screen_size = size;
            cx.shared.screen.cursor_visible = true;
            if no_switch {
                cx.requests.switch_to(ProcessId::OptionsMenu);
            }
        } else if cx.shared.screen.full_screen != cx.shared.options.full_screen() {
            self.wrong_fullscreen_setting_counter += 1;
            if self.wrong_fullscreen_setting_counter > 10 {
                let size = cx.shared.options.screen_size();
                let full = cx.shared.options.full_screen();
                cx.shared.screen.size = if full && Options::is_steam_deck_resolution(size) {
                    [1280.0, 800.0]
                } else {
                    size
                };
                cx.shared.screen.full_screen = full;
                cx.shared.screen.cursor_visible = !full;
                if full {
                    cx.shared.screen_size = size;
                }
                self.wrong_fullscreen_setting_counter = 0;
            }
        }
        let pause = cx.shared.input.pause_button();
        if pause && !self.last_pause_button && self.dialog.is_none() {
            self.on_exit(cx);
        }
        self.last_pause_button = pause;
        let filling = menu
            .hold_button(self.wipe_save_button)
            .is_some_and(|h| h.filled > 0.0);
        if filling {
            self.reset_warning_text_counter += 1;
            self.reset_warning_text_alpha = self
                .reset_warning_text_alpha
                .lerp_and_tick(1.0, 0.08, 0.025);
        } else {
            self.reset_warning_text_alpha =
                self.reset_warning_text_alpha.lerp_and_tick(0.0, 0.08, 0.05);
        }
        let phase =
            (self.reset_warning_text_counter as f32 / 40.0 * std::f32::consts::PI * 2.0).sin();
        if let Kind::Label(l) = &menu.object(self.reset_warning_text).kind {
            let node = l.label;
            let label = menu.stage.label_mut(node);
            label.transform.alpha = (0.7 + 0.3 * phase) * self.reset_warning_text_alpha;
            label.color =
                StageColor::rgb(1.0, 0.0, 0.0).lerp(StageColor::WHITE, (0.5 + 0.5 * phase).powi(2));
        }
    }

    /// `OnExit`.
    fn on_exit(&mut self, cx: &mut MenuCx<'_>) {
        if !self.exiting {
            self.exiting = true;
            cx.requests.switch_to(ProcessId::MainMenu);
            cx.play_sound(MenuSoundId::SwitchPageOut);
            self.requests.push(ContextRequest::SaveOptions);
        }
    }

    fn import_slot(cx: &MenuCx<'_>) -> SaveSlot {
        let slot = u8::try_from(cx.shared.options.save_slot.clamp(0, 2)).unwrap_or(0);
        SaveSlot(slot)
    }

    fn show_dialog(&mut self, cx: &MenuCx<'_>, menu: &Menu, text: &str, purpose: DialogPurpose) {
        let size = Dialog::size_for(cx.shared, text);
        let mut dialog = match purpose {
            DialogPurpose::ConfirmImport(_) => Dialog::confirm(cx.shared, text, size),
            DialogPurpose::Report => Dialog::notify(cx.shared, text, size),
        };
        dialog.set_sound_loop(menu.my_sound_loop);
        self.dialog = Some((dialog, purpose));
    }

    /// The dialog's tick, with the input the menu read.
    fn update_dialog(&mut self, cx: &mut MenuCx<'_>, input: crate::UiInput) {
        let Some((dialog, purpose)) = &mut self.dialog else {
            return;
        };
        let Some(answer) = dialog.update(cx, input) else {
            return;
        };
        let purpose = *purpose;
        if let Some((mut dialog, _)) = self.dialog.take() {
            dialog.shut_down(cx.shared);
        }
        if let (DialogPurpose::ConfirmImport(slot), DialogAnswer::Ok) = (purpose, answer) {
            self.requests.push(ContextRequest::Import(slot));
        }
    }

    fn slider_info(cx: &MenuCx<'_>, slider: SliderId) -> String {
        let o = &cx.shared.options;
        let (text, v) = match slider {
            SliderId::MusicVol => ("Music volume:", o.music_volume),
            SliderId::SfxVol => ("Sound effects volume:", o.sound_effects_volume),
            SliderId::ArenaMusicVolume => (
                "Arena mode music volume:",
                o.arena_music_volume * o.music_volume,
            ),
            _ => return String::new(),
        };
        let percent = ((v * 100.0) as i32).clamp(0, 100);
        format!("{} {percent}%", Self::tr(cx, text))
    }
}

impl MenuScreen for OptionsScreen {
    fn signal(&mut self, menu: &mut Menu, cx: &mut MenuCx<'_>, _sender: ObjectId, message: &str) {
        match message {
            "Toggle Fullscreen" => {
                cx.shared.options.windowed = !cx.shared.options.windowed;
                self.resolution_dirty = true;
                cx.play_sound(MenuSoundId::StandardButtonPressed);
            }
            "BACK" => self.on_exit(cx),
            "CREDITS" | "INPUT" | "BACKUPS" => {
                let to = match message {
                    "CREDITS" => ProcessId::Credits,
                    "INPUT" => ProcessId::InputOptions,
                    _ => ProcessId::BackupManager,
                };
                cx.requests.switch_to(to);
                cx.play_sound(MenuSoundId::SwitchPageIn);
                self.requests.push(ContextRequest::SaveOptions);
            }
            "RESET PROGRESS" => {
                if let Err(e) = cx.shared.saves.wipe_all() {
                    log::error!("wiping the save slot: {e}");
                }
                cx.shared.refresh_progression();
                cx.play_sound(MenuSoundId::SwitchPageIn);
                cx.requests.switch_to(ProcessId::OptionsMenu);
            }
            OptionsMenu::IMPORT => {
                cx.play_sound(MenuSoundId::StandardButtonPressed);
                let slot = Self::import_slot(cx);
                let n = slot.0 + 1;
                let text = format!(
                    "Replace save slot {n} with Rain World's save slot {n}?\r\n\
                     The current save slot {n} is moved to a backup first."
                );
                self.show_dialog(cx, menu, &text, DialogPurpose::ConfirmImport(slot));
            }
            _ => {}
        }
    }

    fn freeze(&self) -> bool {
        self.dialog.is_some()
    }

    /// `GetChecked`, from the options as of the last sync.
    fn checked(&self, _menu: &Menu, check_box: ObjectId) -> bool {
        if check_box == self.vibration_checkbox {
            self.vibration
        } else if check_box == self.validation_checkbox {
            self.validation
        } else {
            false
        }
    }

    /// `SetChecked`: toggles; the options take it after the menu's update.
    fn set_checked(&mut self, _menu: &mut Menu, check_box: ObjectId, _checked: bool) {
        if check_box == self.vibration_checkbox {
            self.vibration = !self.vibration;
        } else if check_box == self.validation_checkbox {
            self.validation = !self.validation;
        }
    }

    fn info_text(&self, menu: &Menu, cx: &MenuCx<'_>) -> String {
        let Some(selected) = menu.selected else {
            return String::new();
        };
        if let (Some(SimpleKind::SelectOne { index, .. }), Some(b)) =
            (menu.simple_kind(selected), menu.button(selected))
        {
            return match Series::from_value(&b.signal_text) {
                Some(Series::ScreenRes) => Self::tr(cx, "Change screen resolution"),
                Some(Series::Language) => Self::tr(cx, "Change language"),
                Some(Series::SaveSlot) => {
                    format!("{} {}", Self::tr(cx, "Select save slot"), index + 1)
                }
                None => String::new(),
            };
        }
        if let Some(s) = menu.slider(selected) {
            return Self::slider_info(cx, s.id);
        }
        let text = if selected == self.wipe_save_button {
            "Hold down to wipe your save slot and start over"
        } else if selected == self.controls_button {
            "Configure controls"
        } else if selected == self.back_button {
            "Back to main menu"
        } else if selected == self.credits_button {
            "View credits"
        } else if selected == self.save_backups_button {
            "backups_description"
        } else if selected == self.validation_checkbox {
            "Show information on loading screens that helps verify settings used and the legitimacy of runs."
        } else if selected == self.import_button {
            let n = Self::import_slot(cx).0 + 1;
            return format!("Import save slot {n} from Rain World");
        } else {
            return String::new();
        };
        Self::tr(cx, text)
    }

    fn selected_of_series(&self, cx: &MenuCx<'_>, series: &str) -> i32 {
        let o = &cx.shared.options;
        match Series::from_value(series) {
            Some(Series::Language) => OptionsMenu::LANGUAGE_ORDER
                .iter()
                .position(|&l| l == o.language)
                .map_or(-1, |i| i as i32),
            Some(Series::SaveSlot) => o.save_slot,
            Some(Series::ScreenRes) => o.resolution,
            None => -1,
        }
    }

    fn set_selected_of_series(
        &mut self,
        _menu: &mut Menu,
        cx: &mut MenuCx<'_>,
        series: &str,
        to: i32,
    ) {
        let o = &mut cx.shared.options;
        match Series::from_value(series) {
            Some(Series::Language) => {
                let Some(&language) = usize::try_from(to)
                    .ok()
                    .and_then(|i| OptionsMenu::LANGUAGE_ORDER.get(i))
                else {
                    return;
                };
                if o.language != language && !self.language_dirty {
                    o.language = language;
                    self.language_dirty = true;
                }
            }
            Some(Series::SaveSlot) if o.save_slot != to => {
                o.save_slot = to;
                self.requests.push(ContextRequest::LoadSaveSlot);
            }
            Some(Series::ScreenRes) if o.resolution != to && !self.resolution_dirty => {
                o.resolution = to;
                self.resolution_dirty = true;
            }
            _ => {}
        }
    }

    fn slider_value(&self, cx: &MenuCx<'_>, slider: SliderId) -> f32 {
        let o = &cx.shared.options;
        match slider {
            SliderId::MusicVol => o.music_volume,
            SliderId::SfxVol => o.sound_effects_volume,
            SliderId::ArenaMusicVolume => o.arena_music_volume * o.music_volume,
            _ => 0.0,
        }
    }

    fn set_slider_value(
        &mut self,
        menu: &mut Menu,
        cx: &mut MenuCx<'_>,
        slider: ObjectId,
        value: f32,
    ) {
        let Some(id) = menu.slider(slider).map(|s| s.id) else {
            return;
        };
        let o = &mut cx.shared.options;
        match id {
            SliderId::MusicVol => o.music_volume = value,
            SliderId::SfxVol => o.sound_effects_volume = value,
            SliderId::ArenaMusicVolume => {
                o.arena_music_volume = Menu::inverse_lerp(0.0, o.music_volume, value);
            }
            _ => {}
        }
        menu.selected = Some(slider);
        let text = self.info_text(menu, cx);
        menu.flash_info_text(&text);
        cx.shared.apply_music_volume();
        if cx.shared.options.music_volume > 0.0 {
            if cx.shared.music.is_none() {
                cx.shared.music = cx
                    .shared
                    .new_music_player(Some(ProcessId::OptionsMenu.music_process()));
            }
            if let Some(music) = &mut cx.shared.music
                && music.song.is_none()
            {
                music.menu_requests_song("RW_8 - Sundown", 1.0, 0.7);
            }
        }
    }
}

impl<C: MenuContext> MainLoopProcess<C> for OptionsMenu {
    fn id(&self) -> ProcessId {
        ProcessId::OptionsMenu
    }

    fn update(&mut self, ctx: &mut C, requests: &mut ProcessRequests) {
        let input = ctx.menu().input.tick_input();
        let mut cx = MenuCx {
            shared: ctx.menu(),
            requests,
        };
        self.menu.update(&mut self.screen, &mut cx);
        self.screen.update(&mut self.menu, &mut cx);
        self.screen.update_dialog(&mut cx, input);
        for request in std::mem::take(&mut self.screen.requests) {
            match request {
                ContextRequest::LoadSaveSlot => ctx.load_save_slot(),
                ContextRequest::SaveOptions => ctx.save_options(),
                ContextRequest::Import(slot) => {
                    let n = slot.0 + 1;
                    let text = match ctx.import_save_slot(slot) {
                        Ok(()) => format!("Imported save slot {n} from Rain World."),
                        Err(e) => format!("Could not import save slot {n}:\r\n{e}"),
                    };
                    let cx = MenuCx {
                        shared: ctx.menu(),
                        requests,
                    };
                    self.screen
                        .show_dialog(&cx, &self.menu, &text, DialogPurpose::Report);
                }
            }
        }
    }

    fn graf_update(&mut self, time_stacker: f32, ctx: &mut C) {
        let mouse = Vec2::from(ctx.menu().input.mouse.pos);
        self.menu.graf_update(&self.screen, time_stacker, mouse);
        if let Some((dialog, _)) = &mut self.screen.dialog {
            dialog.graf_update(time_stacker, mouse);
        }
    }

    /// `ShutDownProcess`: the music player stops if the music is off.
    fn shut_down(&mut self, ctx: &mut C) {
        let shared = ctx.menu();
        if let Some((mut dialog, _)) = self.screen.dialog.take() {
            dialog.shut_down(shared);
        }
        self.menu.shut_down(shared);
        if shared.options.music_volume == 0.0 {
            shared.music = None;
        }
    }
}
