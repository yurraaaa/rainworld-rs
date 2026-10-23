//! Overseers: the abstract AI ([`OverseerAbstractAi`]), and the realized `Overseer`
//! with its `OverseerAI`, the player guide's tutorial and its holograms (drawn by
//! `rw-graphics`; triggers not ported are in docs/divergences/ D14). Behaviour:
//! `rainworld-re/notes/creature-model/overseers-and-flies.md`, `overseer.md`.
//!
//! Mirrors: Overseer.cs, OverseerAI.cs

mod abstract_ai;
mod ai;
mod body;
mod guide_state;
mod holo_light;
pub(crate) mod holo_light_turn;
pub mod hologram;
pub(crate) mod turn;
mod tutorial;
mod update;
pub(crate) mod world_ai;

pub(crate) use abstract_ai::{AbstractStep, GuideContext, Parent};
pub use abstract_ai::{OverseerAbstractAi, TUTORIAL_ROOMS};
pub use ai::{CommunicationModule, OverseerAi, PlayerConcern, ZipMatrix};
pub use body::{APPENDAGE_SEGMENTS, OverseerBody, OverseerMode};
pub use guide_state::PlayerGuideState;
pub use holo_light::{HoloLightTick, HologramLight, LightSourceStep};
pub use tutorial::{TutorialBehavior, TutorialMessage};
pub use update::{EdibleView, OverseerColor, OverseerView, PlayerView};
pub(crate) use update::{OverseerEffect, OverseerTick, OverseerUpdate, StoryStats};
