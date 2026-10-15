//! Overseers: the abstract AI ([`OverseerAbstractAi`]), and the realized `Overseer`
//! with its `OverseerAI` and the player guide's tutorial. The holograms are not
//! ported yet (docs/DIVERGENCES.md D14). Behaviour:
//! `rainworld-re/notes/creature-model.md`, `overseer.md`.

mod abstract_ai;
mod ai;
mod body;
mod tutorial;
mod update;

pub(crate) use abstract_ai::{AbstractStep, GuideContext, Parent};
pub use abstract_ai::{OverseerAbstractAi, PlayerGuideState, TUTORIAL_ROOMS};
pub use ai::{CommunicationModule, OverseerAi, PlayerConcern, ZipMatrix};
pub use body::{APPENDAGE_SEGMENTS, OverseerBody, OverseerMode};
pub use tutorial::{TutorialBehavior, TutorialMessage};
pub use update::{OverseerColor, OverseerView, PlayerView};
pub(crate) use update::{OverseerEffect, OverseerTick, OverseerUpdate, StoryStats};
