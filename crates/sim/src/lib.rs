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
//! `shortcuts.md`, `water.md` and `sound.md`; known differences from the game are in `docs/divergences/`.
//!
//! Above the room: the campaigns ([`campaign`]), the world with its realized rooms, the
//! room realizer and the rain cycle ([`world`]), the creatures with the world's
//! population and the overseers' abstract AI ([`creature`]), the rocks and
//! spears ([`item`]), and [`RainWorldGame`],
//! which moves the player between rooms through room-exit shortcuts
//! (`world-loading.md`, `room-realizer.md`, `creature-model/`).
//!
//! Mirrors: -

mod bubble;
mod camera;
mod camera_image;
pub mod campaign;
mod chunk;
pub mod color;
mod cosmetic;
pub mod creature;
mod decal;
mod dripping_sound;
mod float_ext;
mod game;
pub mod insect;
pub mod item;
mod light_beam;
mod light_source;
mod lightning;
pub mod map_discovery;
mod noise;
mod object;
mod player;
mod rainbow;
mod room_ext;
pub mod room_script;
mod screen_shake;
mod shared_physics;
pub mod shelter;
mod shortcut;
mod slime_mold;
mod sound;
mod spawn;
mod splash;
mod spot_light;
mod token_stalk;
pub mod trigger;
mod vec2;
mod vec3;
mod water;
mod water_drip;
mod water_sound;
pub mod world;

pub use bubble::{Bubble, BubbleMode};
pub use camera::CameraView;
pub use camera_image::CameraImage;
pub use chunk::{BodyChunk, Contact, DEFAULT_RESTRICT_IN_ROOM_RANGE, PhysicsParams, TerrainImpact};
pub use cosmetic::{
    Cosmetic, CosmeticKind, CosmeticTie, ExplosionSpikes, OverseerEffectSprite, Spark,
    StationaryEffect,
};
pub use creature::{ShortCutColor, WorldCoordinate};
pub use decal::CustomDecal;
pub use dripping_sound::DrippingSound;
pub use float_ext::FloatExt;
pub use game::{
    CameraMove, GameOver, Grabability, HeavyAiThrottle, HudEvent, NewGameWorld, OverseerCall,
    PlayerGlow, PlayerHands, PlayerObserver, PlayerSurroundings, RainWorldGame, Start,
    UtilityReadings,
};
pub use light_beam::LightBeam;
pub use light_source::{LightHardSet, LightSource, LightSourceTick, LightTie};
pub use lightning::{
    Lightning, LightningEnv, LightningFrame, LightningRoom, LightningSource, ThunderSound,
};
pub use map_discovery::{DiscoveryTexture, HudMapLink, MapDiscovery, MapRoom, UnityTexture};
pub use noise::{ImpactCall, ImpactCalls, InGameNoise, NoiseSink, RoomCall};
pub use object::{
    BodyChunkConnection, ConnectionKind, PhysicalObject, TerrainImpactHandler, slugcat,
};
pub use player::{
    Animation, BodyMode, DangerGraspState, DangerGraspStep, GrabState, HEAD, HIPS, INPUT_HISTORY,
    Input, Player, PlayerController, ShelterCheck, SpearOnBack, StomachObject, VesselStep,
};
pub use rainbow::Rainbow;
pub use room_ext::RoomExt;
pub use screen_shake::{CameraShake, ScreenMovement};
pub use shelter::SessionEnd;
pub use shortcut::{
    CreatureVessel, EntranceColor, LobbyVessel, ShortcutHandler, ShortcutsBlinking, VesselOwner,
};
pub use slime_mold::CosmeticSlimeMold;
pub use sound::{
    EmitterId, EmitterPlace, LoopEmitter, PlayedSound, SoundEvent, SoundId, SoundRect, SoundSource,
    Sounds, StaticSoundLoop,
};
pub use spawn::{determine_safe_spawn_tile, place_in_room};
pub use splash::Splash;
pub use spot_light::SpotLight;
pub use token_stalk::{CurveLerp, StalkCoord, TokenStalk};
pub use vec2::{TileExt, Vec2};
pub use vec3::Vec3;
pub use water::{BubbleEmitter, ChunkOwner, SurfaceChunk, Water, WaterObject};
pub use water_drip::{WaterDrip, WaterGlitter};
pub use water_sound::WaterSound;

#[cfg(test)]
mod tests;
