//! Read-only loaders for the user's installed game data (rooms, worlds, palettes). Never writes to the game directory.

mod cache;
mod files;
mod noise;
mod palette;
mod png_image;
pub mod room;
mod room_visuals;
mod settings;
pub mod shortcut;

pub use cache::{AssetCache, CACHE_ENV};
pub use files::{AssetError, StreamingAssets};
pub use noise::NoiseTexture;
pub use palette::{EffectColors, PaletteImage, PaletteTexture};
pub use png_image::PngImage;
pub use room::{
    Room, RoomError, Shortcut, Slope, TILE_SIZE, Terrain, Tile, TileFlags, tile_centre,
    tile_position,
};
pub use room_visuals::RoomVisuals;
pub use settings::{FadePalette, RegionTemplates, RoomSettings, SettingsFile};
pub use shortcut::{ShortcutData, ShortcutMap, ShortcutType};
