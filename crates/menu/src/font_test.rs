//! The port's M0 test screen (not a game process): the flat main-menu scene as the
//! game's flat mode places it, and labels in `font` and `DisplayFont`, to compare with
//! the game by eye. UI submit switches to the game.

use crate::font::Font;
use crate::input::UiInput;
use crate::label::{Alignment, Label};
use crate::process::{MainLoopProcess, ProcessId, ProcessRequests};
use crate::shared::{MenuContext, MenuShared};
use crate::stage::{Container, Node, NodeTransform, Sprite};

pub struct FontTest {
    stage: Container,
    last_input: UiInput,
}

impl FontTest {
    /// `MenuScene.BuildScene` in flat mode for `SceneID.MainMenu`: `Main Menu - Flat`
    /// from `Scenes/Main Menu`, centred at (683, 384), not crisp.
    pub const SCENE_FOLDER: &'static str = "Scenes/Main Menu";
    pub const SCENE_FLAT: &'static str = "Main Menu - Flat";

    /// The texts shown, each `(font, text, x, y, alignment)`.
    pub const LABELS: [(&'static str, &'static str, f32, f32, Alignment); 6] = [
        (Font::DISPLAY_FONT, "STORY", 683.0, 560.0, Alignment::Center),
        (Font::FONT, "STORY", 683.0, 530.0, Alignment::Center),
        (
            Font::DISPLAY_FONT,
            "THE SURVIVOR\nAVAILABLE",
            683.0,
            200.0,
            Alignment::Center,
        ),
        (
            Font::FONT,
            "Press any button to continue",
            683.0,
            140.0,
            Alignment::Center,
        ),
        (
            Font::FONT,
            "Left aligned: AV To Yo\nsecond line",
            100.0,
            700.0,
            Alignment::Left,
        ),
        (
            Font::DISPLAY_FONT,
            "Right aligned AV",
            1266.0,
            700.0,
            Alignment::Right,
        ),
    ];

    #[must_use]
    pub fn new(menu: &mut MenuShared) -> Self {
        let mut stage = Container::default();
        match menu.atlases.load_illustration(
            &menu.assets,
            Self::SCENE_FOLDER,
            Self::SCENE_FLAT,
            false,
        ) {
            Ok(_) => {
                let mut sprite = Sprite::new(Self::SCENE_FLAT);
                sprite.transform = NodeTransform::at(683.0, 384.0);
                stage.add(Node::Sprite(sprite));
            }
            Err(e) => log::warn!("no {} illustration: {e}", Self::SCENE_FLAT),
        }
        for (font, text, x, y, alignment) in Self::LABELS {
            let Some(font) = menu.atlases.font(font) else {
                continue;
            };
            let mut label = Label::new(font.clone(), menu.translator.translate(text));
            label.set_alignment(alignment);
            label.transform = NodeTransform::at(x, y);
            stage.add(Node::Label(label));
        }
        Self {
            stage,
            last_input: UiInput::default(),
        }
    }

    #[must_use]
    pub fn stage(&self) -> &Container {
        &self.stage
    }
}

impl<C: MenuContext> MainLoopProcess<C> for FontTest {
    fn id(&self) -> ProcessId {
        ProcessId::FontTest
    }

    fn update(&mut self, ctx: &mut C, requests: &mut ProcessRequests) {
        let input = ctx.menu().input.take_tick_input();
        if input.jmp && !self.last_input.jmp {
            requests.switch_to(ProcessId::Game);
        }
        self.last_input = input;
    }

    fn graf_update(&mut self, _time_stacker: f32, _ctx: &mut C) {}

    fn shut_down(&mut self, ctx: &mut C) {
        // `AssetManager.HardCleanFutileAssets` frees the illustration textures.
        ctx.menu().atlases.unload_atlas(Self::SCENE_FLAT);
    }
}
