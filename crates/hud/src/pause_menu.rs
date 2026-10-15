//! `Menu.PauseMenu` of a story game, base game with Remix off: the dimmed game, the
//! keyboard's control map with the pick-up instructions, and CONTINUE and EXIT, with a
//! confirmation when quitting counts as a loss. The arena and sandbox buttons, the
//! Expedition unlocks and the controller rumble are not ported. Behaviour:
//! `rainworld-re/notes/menu.md` ("PauseMenu").

use rw_menu::menu::{Menu, MenuCx, MenuScreen, ObjectId};
use rw_menu::{Alignment, Font, MenuColor, MenuShared, MenuSoundId, NodeId, ProcessId, Sprite};
use rw_sim::{FloatExt, Vec2};

use crate::controls_page::{ControlMap, IllustrationSource, PageText};
use crate::{CustomFloat, Hud};

/// What the pause menu reads of the game when it opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PauseGame {
    /// `IsStorySession`.
    pub story: bool,
    /// `saveState.cycleNumber`.
    pub cycle_number: i32,
    /// `clock`.
    pub clock: u64,
    /// `RedIsOutOfCycles`.
    pub red_is_out_of_cycles: bool,
}

/// What the menu asks of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseRequest {
    /// `game.ContinuePaused()`: the menu faded out.
    ContinuePaused,
    /// `game.ExitToMenu()`, after which the menu shut down.
    ExitToMenu,
}

/// The pause menu's buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseButton {
    Continue,
    Exit,
    YesExit,
    NoExit,
}

impl PauseButton {
    const ALL: [Self; 4] = [Self::Continue, Self::Exit, Self::YesExit, Self::NoExit];

    /// The signal text.
    #[must_use]
    pub const fn signal(self) -> &'static str {
        match self {
            Self::Continue => "CONTINUE",
            Self::Exit => "EXIT",
            Self::YesExit => "YES_EXIT",
            Self::NoExit => "NO_EXIT",
        }
    }

    /// The display text (translated).
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::Continue => "CONTINUE",
            Self::Exit => "EXIT",
            Self::YesExit => "YES",
            Self::NoExit => "NO",
        }
    }

    #[must_use]
    pub fn from_signal(signal: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.signal() == signal)
    }
}

/// The `PauseMenu` subclass's state the base menu calls back into.
struct PauseScreen {
    story: bool,
    page: ObjectId,
    continue_button: Option<ObjectId>,
    exit_button: Option<ObjectId>,
    confirm_yes_button: Option<ObjectId>,
    confirm_no_button: Option<ObjectId>,
    confirm_message: Option<ObjectId>,
    want_to_continue: bool,
    pause_warning_active: bool,
    /// `moveLeft`: the arena buttons' room (0 in a story game).
    move_left: f32,
    control_map: Option<ControlMap>,
    /// The menu's `blackFade` for the control map.
    black_fade: f32,
    requests: Vec<PauseRequest>,
}

/// The pause menu: a [`Menu`] drawn above the HUD.
pub struct PauseMenu {
    menu: Menu,
    screen: PauseScreen,
    black_sprite: NodeId,
    pub counter: i32,
    pub black_fade: f32,
    pub last_black_fade: f32,
    last_pause_button: bool,
    /// `ShutDownProcess` ran: its sprites are gone, but the game keeps it (and stays
    /// paused) until `ContinuePaused`.
    pub shut_down: bool,
}

impl PauseMenu {
    /// `Button.size`.
    const BUTTON_SIZE: Vec2 = Vec2::new(110.0, 30.0);

