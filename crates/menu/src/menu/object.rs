//! `MenuObject` and the ported subclasses' state, kept in the menu's arena.

use rw_sim::Vec2;

use crate::color::HslColor;
use crate::sound::LoopId;
use crate::tree::NodeId;

/// An object of a [`Menu`](super::Menu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub usize);

/// `MenuObject` with the fields of `PositionedMenuObject`, `RectangularMenuObject` and
/// `CircularMenuObject` (unused by the kinds that are not those).
#[derive(Debug, Clone)]
pub struct MenuObject {
    pub owner: Option<ObjectId>,
    pub sub_objects: Vec<ObjectId>,
    /// Left, up, right, down.
    pub next_selectable: [Option<ObjectId>; 4],
    pub toggled: bool,
    pub inactive: bool,
    /// `myContainer`: the container this object's sprites go in, else its owner's.
    pub my_container: Option<NodeId>,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub size: Vec2,
    pub last_size: Vec2,
    pub rad: f32,
    pub last_rad: f32,
    pub kind: Kind,
}

/// What an object is.
#[derive(Debug, Clone)]
pub enum Kind {
    Page(PageData),
    Label(LabelData),
    Button(ButtonData),
    HoldButton(HoldButtonData),
    RoundedRect(RoundedRectData),
    Illustration(IllustrationData),
    DarkGradient(DarkGradientData),
    /// `MenuContainer`, or a `GradientsContainer` with its alpha.
    Container {
        gradients_alpha: Option<f32>,
    },
    MouseCursor(CursorData),
    Scene(Box<SceneData>),
    RainEffect(Box<super::rain::RainEffectData>),
}

impl Kind {
    /// A `PositionedMenuObject` (everything but the scene).
    #[must_use]
    pub fn positioned(&self) -> bool {
        !matches!(self, Self::Scene(_) | Self::RainEffect(_))
    }

    /// A `RectangularMenuObject`.
    #[must_use]
    pub fn rectangular(&self) -> bool {
        matches!(
            self,
            Self::Label(_)
                | Self::Button(_)
                | Self::RoundedRect(_)
                | Self::Illustration(_)
                | Self::DarkGradient(_)
        )
    }

    /// `SelectableMenuObject`.
    #[must_use]
    pub fn selectable(&self) -> bool {
        matches!(self, Self::Button(_) | Self::HoldButton(_))
    }
}

/// `Page`.
#[derive(Debug, Clone)]
pub struct PageData {
    pub name: String,
    pub index: usize,
    pub selectables: Vec<ObjectId>,
    pub last_selected: Option<ObjectId>,
    pub mouse_cursor: Option<ObjectId>,
}

/// `MenuLabel`.
#[derive(Debug, Clone)]
pub struct LabelData {
    pub label: NodeId,
}

/// `ButtonBehavior`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ButtonBehavior {
    pub last_col: f32,
    pub col: f32,
    pub bump: bool,
    pub clicked: bool,
    pub size_bump: f32,
    pub extra_size_bump: f32,
    pub last_extra_size_bump: f32,
    pub flash: f32,
    pub last_flash: f32,
    pub flash_bool: bool,
    pub sin: f32,
    pub last_sin: f32,
    pub greyed_out: bool,
}

/// A `ButtonTemplate`: its behaviour, `black`, `rectColor` and the subclass.
#[derive(Debug, Clone)]
pub struct ButtonData {
    pub behav: ButtonBehavior,
    pub black: f32,
    pub rect_color: Option<HslColor>,
    pub signal_text: String,
    pub variant: ButtonVariant,
}

#[derive(Debug, Clone)]
pub enum ButtonVariant {
    /// `SimpleButton`.
    Simple {
        rounded_rect: ObjectId,
        select_rect: ObjectId,
        menu_label: ObjectId,
        label_color: HslColor,
        fade_alpha: f32,
    },
    /// `BigArrowButton`.
    BigArrow {
        rounded_rect: ObjectId,
        select_rect: ObjectId,
        symbol: NodeId,
    },
    /// `CheckBox`.
    CheckBox {
        rounded_rect: ObjectId,
        symbol: NodeId,
        label: ObjectId,
        symbol_half_visible: f32,
        last_symbol_half_visible: f32,
        selectable: bool,
        bright_color: bool,
    },
}

