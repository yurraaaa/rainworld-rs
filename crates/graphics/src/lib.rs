//! Procedural creature graphics: the cosmetic simulation the game runs in its
//! `GraphicsModule`s, and a backend-agnostic sprite list for the renderer.
//!
//! Ported: the slugcat ([`PlayerGraphics`]: tail, head, legs, hands, draw offsets,
//! breathing, blinking, sprite frame choice), base game with Remix off; Futile atlas
//! frame data ([`Atlas`]); the overseer's ([`OverseerGraphics`]: mycelia, lens, sprites)
//! and its holograms' ([`HologramGraphics`]); the bat fly's ([`FlyGraphics`]); the
//! lizard's body, legs, tail and head ([`LizardGraphics`]); the shelter door's
//! ([`ShelterDoorSprites`]). Behaviour: `rainworld-re/notes/slugcat-graphics.md` and
//! `sprite-atlases.md`; differences from the game are in `docs/divergences/`.
//!
//! Mirrors: - (crate root)

mod body;
mod core;
mod creature;
mod effects;
mod game;
mod item;
mod player;
mod superstructure;
mod water;

pub use player::elements;

pub use crate::core::atlas::{Atlas, AtlasError, AtlasFrame, Rect, Size};
pub use crate::core::draw::{
    Color, DrawItem, DrawList, ElementMesh, InternalContainer, Layer, Mesh, Shader, SlugcatName,
    Sprite,
};
pub use crate::core::math::{Custom, CustomVec, Mathf, UnityVector3, Vec3, VecToDeg};
pub use body::body_part::{BodyPart, PointConnection};
pub use body::limb::{GripSearch, Limb, LimbMode};
pub use body::tail::{TailAnchor, TailSegment};
pub use creature::fly::graphics::FlyGraphics;
pub use creature::lizard::cosmetics::{
    CosmeticShape, LizardCosmetic, LizardScale, Scales, SpineBumps, SpritesOverlap,
};
pub use creature::lizard::cosmetics_draw::CosmeticDraw;
pub use creature::lizard::draw::{LizardReading, LizardSpineData};
pub use creature::lizard::graphics::{IndividualVariations, LIZARD_BASE_SPRITES, LizardGraphics};
pub use creature::lizard::limb::LizardLimb;
pub use creature::overseer::graphics::{DrawPositions, OverseerGraphics};
pub use creature::overseer::holo_light::HologramLightSprites;
pub use creature::overseer::hologram_draw::{HologramFrame, HologramGraphics};
pub use creature::overseer::hologram_matrix::{HoloLine, HologramMatrix, LensFrame};
pub use creature::overseer::hologram_shape::{HologramLine, PartShape};
pub use effects::cosmetic::{CosmeticSprites, LizardColors, LizardModule, NoLizards};
pub use effects::gameplay::bullet_drip::BulletDripSprites;
pub use effects::gameplay::electric_death::{ElectricDeathSprites, SparkFlashSprites};
pub use effects::gameplay::room_rain::{RainDrawView, RainShaderGlobals, RoomRainSprites};
pub use effects::lights::light_beam::SpotLightSprites;
pub use effects::lights::light_beam::{LightBeamSprites, LightBeamView};
pub use effects::lights::light_fixture::{HolyFireSpriteDraw, SlimeMoldLightSprites};
pub use effects::lights::light_source::{LightSourceSprites, LightSourceView};
pub use effects::lights::quad_grid::QuadGrid;
pub use effects::lights::sun_blocker::SunBlocker;
pub use effects::particles::decal::DecalSprites;
pub use effects::particles::green_sparks::GreenSparkSprites;
pub use effects::particles::insect::{InsectDraw, InsectSprites};
pub use effects::particles::rainbow::{RAINBOW_ELEMENT, RainbowSprites};
pub use effects::particles::sky_dandelion::{DandelionSprites, StalkSprites};
pub use effects::particles::slime_mold::SlimeMoldSprites;
pub use effects::particles::token_stalk::TokenStalkSprites;
pub use effects::particles::zero_g_speck::SpeckSprites;
pub use effects::sky::background_scene::DustPuffSprites;
pub use effects::sky::background_scene::{
    BackgroundSceneSprites, BackgroundSceneView, SceneImageSizes,
};
pub use game::shelter_door::ShelterDoorSprites;
pub use game::shortcut::{
    ExitBehind, ShortcutFrame, ShortcutGraphics, ShortcutPalette, ShortcutRoom, ShortcutVessel,
};
pub use item::weapon::{PEBBLES, PebbleElement, SMALL_SPEAR, WeaponSprites};
pub use player::draw::{SPRITE_COUNT, SPRITE_LAYERS, TAIL_TRIANGLES};
pub use player::graphics::PlayerGraphics;
pub use player::hand::SlugcatHand;
pub use player::looker::ObjectLooker;
pub use superstructure::coral::circuit::{CircuitSprites, MAZE_ELEMENT, MAZE_SIZE};
pub use superstructure::coral::mycelium::WallMyceliaSprites;
pub use superstructure::coral::mycelium::{LongMesh, MyceliumSprites};
pub use superstructure::coral::neuron::{CoralNeuronSprites, NeuronLook};
pub use superstructure::coral::stem::{StemFrame, StemSegmentSprites};
pub use superstructure::electric::{
    FuseSprites, LightRodSprites, ZapCoilSprites, ZapFlashLight, ZapFlashSprites,
};
pub use superstructure::grab::{
    DeepProcessingSprites, DisruptorSpeckSprites, GravityDisruptorSprites,
};
pub use superstructure::projections::projected_circle::{
    CircleSurroundings, ProjectedCircleSprites,
};
pub use superstructure::projections::projector::ProjectedScanLinesSprites;
pub use superstructure::projections::projector::{ProjectionView, ProjectorSprites};
pub use superstructure::projections::star_matrix::StarMatrixSprites;
pub use water::water_drip::{DripPalette, WaterDripSprites};
pub use water::waterfall::WaterFallSprites;

#[cfg(test)]
mod tests;