    /// `new PauseMenu(manager, game)`: plays `HUD_Pause_Game` and sets the HUD's pause
    /// text.
    pub fn new(shared: &mut MenuShared, game: &PauseGame, hud: Option<&mut Hud>) -> Self {
        let mut menu = Menu::new(ProcessId::PauseMenu, shared);
        let page = menu.add_page(shared, "main", 0);
        let container = menu.container_of(page);
        let [w, h] = shared.screen_size;
        let mut black = Sprite::new("pixel");
        black.color = MenuColor::Black.rgb();
        black.transform.scale_x = 1400.0;
        black.transform.scale_y = 800.0;
        black.transform.x = w / 2.0;
        black.transform.y = h / 2.0;
        black.transform.alpha = 0.5;
        let black_sprite = menu.stage.add_sprite(container, black);
        let control_map = game
            .story
            .then(|| Self::control_map(&mut menu, container, shared));
        let mut screen = PauseScreen {
            story: game.story,
            page,
            continue_button: None,
            exit_button: None,
            confirm_yes_button: None,
            confirm_no_button: None,
            confirm_message: None,
            want_to_continue: false,
            pause_warning_active: false,
            move_left: 0.0,
            control_map,
            black_fade: 0.0,
            requests: Vec::new(),
        };
        screen.spawn_exit_continue_buttons(&mut menu, shared);
        menu.selected = None;
        shared.play_menu_sound(MenuSoundId::HudPauseGame, [0.0, 1.0, 1.0]);
        if let Some(hud) = hud {
            screen.set_pause_text(hud, game, shared);
        }
        Self {
            menu,
            screen,
            black_sprite,
            counter: 0,
            black_fade: 0.0,
            last_black_fade: 0.0,
            last_pause_button: false,
            shut_down: false,
        }
    }

    /// `ContinueAndExitButtonsXPos`.
    #[must_use]
    pub fn continue_and_exit_buttons_x(screen: [f32; 2]) -> f32 {
        screen[0] + (1366.0 - screen[0]) / 2.0
    }

    /// The keyboard's control map at `(screen.x / 3 + 170.2, 380)` on the page.
    fn control_map(menu: &mut Menu, container: NodeId, shared: &mut MenuShared) -> ControlMap {
        let [w, _] = shared.screen_size;
        let page_pos = Vec2::new(
            1.0 / 3.0 - menu.horizontal_move_to_get_centered(),
            1.0 / 3.0,
        );
        let x = (w / 3.0).trunc() + 170.2 + ((1366.0 - w) / 2.0).trunc();
        let font = shared.atlases.font(Font::FONT).cloned();
        let text = PageText {
            font: font.as_ref(),
            translator: &shared.translator,
        };
        let mut source = IllustrationSource {
            assets: &shared.assets,
            atlases: &mut shared.atlases,
        };
        let pos = page_pos + Vec2::new(x, 380.0);
        ControlMap::new(&mut menu.stage, container, pos, &mut source, &text, true)
    }