/// `HoldButton`.
#[derive(Debug, Clone)]
pub struct HoldButtonData {
    pub behav: ButtonBehavior,
    pub menu_label: ObjectId,
    pub circles: [NodeId; 5],
    pub filled: f32,
    pub last_filled: f32,
    pub fill_time: f32,
    pub has_signalled: bool,
    pub released_counter: i32,
    pub pulse: f32,
    pub last_pulse: f32,
    pub signal_text: String,
    pub controlled_from_outside: bool,
    pub held: bool,
    pub last_held: bool,
    pub sound_loop: Option<LoopId>,
    pub warning_mode: bool,
}

/// `RoundedRect`.
#[derive(Debug, Clone)]
pub struct RoundedRectData {
    pub add_size: Vec2,
    pub last_add_size: Vec2,
    pub fill_alpha: f32,
    pub last_fill_alpha: f32,
    /// 17 sprites when filled (9 fill, then 4 sides and 4 corners), else 8.
    pub sprites: Vec<NodeId>,
    pub filled: bool,
    pub border_color: Option<HslColor>,
}

impl RoundedRectData {
    pub const MAIN_FILL: usize = 8;

    #[must_use]
    pub fn side(&self, side: usize) -> usize {
        if self.filled { 9 + side } else { side }
    }

    #[must_use]
    pub fn corner(&self, corner: usize) -> usize {
        if self.filled { 13 + corner } else { 4 + corner }
    }
}

/// `MenuIllustration` (flat).
#[derive(Debug, Clone)]
pub struct IllustrationData {
    pub sprite: NodeId,
    pub file_name: String,
    pub alpha: f32,
    pub last_alpha: f32,
    pub set_alpha: Option<f32>,
}

/// `DarkGradient`.
#[derive(Debug, Clone)]
pub struct DarkGradientData {
    pub sprite: NodeId,
    pub alpha: f32,
}

/// `MouseCursor`.
#[derive(Debug, Clone)]
pub struct CursorData {
    pub cursor: NodeId,
    pub shadow: NodeId,
    pub fade: f32,
    pub last_fade: f32,
}

/// `MenuScene.SceneID`s the port builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneId {
    MainMenu,
    MainMenuDownpour,
    SlugcatWhite,
    SlugcatYellow,
    SlugcatRed,
    Empty,
    Intro1Tree,
    Intro2Branch,
    Intro3InTree,
    Intro4Walking,
    Intro5Hunting,
    Intro67RainDrop,
    Intro8Climbing,
    Intro9RainyClimb,
    Intro10Fall,
    Intro105Separation,
    Intro11Drowning,
    Intro12Waking,
    Intro13Alone,
    Intro14Title,
    YellowIntroA,
    YellowIntroB,
}

/// A flat `MenuScene`: an `InteractiveMenuScene` (its idle camera, whose random draws
/// are made although nothing is drawn with it) or a `SlideShowMenuScene` (no camera in
/// flat mode, alphas set by the slideshow's time).
#[derive(Debug, Clone)]
pub struct SceneData {
    pub id: SceneId,
    /// An `InteractiveMenuScene`; else a `SlideShowMenuScene`.
    pub interactive: bool,
    /// `SlideShowMenuScene.displayTime`.
    pub display_time: f32,
    pub flat_illustrations: Vec<ObjectId>,
    pub hidden: bool,
    pub focus: f32,
    pub last_focus: f32,
    pub cam_pos: Vec2,
    pub last_cam_pos: Vec2,
    pub camera_range: f32,
    pub get_to_focus: f32,
    pub cam_vel: Vec2,
    pub cam_get_to: Vec2,
    pub cam_get_to2: Vec2,
    pub idle_cam_speed: f32,
    pub idle_cam_speed_get_to: f32,
    pub mouse_active: f32,
    pub game_pad_active: f32,
    pub mouse_inactive_counter: i32,
    pub game_pad_inactive_counter: i32,
    pub time_since_mouse_on_button_counter: i32,
    pub timer: i32,
}

impl MenuObject {
    #[must_use]
    pub fn new(owner: Option<ObjectId>, pos: Vec2, kind: Kind) -> Self {
        Self {
            owner,
            sub_objects: Vec::new(),
            next_selectable: [None; 4],
            toggled: false,
            inactive: false,
            my_container: None,
            pos,
            last_pos: pos,
            size: Vec2::ZERO,
            last_size: Vec2::ZERO,
            rad: 0.0,
            last_rad: 0.0,
            kind,
        }
    }
}
