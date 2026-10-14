//! Read-only loaders for the user's installed game data (rooms, worlds, palettes, and
//! the Unity asset files: atlases, textures, audio clips). Never writes to the game
//! directory.

mod files;
mod noise;
mod palette;
mod png_image;
pub mod room;
mod room_visuals;
mod settings;
pub mod shortcut;
pub mod unity;

pub use files::{AssetError, StreamingAssets};
pub use noise::NoiseTexture;
pub use palette::{EffectColors, PaletteImage, PaletteTexture, RoomPalette};
pub use png_image::PngImage;
pub use room::{
    Room, RoomError, Shortcut, Slope, TILE_SIZE, Terrain, Tile, TileFlags, tile_centre,
    tile_position,
};
pub use room_visuals::RoomVisuals;
pub use settings::{FadePalette, RegionTemplates, RoomSettings, SettingsFile};
pub use shortcut::{ShortcutData, ShortcutMap, ShortcutType};
pub use unity::UnityResources;