    #[must_use]
    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    /// The buttons shown: CONTINUE and EXIT, or YES and NO.
    #[must_use]
    pub fn buttons(&self) -> Vec<ObjectId> {
        let s = &self.screen;
        [
            s.continue_button,
            s.exit_button,
            s.confirm_yes_button,
            s.confirm_no_button,
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// The confirmation's label, while it shows.
    #[must_use]
    pub fn confirm_message(&self) -> Option<ObjectId> {
        self.screen.confirm_message
    }

    #[must_use]
    pub fn pause_warning_active(&self) -> bool {
        self.screen.pause_warning_active
    }

    #[must_use]
    pub fn want_to_continue(&self) -> bool {
        self.screen.want_to_continue
    }

    /// `Update`; returns what the game is to do.
    pub fn update(&mut self, cx: &mut MenuCx<'_>) -> Vec<PauseRequest> {
        self.counter += 1;
        let s = &self.screen;
        match (s.story, s.continue_button, s.exit_button) {
            (true, Some(continue_button), Some(exit_button)) => {
                let (want, counter) = (s.want_to_continue, self.counter);
                if let Some(b) = self.menu.behavior_mut(continue_button) {
                    b.greyed_out = want;
                }
                if let Some(b) = self.menu.behavior_mut(exit_button) {
                    b.greyed_out = want || counter < 40;
                }
            }
            _ => {
                for b in self.buttons() {
                    let want = self.screen.want_to_continue;
                    if let Some(b) = self.menu.behavior_mut(b) {
                        b.greyed_out = want;
                    }
                }
            }
        }
        let pause = cx.shared.input.pause_button();
        if pause && !self.last_pause_button && self.counter > 10 {
            self.screen.want_to_continue = true;
            cx.play_sound(MenuSoundId::HudUnpauseGame);
        }
        self.last_pause_button = pause;
        self.last_black_fade = self.black_fade;
        if self.screen.want_to_continue {
            self.black_fade = (self.black_fade - 0.125).max(0.0);
            if self.black_fade <= 0.0 {
                self.screen.requests.push(PauseRequest::ContinuePaused);
            }
        } else {
            self.black_fade = (self.black_fade + 0.0625).min(1.0);
        }
        self.screen.black_fade = self.black_fade;
        self.menu.update(&mut self.screen, cx);
        std::mem::take(&mut self.screen.requests)
    }

    /// The volume the menu leaves the game's microphone groups (but 1) at.
    #[must_use]
    pub fn mic_volume(&self) -> f32 {
        1.0 - self.black_fade * 0.5
    }

    /// The `timeSpeed` the menu draws the game's microphone with.
    #[must_use]
    pub fn mic_time_speed(&self, t: f32) -> f32 {
        1.0 - 0.3 * self.last_black_fade.lerp(self.black_fade, t)
    }

    /// `GrafUpdate`.
    pub fn graf_update(&mut self, t: f32, mouse: Vec2) {
        let fade = self.last_black_fade.lerp(self.black_fade, t).s_curve(0.6);
        self.menu
            .stage
            .sprite_mut(self.black_sprite)
            .transform
            .alpha = fade * 0.25;
        self.menu.graf_update(&self.screen, t, mouse);
    }

    /// `ShutDownProcess`: its sprites leave the stage (the illustrations stay loaded:
    /// only a process switch frees them).
    pub fn shut_down(&mut self) {
        self.shut_down = true;
    }
}

impl PauseScreen {
    /// `SpawnExitContinueButtons`.
    fn spawn_exit_continue_buttons(&mut self, menu: &mut Menu, shared: &MenuShared) {
        for o in [
            self.confirm_yes_button.take(),
            self.confirm_no_button.take(),
            self.confirm_message.take(),
        ]
        .into_iter()
        .flatten()
        {
            menu.remove_sub_object(o);
        }
        let [continue_button, exit_button] =
            self.add_buttons(menu, shared, [PauseButton::Continue, PauseButton::Exit]);
        self.continue_button = Some(continue_button);
        self.exit_button = Some(exit_button);
        menu.selected = Some(continue_button);
    }

    /// `SpawnConfirmButtons`: YES and NO where CONTINUE and EXIT were, NO selected, and
    /// the question to their left.
    fn spawn_confirm_buttons(&mut self, menu: &mut Menu, shared: &MenuShared) {
        for o in [self.continue_button.take(), self.exit_button.take()]
            .into_iter()
            .flatten()
        {
            menu.remove_sub_object(o);
        }
        let [yes, no] = self.add_buttons(menu, shared, [PauseButton::YesExit, PauseButton::NoExit]);
        self.confirm_yes_button = Some(yes);
        self.confirm_no_button = Some(no);
        menu.selected = Some(no);
        let text = shared.translator.translate(
            "Really exit? Note that quitting after 30 seconds into a cycle counts as a loss.",
        );
        let no_pos = menu.object(no).pos;
        let label = menu.add_label(
            shared,
            self.page,
            text,
            no_pos,
            Vec2::new(10.0, 30.0),
            false,
        );
        if let rw_menu::menu::Kind::Label(l) = &menu.object(label).kind {
            let node = l.label;
            menu.stage.label_mut(node).set_alignment(Alignment::Left);
        }
        let width = menu.label_text_width(label);
        let o = menu.object_mut(label);
        o.pos.x -= width + 40.0;
        o.last_pos = o.pos;
        menu.add_sub(self.page, label);
        self.confirm_message = Some(label);
    }

    /// The right button at `ContinueAndExitButtonsXPos - 180.2`, the left 140 further,
    /// both 15 up; up and down stay on each.
    fn add_buttons(
        &self,
        menu: &mut Menu,
        shared: &MenuShared,
        buttons: [PauseButton; 2],
    ) -> [ObjectId; 2] {
        let x = PauseMenu::continue_and_exit_buttons_x(shared.screen_size) - self.move_left;
        // `Mathf.Max(SafeScreenOffset.y, 15)`.
        let y = 15.0;
        [180.2, 320.2]
            .map(|dx| x - dx)
            .iter()
            .zip(buttons)
            .map(|(&bx, b)| {
                let text = shared.translator.translate(b.text()).to_owned();
                let pos = Vec2::new(bx, y);
                let id = menu.add_simple_button(
                    shared,
                    self.page,
                    &text,
                    b.signal(),
                    pos,
                    PauseMenu::BUTTON_SIZE,
                );
                menu.add_sub(self.page, id);
                let o = menu.object_mut(id);
                o.next_selectable[1] = Some(id);
                o.next_selectable[3] = Some(id);
                id
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("two buttons")
    }

    /// The constructor's HUD part: the pause text, warning of a loss when quitting
    /// now would count as one.
    fn set_pause_text(&mut self, hud: &mut Hud, game: &PauseGame, shared: &mut MenuShared) {
        let prompt = &mut hud.text_prompt;
        prompt.paused_mode = true;
        if game.story && game.cycle_number > 0 {
            if game.clock > 200 && game.red_is_out_of_cycles {
                self.pause_warning_active = true;
                prompt.paused_warning_text = true;
                prompt.red_is_out_of_cycles = true;
            } else if game.clock > 1200 {
                self.pause_warning_active = true;
                let warned = &mut shared.progression.warned_about_karma_loss_on_exit;
                if *warned < 4 {
                    prompt.paused_warning_text = true;
                    prompt.red_is_out_of_cycles = game.red_is_out_of_cycles;
                    *warned += 1;
                }
            }
        } else {
            prompt.paused_warning_text = false;
        }
    }
}

impl MenuScreen for PauseScreen {
    fn signal(&mut self, menu: &mut Menu, cx: &mut MenuCx<'_>, _sender: ObjectId, message: &str) {
        let Some(button) = PauseButton::from_signal(message) else {
            return;
        };
        match button {
            PauseButton::Continue => {
                self.want_to_continue = true;
                cx.play_sound(MenuSoundId::HudUnpauseGame);
            }
            PauseButton::Exit => {
                let confirm = self.story && self.pause_warning_active;
                cx.play_sound(if confirm {
                    MenuSoundId::StandardButtonPressed
                } else {
                    MenuSoundId::HudExitGame
                });
                if confirm {
                    self.spawn_confirm_buttons(menu, cx.shared);
                } else {
                    self.requests.push(PauseRequest::ExitToMenu);
                }
            }
            PauseButton::NoExit => {
                cx.play_sound(MenuSoundId::StandardButtonPressed);
                self.spawn_exit_continue_buttons(menu, cx.shared);
            }
            PauseButton::YesExit => {
                cx.play_sound(MenuSoundId::HudExitGame);
                self.requests.push(PauseRequest::ExitToMenu);
            }
        }
    }

    fn page_update(&mut self, _menu: &mut Menu, _page: ObjectId, _cx: &mut MenuCx<'_>) {
        if let Some(map) = &mut self.control_map {
            map.update(self.black_fade, false);
        }
    }

    fn page_graf_update(&self, menu: &mut Menu, _page: ObjectId, t: f32) {
        if let Some(map) = &self.control_map {
            map.draw(&mut menu.stage, t);
        }
    }
}
