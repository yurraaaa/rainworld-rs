//! `MainMenu` of the base game with Remix off: the title, the main-menu depth scene
//! and six buttons. Story goes to the slugcat select menu, Options to the options menu and Exit quits; the others
//! switch to processes the port does not have yet (docs/divergences/ D13).

use rw_render::StageShader;
use rw_sim::Vec2;

use crate::input::UiAction;
use crate::menu::{Kind, Menu, MenuCx, MenuScreen, ObjectId, SceneId};
use crate::process::{MainLoopProcess, ProcessId, ProcessRequests};
use crate::shared::{MenuContext, MenuShared};
use crate::sound::MenuSoundId;
use crate::stage::StageColor;

/// What a main-menu button does (`mainMenuButtonCallbacks`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuButton {
    Story,
    Regions,
    Arena,
    Remix,
    Options,
    Exit,
}

impl MainMenuButton {
    /// The buttons a base game with Remix, More Slugcats and Expedition off adds, in
    /// order.
    pub const ALL: [Self; 6] = [
        Self::Story,
        Self::Regions,
        Self::Arena,
        Self::Remix,
        Self::Options,
        Self::Exit,
    ];

    /// The display text (translated) and the signal text.
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::Story => "STORY",
            Self::Regions => "REGIONS",
            Self::Arena => "ARENA",
            Self::Remix => "REMIX",
            Self::Options => "OPTIONS",
            Self::Exit => "EXIT",
        }
    }
}

/// `MainMenu.UnlockCheatSequence`: up, up, down, down, left, right, left, right, UI
/// submit ("a"), UI cancel ("b"), pause ("+") unlocks the Hunter and every arena token.
#[derive(Debug, Clone, PartialEq)]
pub struct UnlockCheatSequence {
    sequence_state: usize,
    button_down: bool,
    check_unlock: bool,
    should_check_unlock: bool,
}

impl Default for UnlockCheatSequence {
    fn default() -> Self {
        Self {
            sequence_state: 0,
            button_down: false,
            check_unlock: true,
            should_check_unlock: true,
        }
    }
}

impl UnlockCheatSequence {
    const SEQUENCE: &'static [u8] = b"uuddlrlrba+";

    /// `FreezeMenuFuctions`: the menu stops once the sequence is nine in.
    #[must_use]
    pub fn freeze_menu_functions(&self) -> bool {
        self.check_unlock && self.sequence_state >= 9
    }

    pub fn update(&mut self, shared: &mut MenuShared) {
        if !self.check_unlock {
            return;
        }
        if self.should_check_unlock {
            self.should_check_unlock = false;
            if shared.progression.red_unlocked && shared.progression.all_tokens_collected {
                self.check_unlock = false;
            }
        }
        if self.sequence_state >= Self::SEQUENCE.len() {
            log::info!("Cheat code success!");
            shared.unlock_red();
            shared.progression.all_tokens_collected = true;
            self.sequence_state = 0;
            self.check_unlock = false;
            return;
        }
        let input = &shared.input;
        let vertical = input.axis(UiAction::Vertical);
        let horizontal = input.axis(UiAction::Horizontal);
        let key = if vertical > 0.5 {
            Some(b'u')
        } else if vertical < -0.5 {
            Some(b'd')
        } else if horizontal < -0.5 {
            Some(b'l')
        } else if horizontal > 0.5 {
            Some(b'r')
        } else if input.button(UiAction::Submit) {
            Some(b'a')
        } else if input.button(UiAction::Cancel) {
            Some(b'b')
        } else if input.pause_action() {
            Some(b'+')
        } else {
            None
        };
        match key {
            Some(key) => {
                if !self.button_down {
                    if Self::SEQUENCE[self.sequence_state] == key {
                        self.sequence_state += 1;
                    } else {
                        self.sequence_state = 0;
                    }
                }
                self.button_down = true;
            }
            None => self.button_down = false,
        }
    }
}

/// The `MainMenu` subclass's state.
struct MainMenuScreen {
    /// `mainMenuButtons`, with their callbacks.
    buttons: Vec<(ObjectId, MainMenuButton)>,
    region_button: ObjectId,
    unlock_cheat: UnlockCheatSequence,
}

