//! Procedural creature graphics: the cosmetic simulation the game runs in its
//! `GraphicsModule`s, and a backend-agnostic sprite list for the renderer.
//!
//! Ported: the slugcat ([`PlayerGraphics`]: tail, head, legs, hands, draw offsets,
//! breathing, blinking, sprite frame choice), base game with Remix off; Futile atlas
//! frame data ([`Atlas`]). Behaviour: `rainworld-re/notes/slugcat-graphics.md` and
//! `sprite-atlases.md`; differences from the game are in `docs/DIVERGENCES.md`.

mod atlas;
mod body_part;
mod draw;
pub mod elements;
mod hand;
mod limb;
mod math;
mod player_draw;
mod player_graphics;
mod tail;

pub use atlas::{Atlas, AtlasError, AtlasFrame, Rect, Size};
pub use body_part::{BodyPart, PointConnection};
pub use draw::{Color, DrawItem, DrawList, Layer, Mesh, SlugcatName, Sprite};
pub use hand::SlugcatHand;
pub use limb::{GripSearch, Limb, LimbMode};
pub use math::{CustomVec, Mathf, UnityRandom};
pub use player_draw::{SPRITE_COUNT, SPRITE_LAYERS, TAIL_TRIANGLES};
pub use player_graphics::PlayerGraphics;
pub use tail::{TailAnchor, TailSegment};

#[cfg(test)]
mod tests;
