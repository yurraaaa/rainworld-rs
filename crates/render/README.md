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
| `src/gpu.rs` | `WgpuRenderer` (`new`, `headless`, `render_image`, `present`), pipelines and frame orchestration; `gpu/tests.rs` |
| `src/camera.rs` | `Camera`: room pixels (y up) to clip space |
| `src/room_camera.rs` | `RoomCamera`, `Follow`: camera image choice and window position (`RoomCamera.GetCameraBestIndex/Update/DrawUpdate`) |
| `src/level.rs`, `src/level_gpu.rs` | level graphic (`LevelGraphics`, `LevelParams`, `LevelView`) and its GPU side; `LevelColor` shader |
| `src/sprites.rs` | Futile sprites on CPU: atlas coords, `FSprite` quads, `TriangleMesh`es from `DrawList`s, draw queue |
| `src/sprite_gpu.rs` | GPU side: one `sprite.wgsl` pipeline per game shader, atlas pages, `_NoiseTex2`, grab textures |
| `src/stage.rs`, `src/stage_gpu.rs` | Futile stage quads in screen pixels (menus, HUD) and their GPU side |
| `src/water.rs`, `src/water_sprites.rs`, `src/water_gpu.rs` | debug flat water; `Water.InitiateSprites/DrawSprites` meshes; `WaterSurface`/`DeepWater` pipelines |
| `src/present.rs`, `src/present_gpu.rs` | `Resolution`, `PresentRect`, `PresentFilter`: letterboxed window presentation |
| `src/overlay.rs`, `src/overlay_gpu.rs` | `Overlay` (ImGui triangles in target pixels) and its pass |
| `src/room_geometry.rs`, `src/mesh.rs`, `src/status.rs`, `src/body.rs` | debug view: tile geometry, tessellation (`Mesh`, `Color`), body style by condition, `ChunkInterpolation` |
| `src/frame_image.rs`, `src/error.rs`, `src/test_room.rs` | `FrameImage` PNG readback, `RenderError`, ASCII test rooms |
| `shaders/level.wgsl` | `LevelColor` (camera image through palette) |
| `shaders/sprite.wgsl` | room sprite shaders (`fs_basic`, `fs_flat_light`, `fs_hologram*`, `fs_light_source`, `fs_under_water_light`, `fs_shortcut`, ...) |
| `shaders/stage.wgsl` | menu/HUD shaders (`fs_vector_circle`, `fs_menu_text`, `fs_local_bloom`, ...) |
| `shaders/water.wgsl`, `shaders/map.wgsl` | water; region map (`fs_map`, `fs_map_aerial`, `fs_map_shortcut`) |
| `shaders/debug.wgsl`, `shaders/overlay.wgsl`, `shaders/present.wgsl` | debug shapes, ImGui overlay, window blit |
| `tests/installed_rooms.rs` | render test on the installed game's rooms |

## Where is X?

- Renderer API: `lib.rs` `Renderer` trait; implementation `gpu.rs` `WgpuRenderer`.
- Vulkan device/adapter setup: `gpu.rs` `new` (window), `headless` (offscreen).
- Offscreen screenshots: `gpu.rs` `render_image` -> `frame_image.rs`.
- Window presentation / letterbox: `present.rs`, `present_gpu.rs`, `present.wgsl`.
- Level graphic and palette: `level.rs`, `level_gpu.rs`, `level.wgsl`.
- Which camera image shows / scroll: `room_camera.rs`.
- Sprite drawing from a `DrawList`: `sprites.rs`, then `sprite_gpu.rs` + `sprite.wgsl`, pipeline chosen by game shader.
- Menu/HUD quads: `stage.rs`, `stage_gpu.rs`, `stage.wgsl`.
- Water: `water_sprites.rs` (mesh), `water_gpu.rs` + `water.wgsl`.
- Debug ImGui overlay: `overlay.rs`, `overlay_gpu.rs`, `overlay.wgsl`.
- Debug tile/body view: `room_geometry.rs`, `mesh.rs`, `status.rs`, `debug.wgsl`.
- Region map shader: `map.wgsl`; which module drives it? (not traced, probably `stage_gpu.rs`)?

## Cross-links

- Divergences: `../../docs/divergences/render.md` (D7 room graphics: level image and palette); see also `graphics.md` there.
- Notes (`../../../rainworld-re/notes/`): `room-rendering.md`, `water-rendering.md`, `water.md`, `sprite-atlases.md`, `unity-assets.md`, `room-format.md`.
- Surveys (`../../../rainworld-re/surveys/`): `depth-scenes.md`.
