//! The front end: Futile's stage, fonts and labels, menu illustrations, the English
//! translator, menu input, the menu framework ([`menu`]) with the intro roll, the main
//! menu, the slugcat select menu and the intro slideshows, the menu microphone, the music player's place, and the process manager that switches
//! between the game and the menus behind a fade. Windowing stays in the app; drawing goes through
//! `rw-render`'s stage. Behaviour: `rainworld-re/notes/menu.md`; differences from the
//! game are in `docs/DIVERGENCES.md`.

mod atlas;
mod color;
mod font;
mod font_test;
mod geometry;
mod initialization;
mod input;
mod intro_roll;
mod label;
mod main_menu;
pub mod menu;
mod process;
mod shared;
mod slideshow;
mod slugcat_select;
mod sound;
mod stage;
mod translator;
mod tree;

pub use atlas::{AtlasLoadError, AtlasManager, Element, TextureSync};
pub use color::{HslColor, MenuColor};
pub use font::{CharInfo, Font, LetterQuad, LetterQuadLine, TextParams};
pub use font_test::FontTest;
pub use geometry::{Matrix, Rect};
pub use initialization::Initialization;
pub use input::{KeyBinding, KeyboardKey, MouseState, UiAction, UiInput, UiInputState};
pub use intro_roll::IntroRoll;
pub use label::{Alignment, Label};
pub use main_menu::{MainMenu, MainMenuButton, UnlockCheatSequence};
pub use process::{
    FadeSprite, MainLoopProcess, ProcessFactory, ProcessId, ProcessManager, ProcessRequests,
    TimeStacker,
};
pub use shared::{Dlcs, MenuContext, MenuShared, Progression, Slugcat, StoryGameInitCondition};
pub use slideshow::{SlideScene, SlideShow, SlideShowId};
pub use slugcat_select::{SlugcatPage, SlugcatSelectMenu, SlugcatSelectScreen};
pub use sound::{LoopId, MenuMic, MenuSound, MenuSoundEvent, MenuSoundId, MenuSoundLoop};
pub use stage::{Container, Node, NodeTransform, Sprite, StageColor};
pub use translator::Translator;
pub use tree::{NodeId, StageTree};
