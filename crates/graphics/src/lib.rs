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

mod atlas;
mod background_scene;
mod body_part;
mod cosmetic_graphics;
mod decal;
mod draw;
mod dust_puff;
mod electric_death;
pub mod elements;
mod fly_graphics;
mod green_spark;
mod hand;
mod holo_light;
mod hologram_draw;
mod hologram_matrix;
mod hologram_shape;
mod insect_graphics;
mod light_beam;
mod light_fixture;
mod light_source;
mod limb;
mod lizard_cosmetics;
mod lizard_cosmetics_draw;
mod lizard_draw;
mod lizard_graphics;
mod lizard_limb;
mod looker;
mod math;
mod mycelium;
mod overseer_draw;
mod overseer_graphics;
mod player_draw;
mod player_graphics;
mod projected_scan_lines_draw;
mod projector_draw;
mod quad_grid;
mod rainbow;
mod shelter_door_graphics;
mod shortcut_graphics;
mod sky_dandelion;
mod slime_mold;
mod spot_light;
mod ss_electric;
mod ss_grab;
mod star_matrix_draw;
mod sun_blocker;
mod tail;
mod token_stalk;
mod wall_mycelia_draw;
mod water_drip;
mod water_fall;
mod weapon_graphics;
mod zero_g_speck;

pub use atlas::{Atlas, AtlasError, AtlasFrame, Rect, Size};
pub use background_scene::{BackgroundSceneSprites, BackgroundSceneView, SceneImageSizes};
pub use body_part::{BodyPart, PointConnection};
pub use cosmetic_graphics::{CosmeticSprites, LizardColors, LizardModule, NoLizards};
pub use decal::DecalSprites;
pub use draw::{
    Color, DrawItem, DrawList, ElementMesh, InternalContainer, Layer, Mesh, Shader, SlugcatName,
    Sprite,
};
pub use dust_puff::DustPuffSprites;
pub use electric_death::{ElectricDeathSprites, SparkFlashSprites};
pub use fly_graphics::FlyGraphics;
pub use green_spark::GreenSparkSprites;
pub use hand::SlugcatHand;
pub use holo_light::HologramLightSprites;
pub use hologram_draw::{HologramFrame, HologramGraphics};
pub use hologram_matrix::{HoloLine, HologramMatrix, LensFrame};
pub use hologram_shape::{HologramLine, PartShape};
pub use insect_graphics::{InsectDraw, InsectSprites};
pub use light_beam::{LightBeamSprites, LightBeamView};
pub use light_fixture::{HolyFireSpriteDraw, SlimeMoldLightSprites};
pub use light_source::{LightSourceSprites, LightSourceView};
pub use limb::{GripSearch, Limb, LimbMode};
pub use lizard_cosmetics::{
    CosmeticShape, LizardCosmetic, LizardScale, Scales, SpineBumps, SpritesOverlap,
};
pub use lizard_cosmetics_draw::CosmeticDraw;
pub use lizard_draw::{LizardReading, LizardSpineData};
pub use lizard_graphics::{IndividualVariations, LIZARD_BASE_SPRITES, LizardGraphics};
pub use lizard_limb::LizardLimb;
pub use looker::ObjectLooker;
pub use math::{Custom, CustomVec, Mathf, UnityVector3, Vec3, VecToDeg};
pub use mycelium::{LongMesh, MyceliumSprites};
pub use overseer_graphics::{DrawPositions, OverseerGraphics};
pub use player_draw::{SPRITE_COUNT, SPRITE_LAYERS, TAIL_TRIANGLES};
pub use player_graphics::PlayerGraphics;
pub use projected_scan_lines_draw::ProjectedScanLinesSprites;
pub use projector_draw::{ProjectionView, ProjectorSprites};
pub use quad_grid::QuadGrid;
pub use rainbow::{RAINBOW_ELEMENT, RainbowSprites};
pub use shelter_door_graphics::ShelterDoorSprites;
pub use shortcut_graphics::{
    ExitBehind, ShortcutFrame, ShortcutGraphics, ShortcutPalette, ShortcutRoom, ShortcutVessel,
};
pub use sky_dandelion::{DandelionSprites, StalkSprites};
pub use slime_mold::SlimeMoldSprites;
pub use spot_light::SpotLightSprites;
pub use ss_electric::{
    FuseSprites, LightRodSprites, ZapCoilSprites, ZapFlashLight, ZapFlashSprites,
};
pub use ss_grab::{DeepProcessingSprites, DisruptorSpeckSprites, GravityDisruptorSprites};
pub use star_matrix_draw::StarMatrixSprites;
pub use sun_blocker::SunBlocker;
pub use tail::{TailAnchor, TailSegment};
pub use token_stalk::TokenStalkSprites;
pub use wall_mycelia_draw::WallMyceliaSprites;
pub use water_drip::{DripPalette, WaterDripSprites};
pub use water_fall::WaterFallSprites;
pub use weapon_graphics::{PEBBLES, PebbleElement, SMALL_SPEAR, WeaponSprites};
pub use zero_g_speck::SpeckSprites;

#[cfg(test)]
mod coral_tests;
#[cfg(test)]
mod hologram_tests;
#[cfg(test)]
mod placed_visuals_tests;
#[cfg(test)]
mod projection_tests;
#[cfg(test)]
mod shortcut_tests;
#[cfg(test)]
mod tests;
