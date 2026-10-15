//! Engine-agnostic simulation: fixed 40 Hz tick, body chunks, player movement. No I/O, no rendering.
//!
//! Ported so far: body-chunk physics on the tile grid (gravity, air drag, flat-tile
//! and slope collision, one-way floors), distance-constrained multi-chunk objects,
//! spawning, and base slugcat movement ([`Player`]: standing, crawling, running,
//! jumping, slopes, one-way floors, horizontal beams, vertical poles, ledge grabs,
//! corridors, skid, flip, roll, belly slide, rocket jump, fall stun and death, in-room
//! shortcut pipes, swimming, breath and drowning), the room's wavy water surface
//! ([`Water`]) and the sounds they play ([`Sounds`]: `Room.PlaySound` with its random
//! draws, as events for `rw-audio`). Behaviour is specified in `rainworld-re/notes/physics-core.md`,
//! `slope-collision.md`, `slugcat-movement.md`, `horizontal-beams.md`,
//! `vertical-poles.md`, `ledge-grab.md`, `advanced-moves.md`, `corridors.md`, `falls.md`
//! `shortcuts.md`, `water.md` and `sound.md`; known differences from the game are in `docs/DIVERGENCES.md`.
//!
//! Above the room: the campaigns ([`campaign`]), the world with its realized rooms, the
//! room realizer and the rain cycle ([`world`]), the creatures with the world's
//! population and the overseers' abstract AI ([`creature`]), and [`RainWorldGame`],
//! which moves the player between rooms through room-exit shortcuts
//! (`world-loading.md`, `room-realizer.md`, `creature-model.md`).

mod camera;
pub mod campaign;
mod chunk;
pub mod creature;
mod float_ext;
mod game;
mod object;
mod player;
mod room_ext;
pub mod room_script;
mod shortcut;
mod sound;
mod spawn;
mod vec2;
mod vec3;
mod water;
pub mod world;

pub use camera::CameraView;
pub use chunk::{BodyChunk, Contact, DEFAULT_RESTRICT_IN_ROOM_RANGE, PhysicsParams, TerrainImpact};
pub use creature::WorldCoordinate;
pub use float_ext::FloatExt;
pub use game::{
    CameraMove, HudEvent, NewGameWorld, OverseerCall, OverseerCallRecord, PlayerObserver,
    RainWorldGame, Start,
};
pub use object::{
    BodyChunkConnection, ConnectionKind, PhysicalObject, TerrainImpactHandler, slugcat,
};
pub use player::{
    Animation, BodyMode, HEAD, HIPS, INPUT_HISTORY, Input, Player, PlayerController, VesselStep,
};
pub use shortcut::{EntranceColor, LobbyVessel, ShortcutHandler, ShortcutsBlinking};
pub use sound::{EmitterId, PlayedSound, SoundEvent, SoundId, SoundSource, Sounds};
pub use spawn::{determine_safe_spawn_tile, place_in_room};
pub use vec2::{TileExt, Vec2};
pub use vec3::Vec3;
pub use water::Water;

#[cfg(test)]
mod tests;
