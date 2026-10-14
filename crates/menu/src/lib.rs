//! The front end: Futile's stage, fonts and labels, menu illustrations, the English
//! translator, menu input and the process manager that switches between the game and
//! the menus behind a fade. Windowing stays in the app; drawing goes through
//! `rw-render`'s stage. Behaviour: `rainworld-re/notes/menu.md`; differences from the
//! game are in `docs/DIVERGENCES.md`.

mod atlas;
mod font;
mod font_test;
mod geometry;
mod initialization;
mod input;
mod label;
mod process;
mod shared;
mod stage;
mod translator;

pub use atlas::{AtlasLoadError, AtlasManager, Element, TextureSync};
pub use font::{CharInfo, Font, LetterQuad, LetterQuadLine, TextParams};
pub use font_test::FontTest;
pub use geometry::{Matrix, Rect};
pub use initialization::Initialization;
pub use input::{KeyBinding, KeyboardKey, MouseState, UiAction, UiInput, UiInputState};
pub use label::{Alignment, Label};
pub use process::{
    FadeSprite, MainLoopProcess, ProcessFactory, ProcessId, ProcessManager, ProcessRequests,
    TimeStacker,
};
pub use shared::{MenuContext, MenuShared};
pub use stage::{Container, Node, NodeTransform, Sprite, StageColor};
pub use translator::Translator;
