//! Read-only loaders for the user's installed game data (rooms, worlds, palettes). Never writes to the game directory.

pub mod room;
pub mod shortcut;

pub use room::{
    Room, RoomError, Shortcut, Slope, TILE_SIZE, Terrain, Tile, TileFlags, tile_centre,
    tile_position,
};
pub use shortcut::{ShortcutData, ShortcutMap, ShortcutType};
