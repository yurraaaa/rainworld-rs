# rw-assets

Read-only loaders for the installed game's data: `StreamingAssets` text and
PNG files (rooms, worlds, settings, palettes, region files, sound tables) and,
in `unity/`, a native reader for the Unity asset files (atlases, textures,
fonts, audio clips). Never writes to the game directory. Dependencies are only
`png`, `lz4_flex`, `lewton`, `base64`, `rustc-hash`, `log`: no other workspace
crate. The sim, graphics and HUD crates build on it; it holds data formats,
not behaviour. Game assets are never committed.

## File map (`crates/assets/src/`)

| File | Holds | Game class |
|---|---|---|
| `lib.rs` | module list | |
| `files.rs` | `StreamingAssets` (path lookup), `AssetError` | |
| `room.rs` | room geometry file: `Room`, `Tile`, `Terrain`, `Shortcut`, `Slope`, `RoomItem` | room txt, `Room.Loaded` |
| `room_visuals.rs` | settings, palettes and per-camera images for one room's level graphic | `RoomCamera` |
| `settings.rs` | `RoomSettings`, `SettingsFile`, template inheritance, `FadePalette`, `DangerType` | `RoomSettings` |
| `placed.rs`, `placed_types.rs` | room effects and placed objects by type name; the `PlacedObject.Type` / `RoomEffect.Type` enums | `PlacedObject`, `RoomSettings.RoomEffect` |
| `placed_light.rs` | a placed light's data: `LightSourceData`, `LightColorType`, `LightBlinkType` | `PlacedObject.LightSourceData` |
| `triggers.rs` | `Triggers:` entries (event/spot triggers, music and image events) | `RoomSettings.LoadTriggers` |
| `ambient.rs` | `AmbientSounds:` line | `RoomSettings.LoadAmbientSounds` |
| `rain_mask.rs` | `RainMask`: a room's `RainMask_<room>` texture (sky columns, the water's run) | `RoomRain.GenerateShelterTex` |
| `palette.rs` | `PaletteImage`, `EffectColors`, `PaletteTexture` (`with_colors`: `SetPixel` overrides), `RoomPalette` | `RoomCamera.LoadPalette` |
| `png_image.rs` | PNG to raw RGBA (`PngImage`) | `Texture2D.LoadImage` |
| `noise.rs` | the level shader's `_NoiseTex` from `resources.assets` | |
| `shortcut.rs` | shortcut map and tube walk | `ShortcutMapper`, `ShortcutHandler` |
| `border_exits.rs` | `RoomBorderExit`: open runs on the room edge | `ShortcutMapper.MapRoomBorders` |
| `nodes.rs` | abstract room nodes from the baked connectivity line | `AbstractRoomNode`, `RoomPreprocessor` |
| `world.rs` | `Regions`, `RegionInfo`, `WorldData`, `AbstractRoomData`, line filters (timelines, `EXCLUSIVEROOM`) | `Region.LoadAllRegions`, `WorldLoader` |
| `region_properties.rs` | a region's `properties.txt` (`RegionParams`, `Room_Attr` lines) | `Region`, `World.LoadMapConfig` |
| `region_map.rs` | `map_<region>.png` layers, `map_<region>.txt` connection lines | `HUD.Map.LoadConnectionPositions` |
| `creatures.rs` | `CreatureType`, world-file `CREATURES` section, lineages, attractions | `CreatureTemplate.Type`, `WorldLoader.AddSpawnersFromString` |
| `objects.rs` | abstract object type names | `AbstractPhysicalObject.AbstractObjectType` |
| `identity.rs` | slugcat names, timelines, active mods | `SlugcatStats.Name`, `.Timeline`, `ModManager` |
| `sound_table.rs` | `soundeffects/sounds.txt`: clips, volume, pitch per `SoundID` | `SoundLoader.LoadSounds` |
| `short_strings.rs` | `text/text_<lang>/strings.txt` table | `InGameTranslator.LoadShortStrings` |
| `font_config.rs` | BMFont text config of Futile fonts | `FFont.LoadAndParseConfigFile` |
| `unity/mod.rs` | Unity reader overview | |
| `unity/serialized.rs` | serialized file header, `ObjectInfo` table, externals | |
| `unity/bundle.rs` | UnityFS bundles (`AssetBundles/*`) | |
| `unity/resources.rs` | `UnityResources`: `open`, `find`, `load`, `text`, `texture_image`, `audio_data` | `Resources.Load` |
| `unity/objects.rs` | the object classes read (`StreamedResource`, clips, textures, text assets) | |
| `unity/bcn.rs` | DXT1 / DXT5 decoding | |
| `unity/fsb5.rs` | FMOD FSB5 banks (PCM / headerless Vorbis) | |
| `unity/reader.rs` | bounds-checked byte cursor | |

## Where is X?

- Room tiles, water, cameras: `room.rs` (`Room`).
- A room's rain/water mask texture: `rain_mask.rs`.
- Room settings (palette, effects, grime, waves, water reflection): `settings.rs`; placed objects: `placed.rs` (lights' data: `placed_light.rs`).
- Room palette blending: `palette.rs`; everything for one level graphic: `room_visuals.rs`.
- World file, room list, connections: `world.rs` (`WorldData`); region list: `Regions`.
- Region `properties.txt`: `region_properties.rs`.
- Initial creature population as data: `creatures.rs`.
- Shortcut and tube geometry: `shortcut.rs`; node connectivity: `nodes.rs`.
- Atlas textures: `unity/resources.rs` (`texture_image`); frame JSON parsing is in `rw-graphics` `atlas.rs`, not here.
- Fonts: glyph config `font_config.rs`; font atlas via `unity/resources.rs`.
- Audio clips: `unity/fsb5.rs` + `UnityResources::audio_data`; which clip a sound plays: `sound_table.rs`.
- Unity asset reader entry point: `unity::UnityResources::open`.
- Translated strings: `short_strings.rs`.
- Map picture for the HUD map: `region_map.rs`.

## Cross-links

Notes in `../../../rainworld-re/notes/`: `room-format.md`, `world-format.md`,
`world-loading.md`, `room-rendering.md`, `unity-assets.md`,
`sprite-atlases.md`, `sound.md`, `menu.md` (fonts, strings), `shortcuts.md`,
`ai-map.md`. Surveys: `spawning.md`.

Divergences: `../../docs/divergences/` (one file per crate, IDs global): D7 room graphics, D12 sound, D13 front end
(fonts, strings), D14 worlds, rooms and the realizer.