/// The main menu process.
pub struct MainMenu {
    menu: Menu,
    screen: MainMenuScreen,
}

impl MainMenu {
    /// `GetButtonWidth` for English.
    pub const BUTTON_WIDTH: f32 = 110.0;

    /// `new MainMenu(manager, showRegionSpecificBkg)` with no saved game (so never a
    /// region background).
    #[must_use]
    pub fn new(shared: &mut MenuShared) -> Self {
        let mut menu = Menu::new(ProcessId::MainMenu, shared);
        let page = menu.add_page(shared, "main", 0);
        let scene_id = if shared.dlcs.downpour {
            SceneId::MainMenuDownpour
        } else {
            SceneId::MainMenu
        };
        let scene = menu.add_scene(shared, page, scene_id);
        menu.add_sub(page, scene);
        menu.scene = Some(scene);
        // Neither the main-menu scene nor the Downpour one is a landscape with its own
        // gradient values.
        let (dark_alpha, gradients_alpha) = (0.3, 0.5);
        let gradients = menu.add_menu_container(page, Vec2::ZERO, Some(gradients_alpha));
        menu.add_sub(page, gradients);
        menu.gradients_container = Some(gradients);
        let dark =
            menu.add_dark_gradient(gradients, Vec2::new(683.0, 580.0), 600.0, 350.0, dark_alpha);
        menu.add_sub(gradients, dark);
        let pos = Vec2::new(683.0 - Self::BUTTON_WIDTH / 2.0, 0.0);
        let size = Vec2::new(Self::BUTTON_WIDTH, 30.0);
        let mut screen = MainMenuScreen {
            buttons: Vec::new(),
            region_button: ObjectId(0),
            unlock_cheat: UnlockCheatSequence::default(),
        };
        for kind in MainMenuButton::ALL {
            let text = shared.translator.translate(kind.text()).to_owned();
            let button = menu.add_simple_button(shared, page, &text, kind.text(), pos, size);
            if kind == MainMenuButton::Regions {
                screen.region_button = button;
            }
            screen.add_main_menu_button(&mut menu, button, kind);
        }
        for file in ["MainTitleShadow", "MainTitleBevel"] {
            let title =
                menu.add_illustration(shared, page, "", file, Vec2::new(378.0, 440.0), true, false);
            menu.add_sub(page, title);
        }
        let bevel = *menu.object(page).sub_objects.last().expect("just added");
        if let Kind::Illustration(i) = &menu.object(bevel).kind {
            let sprite = menu.stage.sprite_mut(i.sprite);
            sprite.shader = StageShader::MenuText;
            sprite.color = StageColor::rgb(0.0, 1.0, 1.0);
        }
        let subs = menu.object(page).sub_objects.clone();
        for sub in subs {
            if menu.button(sub).is_some() {
                let o = menu.object_mut(sub);
                o.next_selectable[0] = Some(sub);
                o.next_selectable[2] = Some(sub);
            }
        }
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

    /// The main-menu buttons in order.
    #[must_use]
    pub fn buttons(&self) -> Vec<ObjectId> {
        self.screen.buttons.iter().map(|(b, _)| *b).collect()
    }

    #[must_use]
    pub fn unlock_cheat(&self) -> &UnlockCheatSequence {
        &self.screen.unlock_cheat
    }
}

impl MainMenuScreen {
    /// `AddMainMenuButton(button, callback, 0)`: appends the button, then lays out all
    /// of them in columns of eight centred on x = 683 and links their neighbours (up
    /// and down wrap, left and right stay unless there are several columns).
    fn add_main_menu_button(&mut self, menu: &mut Menu, button: ObjectId, kind: MainMenuButton) {
        const PER_COLUMN: usize = 8;
        self.buttons.push((button, kind));
        let page = menu.pages[0];
        menu.add_sub(page, button);
        let count = self.buttons.len();
        let first = menu.object(self.buttons[0].0).size;
        let step_y = first.y + 10.0;
        let step_x = first.x + 10.0;
        let column_height = (PER_COLUMN - 1).min(count - 1) as f32 * step_y;
        let columns = 1.max(((count - 1) as f32 / PER_COLUMN as f32) as usize + 1);
        let mut top = 290.0 + column_height / 2.0;
        let mut left = 683.0 - MainMenu::BUTTON_WIDTH / 2.0;
        if count > 5 {
            top -= (PER_COLUMN.min(count) - 5) as f32 * 10.0;
        }
        if columns > 1 {
            left -= step_x * columns as f32 / 2.0 - first.x / 2.0;
        }
        for i in 0..count {
            let id = self.buttons[i].0;
            let up = if i == 0 { count - 1 } else { i - 1 };
            let down = if i == count - 1 { 0 } else { i + 1 };
            let mut links = [
                Some(id),
                Some(self.buttons[up].0),
                Some(id),
                Some(self.buttons[down].0),
            ];
            if columns > 1 {
                if i >= PER_COLUMN {
                    links[0] = Some(self.buttons[i - PER_COLUMN].0);
                }
                if i + PER_COLUMN < count {
                    links[2] = Some(self.buttons[i + PER_COLUMN].0);
                }
            }
            let o = menu.object_mut(id);
            o.pos.x = left + step_x * (i / PER_COLUMN) as f32;
            o.pos.y = top - step_y * (i % PER_COLUMN) as f32;
            o.next_selectable = links;
        }
    }

