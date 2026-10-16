//! Read-only loaders for the user's installed game data (rooms, worlds, palettes, and
//! the Unity asset files: atlases, textures, audio clips). Never writes to the game
//! directory.

pub mod ambient;
mod border_exits;
pub mod creatures;
mod files;
mod font_config;
mod identity;
mod nodes;
mod noise;
mod objects;
mod palette;
mod placed;
mod placed_types;
mod png_image;
pub mod region_map;
pub mod region_properties;
pub mod room;
mod room_visuals;
mod settings;
mod short_strings;
pub mod shortcut;
pub mod sound_table;
pub mod unity;
pub mod world;

pub use ambient::{AmbientKind, AmbientSound};
pub use border_exits::RoomBorderExit;
pub use creatures::{
    CreatureSpawner, CreatureType, Lineage, RoomAttraction, SimpleSpawner, SpawnerDen,
};
pub use files::{AssetError, StreamingAssets};
pub use font_config::{FontChar, FontConfig, FontConfigError, FontKerning};
pub use identity::{ActiveMods, SlugcatName, Timeline};
pub use nodes::{
    AbstractRoomNode, BASE_PRE_BAKED_PATHING_CREATURES, ConnMap, Connectivity, NodeType,
};
pub use noise::NoiseTexture;
pub use objects::AbstractObjectType;
pub use palette::{EffectColors, PaletteImage, PaletteTexture, RoomPalette};
pub use placed::{MultiplayerItemData, PlacedObject, ResizableData, RoomEffect};
pub use placed_types::{MultiplayerItemType, PlacedObjectType, RoomEffectType};
pub use png_image::PngImage;
pub use region_map::{MapConnectionLine, RegionMapFiles};
pub use region_properties::{RegionParams, RegionProperties, RoomAttractions};
pub use room::{
    Room, RoomError, RoomItem, Shortcut, Slope, TILE_SIZE, Terrain, Tile, TileFlags, tile_centre,
    tile_position,
};
pub use room_visuals::RoomVisuals;
pub use settings::{DangerType, FadePalette, RegionTemplates, RoomSettings, SettingsFile};
pub use short_strings::ShortStrings;
pub use shortcut::{ShortcutData, ShortcutMap, ShortcutType};
pub use sound_table::{
    ClipNames, ClipVariation, SoundClip, SoundSample, SoundTable, SoundTrigger, TriggerState,
};
pub use unity::UnityResources;
