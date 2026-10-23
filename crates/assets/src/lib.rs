//! Read-only loaders for the user's installed game data (rooms, worlds, palettes, and
//! the Unity asset files: atlases, textures, audio clips). Never writes to the game
//! directory.
//!
//! Mirrors: -

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
mod placed_light;
mod placed_types;
mod placed_visuals;
mod png_image;
mod rain_mask;
pub mod region_map;
pub mod region_properties;
pub mod room;
mod room_visuals;
mod settings;
mod short_strings;
pub mod shortcut;
pub mod sound_table;
pub mod triggers;
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
pub use placed::{
    IggyCondition, IggySymbol, MultiplayerItemData, PlacedObject, QuadData,
    ReliableIggyDirectionData, ResizableData, RoomEffect,
};
pub use placed_light::{LightBlinkType, LightColorType, LightSourceData};
pub use placed_types::{MultiplayerItemType, PlacedObjectType, RoomEffectType};
pub use placed_visuals::{CustomDecalData, RainbowData};
pub use png_image::PngImage;
pub use rain_mask::RainMask;
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
pub use triggers::{
    EventTrigger, EventType, MusicEvent, ShowProjectedImageEvent, SpotArea, StopMusicEvent,
    StopMusicType, TriggerType, TriggeredEvent,
};
pub use unity::UnityResources;