    /// `MainMenu.Update` after the base: no popup, options ready, so only Regions can
    /// be greyed (no discovered shelters); then the unlock cheat.
    fn update(&mut self, menu: &mut Menu, shared: &mut MenuShared) {
        for &(button, _) in &self.buttons {
            if let Some(b) = menu.behavior_mut(button) {
                b.greyed_out = false;
            }
        }
        let greyed = !shared.progression.any_discovered_shelters;
        if let Some(b) = menu.behavior_mut(self.region_button) {
            b.greyed_out = greyed;
        }
        self.unlock_cheat.update(shared);
    }

    fn press(button: MainMenuButton, cx: &mut MenuCx<'_>) {
        let target = match button {
            MainMenuButton::Story => {
                cx.play_sound(MenuSoundId::SwitchPageIn);
                cx.requests.switch_to(ProcessId::SlugcatSelect);
                return;
            }
            MainMenuButton::Exit => {
                cx.shared.quit = true;
                return;
            }
            MainMenuButton::Regions => ProcessId::RegionsOverviewScreen,
            MainMenuButton::Arena => ProcessId::MultiplayerMenu,
            MainMenuButton::Remix => ProcessId::ModdingMenu,
            MainMenuButton::Options => ProcessId::OptionsMenu,
        };
        cx.requests.switch_to(target);
        cx.play_sound(MenuSoundId::SwitchPageIn);
    }
}

impl MenuScreen for MainMenuScreen {
    fn signal(&mut self, _menu: &mut Menu, cx: &mut MenuCx<'_>, _sender: ObjectId, message: &str) {
        let pressed: Vec<MainMenuButton> = self
            .buttons
            .iter()
            .filter(|(_, kind)| kind.text() == message)
            .map(|(_, kind)| *kind)
            .collect();
        for kind in pressed {
            Self::press(kind, cx);
        }
    }

    fn freeze(&self) -> bool {
        self.unlock_cheat.freeze_menu_functions()
    }
}

impl<C: MenuContext> MainLoopProcess<C> for MainMenu {
    fn id(&self) -> ProcessId {
        ProcessId::MainMenu
    }

    fn update(&mut self, ctx: &mut C, requests: &mut ProcessRequests) {
        let mut cx = MenuCx {
            shared: ctx.menu(),
            requests,
        };
        self.menu.update(&mut self.screen, &mut cx);
        self.screen.update(&mut self.menu, cx.shared);
    }

    fn graf_update(&mut self, time_stacker: f32, ctx: &mut C) {
        let mouse = Vec2::from(ctx.menu().input.mouse.pos);
        self.menu.graf_update(&self.screen, time_stacker, mouse);
    }

    fn shut_down(&mut self, ctx: &mut C) {
        self.menu.shut_down(ctx.menu());
    }
}
