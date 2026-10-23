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

pub mod creature;
mod effects;
mod game;
pub mod item;
mod math;
mod physics;
mod player;
mod sound;
mod superstructure;
mod vines;
mod water;
pub mod world;

pub use effects::particles::insect;
pub use game::{campaign, map_discovery, room_script, shelter, trigger};
pub use math::color;

pub use crate::effects::gameplay::anti_gravity::{AntiGravity, BrokenAntiGravity, BrokenTick};
pub use crate::effects::gameplay::border_push_back::BorderPushBack;
pub use crate::effects::gameplay::death_fall::DeathFall;
pub use crate::effects::gameplay::electric_death::{
    ElectricDeath, LightFlash, SparkFlash, Strike, WallSpark,
};
pub use creature::{ShortCutColor, WorldCoordinate};
pub use effects::cosmetic::{
    Cosmetic, CosmeticKind, CosmeticTie, ExplosionSpikes, OverseerEffectSprite, Spark,
    StationaryEffect,
};
pub use effects::lights::light_beam::LightBeam;
pub use effects::lights::light_beam::SpotLight;
pub use effects::lights::light_fixture::{HolyFire, HolyFireSprite, Redlight, SlimeMoldLight};
pub use effects::lights::light_source::{
    LightElement, LightHardSet, LightSource, LightSourceTick, LightTie,
};
pub use effects::particles::decal::CustomDecal;
pub use effects::particles::green_sparks::{GreenSpark, GreenSparks, SparkCx};
pub use effects::particles::rainbow::Rainbow;
pub use effects::particles::room_particles::RoomParticles;
pub use effects::particles::sky_dandelion::{
    ChunkView, DandelionPatch, DandelionStalk, MadeDandelion, SkyDandelion, SkyDandelions,
};
pub use effects::particles::slime_mold::CosmeticSlimeMold;
pub use effects::particles::token_stalk::{CurveLerp, StalkCoord, TokenStalk};
pub use effects::particles::zero_g_speck::{
    BlinkState, SpeckCx, SpeckKind, SpeckType, SporeState, ZeroGSpeck,
};
pub use effects::sky::above_clouds_view::AboveCloudsContext;
pub use effects::sky::above_clouds_view::DistantLightning;
pub use effects::sky::background_scene::{
    AboveCloudsParams, BackgroundScene, CloudData, ElementKind, SCREEN_SIZE, SceneElement,
    SceneImages, SceneKind, SceneParams, SkyTime,
};
pub use effects::sky::lightning::{
    Lightning, LightningEnv, LightningFrame, LightningRoom, LightningSource, ThunderSound,
};
pub use effects::sky::roof_top_view::{DustPuff, DustpuffSpawner};
pub use game::camera::CameraView;
pub use game::camera_image::CameraImage;
pub use game::map_discovery::{DiscoveryTexture, HudMapLink, MapDiscovery, MapRoom, UnityTexture};
pub use game::screen_shake::{CameraShake, ScreenMovement};
pub use game::shelter::SessionEnd;
pub use game::shortcut::{
    CreatureVessel, EntranceColor, LobbyVessel, ShortcutHandler, ShortcutsBlinking, VesselOwner,
};
pub use game::shortcut_helper::{ObjectPusher, ShortcutHelper, ShortcutPusher};
pub use game::spawn::{determine_safe_spawn_tile, place_in_room};
pub use game::{
    CameraMove, DevRainControl, GameOver, Grabability, HeavyAiThrottle, HudEvent, LookTarget,
    NewGameWorld, OverseerCall, PlayerGlow, PlayerHands, PlayerObserver, PlayerSurroundings,
    RainWorldGame, Start, UtilityReadings,
};
pub use math::float_ext::FloatExt;
pub use math::vec2::{TileExt, Vec2};
pub use math::vec3::Vec3;
pub use physics::chunk::{
    BodyChunk, Contact, DEFAULT_RESTRICT_IN_ROOM_RANGE, PhysicsParams, TerrainImpact,
};
pub use physics::object::{
    BodyChunkConnection, ConnectionKind, PhysicalObject, TerrainImpactHandler, slugcat,
};
pub use physics::room_ext::RoomExt;
pub use player::{
    Animation, BodyMode, CircuitCollision, CircuitExplosion, DangerGraspState, DangerGraspStep,
    GrabState, HEAD, HIPS, INPUT_HISTORY, Input, Player, PlayerController, ShelterCheck,
    SpearOnBack, StomachObject, SwimBit, VesselStep,
};
pub use sound::dripping_sound::DrippingSound;
pub use sound::noise::{ImpactCall, ImpactCalls, InGameNoise, NoiseSink, NoiseSource, RoomCall};
pub use sound::water_sound::WaterSound;
pub use sound::{
    EmitterId, EmitterPlace, LoopEmitter, PlayedSound, SoundEvent, SoundId, SoundRect, SoundSource,
    Sounds, StaticSoundLoop,
};
pub use superstructure::coral::circuit::{
    BitCx, BitRef, CircuitBit, CircuitCx, CircuitPlace, CoralCircuit, PushChunk,
};
pub use superstructure::coral::circuit_connector::{CircuitConnector, ConnectorAnchor};
pub use superstructure::coral::circuit_crab::{CircuitCrab, CrabLeg};
pub use superstructure::coral::mycelium::{
    MyceliaConnection, MyceliaCx, Mycelium, MyceliumEnv, MyceliumId, MyceliumOwner, MyceliumPool,
    OwnerKey, PointOwner, SystemId, SystemState,
};
pub use superstructure::coral::neuron::{CoralNeuron, NeuronCx, NeuronGeometry, NewNeuron};
pub use superstructure::coral::neuron_spark::NeuronSpark;
pub use superstructure::coral::stem::{
    CoralStem, NewStem, SegmentHang, SegmentRef, StemCx, StemSegment, StemTick,
};
pub use superstructure::coral::system::CoralNeuronSystem;
pub use superstructure::coral::wall_mycelia::{WallMycelia, WallPlace, WallRoots};
pub use superstructure::electric::fuses::{FuseSetup, SuperStructureFuses};
pub use superstructure::electric::light_rod::{LightVessel, ROD_COLOR, RodCamera, SsLightRod};
pub use superstructure::electric::room_fixtures::RoomFixtures;
pub use superstructure::electric::zap_coil::{CoilEnv, ZapCoil, ZapCoilLight, ZapFlash};
pub use superstructure::grab::deep_processing::{DeepProcessing, DeepProcessingLight};
pub use superstructure::grab::gravity_disruptor::{
    DisruptorSpeck, GravityDisruptor, SPECKS, STREAKS,
};
pub use superstructure::projections::projected_circle::{CircleOwner, ProjectedCircle};
pub use superstructure::projections::projector::{
    Cursor, Glyph, GlyphMatrix, PartKind, ProjectorFrame, ProjectorPart, ScanLine, SingleGlyph,
    SuperStructureProjector,
};
pub use superstructure::projections::room_projections::RoomProjections;
pub use superstructure::projections::scan_lines::{ProjectedScanLine, ProjectedScanLines};
pub use superstructure::projections::star_matrix::{Star, StarMatrix};
pub use vines::{
    ClimbableVine, ClimbableVinesSystem, PlacedVines, RoomVines, RoomVinesView, VineAccess,
    VineClimber, VineHost, VineHostMut, VineKind, VinePosition, VineRef,
};
pub use water::bubble::{Bubble, BubbleMode};
pub use water::splash::Splash;
pub use water::water_drip::{WaterDrip, WaterGlitter};
pub use water::waterfall::{FallBubble, FallObject, FallSpec, WaterFall};
pub use water::{BubbleEmitter, ChunkOwner, SurfaceChunk, Water, WaterObject};

#[cfg(test)]
mod tests;
