# rw-render (crates/render)

The wgpu (Vulkan-pinned) renderer behind a small `Renderer` trait (`lib.rs`). Everything
is drawn into an offscreen frame of the game's screen size, then presented letterboxed in
the window or read back as a PNG (`FrameImage`) for headless screenshots. Depends on
`rw-sim`, `rw-assets`, `rw-graphics` (backend-agnostic `DrawList`), `rw-save`, `wgpu`,
`bytemuck`, `pollster`, `png`. It holds no gameplay; the app feeds it room data, draw
lists and overlays.

## File map

| File | Holds |
|---|---|
| `src/lib.rs` | `Renderer` trait, `RoomLayers`, re-exports |
| `src/gpu.rs` | `WgpuRenderer` (`new`, `headless`, `render_image`, `present`), pipelines and frame orchestration; `gpu/tests.rs`, `gpu/full_screen_tests.rs` (full-screen effects, `DeathFall`, `WaterSplash` and `WaterLight` at 1366x768), `gpu/placed_visuals_tests.rs` (`Decal` and `Rainbow`) |
| `src/camera.rs` | `Camera`: room pixels (y up) to clip space |
| `src/room_camera.rs` | `RoomCamera`, `Follow`: camera image choice and window position, moved by the shake (`displace`) and the frame's micro shake (`RoomCamera.GetCameraBestIndex/Update/DrawUpdate`) |
| `src/level.rs`, `src/level_gpu.rs` | level graphic (`LevelGraphics`, with the room's `RainMask` texture; `LevelParams`: the room's shader globals incl. `_rimFix` and light angle; `LevelView`: window, `_RAIN`, `_light`) and its GPU side (palette rewrites for `ApplyFade`) |
| `src/sprites.rs` | Futile sprites on CPU: atlas coords, `FSprite` quads, `TriangleMesh`es (and `ElementMesh`es over a decal or the rainbow gradient) from `DrawList`s, draw queue |
| `src/sprite_gpu.rs` | GPU side: one `sprite.wgsl` pipeline per game shader, atlas pages, `_NoiseTex2`, grab textures, the rain mask (`_camInRoomRect`) |
| `src/stage.rs`, `src/stage_gpu.rs` | Futile stage quads in screen pixels (menus, HUD) and their GPU side |
| `src/water.rs`, `src/water_sprites.rs`, `src/water_gpu.rs` | debug flat water; `Water.InitiateSprites/DrawSprites` meshes; `WaterSurface`/`DeepWater` pipelines |
| `src/present.rs`, `src/present_gpu.rs` | `Resolution`, `PresentRect`, `PresentFilter`: letterboxed window presentation |
| `src/overlay.rs`, `src/overlay_gpu.rs` | `Overlay` (ImGui triangles in target pixels) and its pass |
| `src/room_geometry.rs`, `src/mesh.rs`, `src/status.rs`, `src/body.rs` | debug view: tile geometry, tessellation (`Mesh`, `Color`), body style by condition, `ChunkInterpolation` |
| `src/frame_image.rs`, `src/error.rs`, `src/test_room.rs` | `FrameImage` PNG readback, `RenderError`, ASCII test rooms |
| `shaders/level.wgsl` | `LevelColor` (camera image through palette, object shadows from the pre-level grab, `SunBlock`, rimFix, wet terrain, grading) |
| `shaders/sprite.wgsl` | room sprite shaders (`fs_basic`, `fs_flat_light`, `fs_hologram*`, `fs_light_source`, `fs_under_water_light`, `fs_shortcut`, `fs_water_splash`, `fs_water_light`, ...) |
| `shaders/full_screen.wgsl` | the room's full-screen effects, appended to `sprite.wgsl` (`fs_bloom`, `fs_sky_bloom`, `fs_light_bloom`, `fs_light_and_sky_bloom`, `fs_fog`) |
| `shaders/placed_visuals.wgsl` | `CustomDecal`'s `Decal` shader (`fs_decal`) and `Rainbow`'s (`fs_rainbow`), appended after `death_fall.wgsl` |
| `shaders/death_fall.wgsl` | `DeathFallGraphic`'s `DeathFall` shader (`fs_death_fall`), appended after `full_screen.wgsl` |
| `shaders/stage.wgsl` | menu/HUD shaders (`fs_vector_circle`, `fs_menu_text`, `fs_local_bloom`, ...) |
| `shaders/water.wgsl`, `shaders/map.wgsl` | water; region map (`fs_map`, `fs_map_aerial`, `fs_map_shortcut`) |
| `shaders/debug.wgsl`, `shaders/overlay.wgsl`, `shaders/present.wgsl` | debug shapes, ImGui overlay, window blit |
| `tests/installed_rooms.rs` | render test on the installed game's rooms |

