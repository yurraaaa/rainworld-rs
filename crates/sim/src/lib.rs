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

mod above_clouds_view;
mod anti_gravity;
mod background_scene;
mod border_push_back;
mod bubble;
mod camera;
mod camera_image;
pub mod campaign;
mod chunk;
pub mod color;
mod coral_system;
mod cosmetic;
pub mod creature;
mod decal;
mod deep_processing;
mod distant_lightning;
mod dripping_sound;
mod dust_puff;
mod electric_death;
mod float_ext;
mod game;
mod gravity_disruptor;
mod green_sparks;
pub mod insect;
pub mod item;
mod light_beam;
mod light_fixture;
mod light_source;
mod lightning;
pub mod map_discovery;
mod mycelium;
mod neuron_spark;
mod noise;
mod object;
mod player;
mod projected_scan_lines;
mod projector;
mod rainbow;
mod roof_top_view;
mod room_ext;
mod room_fixtures;
mod room_particles;
mod room_projections;
pub mod room_script;
mod screen_shake;
mod shared_physics;
pub mod shelter;
mod shortcut;
mod sky_dandelion;
mod slime_mold;
mod sound;
mod spawn;
mod splash;
mod spot_light;
mod ss_fuses;
mod ss_light_rod;
mod star_matrix;
mod token_stalk;
pub mod trigger;
mod vec2;
mod vec3;
mod wall_mycelia;
mod water;
mod water_drip;
mod water_sound;
mod waterfall;
pub mod world;
mod zap_coil;
mod zero_g_speck;

pub use above_clouds_view::AboveCloudsContext;
pub use anti_gravity::{AntiGravity, BrokenAntiGravity, BrokenTick};
pub use background_scene::{
    AboveCloudsParams, BackgroundScene, CloudData, ElementKind, SCREEN_SIZE, SceneElement,
    SceneImages, SceneKind, SceneParams, SkyTime,
};
pub use border_push_back::BorderPushBack;
pub use bubble::{Bubble, BubbleMode};
pub use camera::CameraView;
pub use camera_image::CameraImage;
pub use chunk::{BodyChunk, Contact, DEFAULT_RESTRICT_IN_ROOM_RANGE, PhysicsParams, TerrainImpact};
pub use coral_system::CoralNeuronSystem;
pub use cosmetic::{
    Cosmetic, CosmeticKind, CosmeticTie, ExplosionSpikes, OverseerEffectSprite, Spark,
    StationaryEffect,
};
pub use creature::{ShortCutColor, WorldCoordinate};
pub use decal::CustomDecal;
pub use deep_processing::{DeepProcessing, DeepProcessingLight};
pub use distant_lightning::DistantLightning;
pub use dripping_sound::DrippingSound;
pub use dust_puff::{DustPuff, DustpuffSpawner};
pub use electric_death::{ElectricDeath, LightFlash, SparkFlash, Strike, WallSpark};
pub use float_ext::FloatExt;
pub use game::{
    CameraMove, DevRainControl, GameOver, Grabability, HeavyAiThrottle, HudEvent, NewGameWorld,
    OverseerCall, PlayerGlow, PlayerHands, PlayerObserver, PlayerSurroundings, RainWorldGame,
    Start, UtilityReadings,
};
pub use gravity_disruptor::{DisruptorSpeck, GravityDisruptor, SPECKS, STREAKS};
pub use green_sparks::{GreenSpark, GreenSparks, SparkCx};
pub use light_beam::LightBeam;
pub use light_fixture::{HolyFire, HolyFireSprite, Redlight, SlimeMoldLight};
pub use light_source::{LightElement, LightHardSet, LightSource, LightSourceTick, LightTie};
pub use lightning::{
    Lightning, LightningEnv, LightningFrame, LightningRoom, LightningSource, ThunderSound,
};
pub use map_discovery::{DiscoveryTexture, HudMapLink, MapDiscovery, MapRoom, UnityTexture};
pub use mycelium::{
    MyceliaConnection, MyceliaCx, Mycelium, MyceliumEnv, MyceliumId, MyceliumOwner, MyceliumPool,
    OwnerKey, PointOwner, SystemId, SystemState,
};
pub use neuron_spark::NeuronSpark;
pub use noise::{ImpactCall, ImpactCalls, InGameNoise, NoiseSink, RoomCall};
pub use object::{
    BodyChunkConnection, ConnectionKind, PhysicalObject, TerrainImpactHandler, slugcat,
};
pub use player::{
    Animation, BodyMode, DangerGraspState, DangerGraspStep, GrabState, HEAD, HIPS, INPUT_HISTORY,
    Input, Player, PlayerController, ShelterCheck, SpearOnBack, StomachObject, VesselStep,
};
pub use projected_scan_lines::{ProjectedScanLine, ProjectedScanLines};
pub use projector::{
    Cursor, Glyph, GlyphMatrix, PartKind, ProjectorFrame, ProjectorPart, ScanLine, SingleGlyph,
    SuperStructureProjector,
};
pub use rainbow::Rainbow;
pub use room_ext::RoomExt;
pub use room_fixtures::RoomFixtures;
pub use room_particles::RoomParticles;
pub use room_projections::RoomProjections;
pub use screen_shake::{CameraShake, ScreenMovement};
pub use shelter::SessionEnd;
pub use shortcut::{
    CreatureVessel, EntranceColor, LobbyVessel, ShortcutHandler, ShortcutsBlinking, VesselOwner,
};
pub use sky_dandelion::{
    ChunkView, DandelionPatch, DandelionStalk, MadeDandelion, SkyDandelion, SkyDandelions,
};
pub use slime_mold::CosmeticSlimeMold;
pub use sound::{
    EmitterId, EmitterPlace, LoopEmitter, PlayedSound, SoundEvent, SoundId, SoundRect, SoundSource,
    Sounds, StaticSoundLoop,
};
pub use spawn::{determine_safe_spawn_tile, place_in_room};
pub use splash::Splash;
pub use spot_light::SpotLight;
pub use ss_fuses::{FuseSetup, SuperStructureFuses};
pub use ss_light_rod::{LightVessel, ROD_COLOR, RodCamera, SsLightRod};
pub use star_matrix::{Star, StarMatrix};
pub use token_stalk::{CurveLerp, StalkCoord, TokenStalk};
pub use vec2::{TileExt, Vec2};
pub use vec3::Vec3;
pub use wall_mycelia::{WallMycelia, WallPlace, WallRoots};
pub use water::{BubbleEmitter, ChunkOwner, SurfaceChunk, Water, WaterObject};
pub use water_drip::{WaterDrip, WaterGlitter};
pub use water_sound::WaterSound;
pub use waterfall::{FallBubble, FallObject, FallSpec, WaterFall};
pub use zap_coil::{CoilEnv, ZapCoil, ZapCoilLight, ZapFlash};
pub use zero_g_speck::{BlinkState, SpeckCx, SpeckKind, SpeckType, SporeState, ZeroGSpeck};

#[cfg(test)]
mod tests;
