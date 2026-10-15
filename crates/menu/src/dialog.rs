//! `Dialog` with `DialogConfirm` (OK, CANCEL) and `DialogNotify` (OK): a menu shown over
//! the current one (`ProcessManager.ShowDialog`), which freezes it until a button is
//! clicked (`StopSideProcess`).

use rw_sim::Vec2;

use crate::font::Font;
use crate::input::UiInput;
use crate::label::Label;
use crate::menu::{Kind, Menu, MenuCx, MenuScreen, ObjectId};
use crate::process::ProcessId;
use crate::shared::MenuShared;
use crate::sound::MenuSoundId;
use crate::stage::{Sprite, StageColor};

/// The button that closed a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogAnswer {
    Ok,
    Cancel,
}

/// A dialog over a menu.
pub struct Dialog {
    menu: Menu,
    screen: DialogScreen,
}

struct DialogScreen {
    answer: Option<DialogAnswer>,
}

impl MenuScreen for DialogScreen {
    fn signal(&mut self, _menu: &mut Menu, _cx: &mut MenuCx<'_>, _sender: ObjectId, message: &str) {
        self.answer = match message {
            "OK" => Some(DialogAnswer::Ok),
            "CANCEL" => Some(DialogAnswer::Cancel),
            _ => self.answer,
        };
    }
}

impl Dialog {
    /// `DialogBoxNotify.CalculateDialogBoxSize(text, dialogUsesWordWrapping: false)`.
    #[must_use]
    pub fn size_for(shared: &MenuShared, text: &str) -> Vec2 {
        let text = text.replace("\r\n", "\n");
        let Some(font) = shared.atlases.font(Font::FONT).cloned() else {
            return Vec2::new(200.0, 120.0);
        };
        let line_height = font.line_height();
        let width = Label::new(font, &text).text_rect().width;
        let lines = text.split('\n').count() as f32;
        Vec2::new(
            (width + 44.0).max(200.0),
            (line_height * lines + 100.0).max(120.0),
        )
    }

    /// `new DialogConfirm(description, size, manager, onOK, onCancel)`.
    #[must_use]
    pub fn confirm(shared: &MenuShared, description: &str, size: Vec2) -> Self {
        let mut d = Self::new(shared, description, size);
        let pos = Self::pos_of(size);
        let y = pos.y + (size.y * 0.04).max(7.0);
        let ok = shared.translator.translate("ok").to_owned();
        let cancel = shared.translator.translate("cancel").to_owned();
        d.add_button(
            shared,
            &ok,
            "OK",
            Vec2::new(pos.x + (size.x * 0.5 - 110.0) * 0.5, y),
        );
        d.add_button(
            shared,
            &cancel,
            "CANCEL",
            Vec2::new(1366.0 - pos.x - (size.x * 0.5 - 110.0) * 0.5 - 110.0, y),
        );
        d
    }

    /// `new DialogNotify(description, size, manager, onOK)` (its `timeOut` is 0).
    #[must_use]
    pub fn notify(shared: &MenuShared, description: &str, size: Vec2) -> Self {
        let mut d = Self::new(shared, description, size);
        let pos = Self::pos_of(size);
        let ok = shared.translator.translate("ok").to_owned();
        d.add_button(
            shared,
            &ok,
            "OK",
            Vec2::new(
                pos.x + (size.x - 110.0) * 0.5,
                pos.y + (size.y * 0.04).max(7.0),
            ),
        );
        d
    }

    /// `Dialog.Init(description, size)`: the page, the darkening, the box and the
    /// description.
    fn new(shared: &MenuShared, description: &str, size: Vec2) -> Self {
        let mut menu = Menu::new(ProcessId::Dialog, shared);
        let page = menu.add_page(shared, "main", 0);
        let pos = Self::pos_of(size);
        let mut dark = Sprite::new("pixel");
        dark.color = StageColor::BLACK;
        dark.anchor = [0.0, 0.0];
        dark.transform.scale_x = 1368.0;
        dark.transform.scale_y = 770.0;
        dark.transform.x = -1.0;
        dark.transform.y = -1.0;
        dark.transform.alpha = 0.75;
        let container = menu.container_of(page);
        menu.stage.add_sprite(container, dark);
        let rect = menu.add_rounded_rect(page, pos, size, true);
        if let Kind::RoundedRect(r) = &mut menu.object_mut(rect).kind {
            r.fill_alpha = 0.95;
        }
        menu.add_sub(page, rect);
        let label = menu.add_label(
            shared,
            page,
            description,
            Vec2::new(pos.x + size.x * 0.07, pos.y + 30.0 + size.y * 0.08),
            Vec2::new(size.x * 0.86, size.y * 0.88 - 30.0),
            false,
        );
        menu.add_sub(page, label);
        Self {
            menu,
            screen: DialogScreen { answer: None },
        }
    }

    fn pos_of(size: Vec2) -> Vec2 {
        Vec2::new((1366.0 - size.x) * 0.5, (768.0 - size.y) * 0.5)
    }

    fn add_button(&mut self, shared: &MenuShared, text: &str, signal: &str, pos: Vec2) {
        let page = self.menu.pages[0];
        let b =
            self.menu
                .add_simple_button(shared, page, text, signal, pos, Vec2::new(110.0, 30.0));
        self.menu.add_sub(page, b);
    }

    /// `ActualShowDialog`: the dialog keeps the menu's background loop.
    pub fn set_sound_loop(&mut self, id: Option<MenuSoundId>) {
        self.menu.my_sound_loop = id;
    }

    /// One tick with the input the menu under it read; the answer once clicked.
    pub fn update(&mut self, cx: &mut MenuCx<'_>, input: UiInput) -> Option<DialogAnswer> {
        self.menu.given_input = Some(input);
        self.menu.update(&mut self.screen, cx);
        self.screen.answer
    }

    pub fn graf_update(&mut self, time_stacker: f32, mouse: Vec2) {
        self.menu.graf_update(&self.screen, time_stacker, mouse);
    }

    #[must_use]
    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    pub fn menu_mut(&mut self) -> &mut Menu {
        &mut self.menu
    }

    /// The description text.
    #[must_use]
    pub fn text(&self) -> &str {
        let page = self.menu.pages[0];
        self.menu
            .object(page)
            .sub_objects
            .iter()
            .find_map(|&o| self.menu.label_text(o))
            .unwrap_or("")
    }

    /// `StopSideProcess` → `ShutDownProcess`.
    pub fn shut_down(&mut self, shared: &mut MenuShared) {
        self.menu.shut_down(shared);
    }
}