## Where is X?

- Renderer API: `lib.rs` `Renderer` trait; implementation `gpu.rs` `WgpuRenderer`.
- Vulkan device/adapter setup: `gpu.rs` `new` (window), `headless` (offscreen).
- Offscreen screenshots: `gpu.rs` `render_image` -> `frame_image.rs`.
- Window presentation / letterbox: `present.rs`, `present_gpu.rs`, `present.wgsl`.
- Level graphic and palette: `level.rs`, `level_gpu.rs`, `level.wgsl`; the palette's rain darkening is rebuilt by the app (`visuals.rs`) and re-uploaded with `Renderer::set_level_palette`.
- Grabs for screen effects: the frame before the level is `FrameTargets::behind` in `gpu.rs` (the game's `_GrabTexture` / `_PreLevelColorGrab`, read by the level, water and sprite shaders, and by `Fog`); a later draw that grabs (`Shader::grabs_frame`: `HologramImage`, the bloom effects and `DeathFall`) gets the frame so far in `FrameTargets::grab` (`copy_to_grab` before each grabbing draw, found by `next_grab`), which `full_screen.wgsl` samples bilinear; the level image, palette and noise views are `LevelTextures::views`.
- Full-screen effects: an ordinary room sprite (`Futile_White` over the screen with a `Shader::Bloom`/`SkyBloom`/`LightBloom`/`LightAndSkyBloom`/`Fog`), chosen and placed by the app (`full_screen_effect.rs`); shaders in `full_screen.wgsl`; `_waterLevel` is `LevelView::water_level`. The death fall is one too (`Shader::DeathFall` in `Bloom`, placed by `death_fall.rs`; `death_fall.wgsl`).
- Which camera image shows / scroll: `room_camera.rs`.
- Decal and rainbow textures: extra atlas pages added with `Renderer::add_atlas_page` (the app uploads a decal's PNG when its room is first shown; the gradient is a page loaded at start-up); shaders in `placed_visuals.wgsl`, which read `_light`, `_cloudsSpeed`, `_WetTerrain` and the light angle from `RoomGlobals`.
- Sprite drawing from a `DrawList`: `sprites.rs`, then `sprite_gpu.rs` + `sprite.wgsl`, pipeline chosen by game shader.
- Menu/HUD quads: `stage.rs`, `stage_gpu.rs`, `stage.wgsl`.
- Water: `water_sprites.rs` (mesh), `water_gpu.rs` + `water.wgsl`.
- Debug ImGui overlay: `overlay.rs`, `overlay_gpu.rs`, `overlay.wgsl`.
- Debug tile/body view: `room_geometry.rs`, `mesh.rs`, `status.rs`, `debug.wgsl`.
- Region map shader: `map.wgsl`, pipelines built in `stage_gpu.rs` (`StageShader::Map*`), chosen by `rw-hud`'s `map/mod.rs`.

## Cross-links

- Divergences: `../../docs/divergences/render.md` (D7 room graphics: level image and palette); see also `graphics.md` there.
- Notes (`../../../rainworld-re/notes/`): `room-rendering.md`, `water-rendering.md`, `water.md`, `sprite-atlases.md`, `unity-assets.md`, `room-format.md`.
- Surveys (`../../../rainworld-re/surveys/`): `depth-scenes.md`.
