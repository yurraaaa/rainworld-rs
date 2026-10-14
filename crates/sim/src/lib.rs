//! Engine-agnostic simulation: fixed 40 Hz tick, body chunks, player movement. No I/O, no rendering.
//!
//! Ported so far: body-chunk physics on the tile grid (gravity, air drag, flat-tile
//! and slope collision, one-way floors), distance-constrained multi-chunk objects,
//! spawning, and base slugcat movement ([`Player`]: standing, crawling, running,
//! jumping, slopes, one-way floors, horizontal beams, fall stun and death, in-room
//! shortcut pipes). Behaviour is specified in `rainworld-re/notes/physics-core.md`,
//! `slope-collision.md`, `slugcat-movement.md`, `horizontal-beams.md`, `falls.md` and
//! `shortcuts.md`; known differences from the game are in `docs/DIVERGENCES.md`.

mod chunk;
mod object;
mod player;
mod room_ext;
mod shortcut;
mod spawn;
mod vec2;

pub use chunk::{BodyChunk, Contact, DEFAULT_RESTRICT_IN_ROOM_RANGE, PhysicsParams, TerrainImpact};
pub use object::{BodyChunkConnection, ConnectionKind, PhysicalObject, slugcat};
pub use player::{Animation, BodyMode, HEAD, HIPS, INPUT_HISTORY, Input, Player};
pub use shortcut::ShortcutHandler;
pub use spawn::{determine_safe_spawn_tile, place_in_room};
pub use vec2::Vec2;

#[cfg(test)]
mod tests;
