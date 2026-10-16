//! Procedural creature graphics: the cosmetic simulation the game runs in its
//! `GraphicsModule`s, and a backend-agnostic sprite list for the renderer.
//!
//! Ported: the slugcat ([`PlayerGraphics`]: tail, head, legs, hands, draw offsets,
//! breathing, blinking, sprite frame choice), base game with Remix off; Futile atlas
//! frame data ([`Atlas`]); the overseer's ([`OverseerGraphics`]: mycelia, lens, sprites)
//! and its holograms' ([`HologramGraphics`]); the bat fly's ([`FlyGraphics`]); the shelter door's
//! ([`ShelterDoorSprites`]). Behaviour: `rainworld-re/notes/slugcat-graphics.md` and
//! `sprite-atlases.md`; differences from the game are in `docs/DIVERGENCES.md`.

mod atlas;
mod body_part;
mod cosmetic_graphics;
mod draw;
pub mod elements;
mod fly_graphics;
mod hand;
mod hologram_draw;
mod hologram_matrix;
mod hologram_shape;
mod limb;
mod looker;
mod math;
mod mycelium;
mod overseer_draw;
mod overseer_graphics;
mod player_draw;
mod player_graphics;
mod shelter_door_graphics;
mod shortcut_graphics;
mod tail;
mod water_drip;
mod weapon_graphics;

pub use atlas::{Atlas, AtlasError, AtlasFrame, Rect, Size};
pub use body_part::{BodyPart, PointConnection};
pub use cosmetic_graphics::CosmeticSprites;
pub use draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, SlugcatName, Sprite};
pub use fly_graphics::FlyGraphics;
pub use hand::SlugcatHand;
pub use hologram_draw::{HologramFrame, HologramGraphics};
pub use hologram_matrix::{HoloLine, HologramMatrix, LensFrame};
pub use hologram_shape::{HologramLine, PartShape};
pub use limb::{GripSearch, Limb, LimbMode};
pub use looker::ObjectLooker;
pub use math::{Custom, CustomVec, Mathf, UnityVector3, Vec2Bezier, Vec3};
pub use mycelium::{Mycelium, MyceliumLink};
pub use overseer_graphics::{DrawPositions, OverseerGraphics};
pub use player_draw::{SPRITE_COUNT, SPRITE_LAYERS, TAIL_TRIANGLES};
pub use player_graphics::PlayerGraphics;
pub use shelter_door_graphics::ShelterDoorSprites;
pub use shortcut_graphics::{
    ExitBehind, ShortcutFrame, ShortcutGraphics, ShortcutPalette, ShortcutRoom, ShortcutVessel,
};
pub use tail::{TailAnchor, TailSegment};
pub use water_drip::WaterDripSprites;
pub use weapon_graphics::{PEBBLES, PebbleElement, SMALL_SPEAR, WeaponSprites};

#[cfg(test)]
mod hologram_tests;
#[cfg(test)]
mod shortcut_tests;
#[cfg(test)]
mod tests;
