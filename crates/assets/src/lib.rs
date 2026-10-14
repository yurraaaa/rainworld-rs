//! Read-only loaders for the user's installed game data (rooms, worlds, palettes). Never writes to the game directory.

pub mod room;

pub use room::{
    Room, RoomError, Shortcut, Slope, TILE_SIZE, Terrain, Tile, TileFlags, tile_centre,
    tile_position,
};
