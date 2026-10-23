# rainworld-rs (crates/app)

The binary: CLI parsing, the winit window and main loop, the process manager's
processes (menus, game), and the MCP server (`--mcp`) and debug UI (F1) that drive it.
It glues the other crates together (`rw-sim`, `rw-assets`, `rw-render`, `rw-graphics`,
`rw-audio`, `rw-menu`, `rw-hud`, `rw-save`, `rw-random`) and owns the game-side state
those crates must not know about (`Game`, save slot, HUD map data, music requests).
Extra deps: `winit`, `rmcp`/`tokio`/`schemars` (MCP), `imgui` + `imgui-winit-support`.
Gameplay logic belongs in `rw-sim`, not here.

## File map (`src/`)

| File | Holds |
|---|---|
| `main.rs` | `Args::parse` (CLI), `GameDir::from_env` (`RW_DIR`), `run()` choosing MCP / window / screenshot |
| `app.rs` | winit `ApplicationHandler` (`App`: `resumed`, `window_event`, `about_to_wait`), frame pacing |
| `timestep.rs` | sim rate and Unity `Time.deltaTime` as the process manager sees it |
| `input.rs` | keyboard to per-tick `Input` (`InputState`), Rewired-style key map |
| `frontend.rs` | `Frontend`/`FrontendRenderer`: processes under `rw_menu::ProcessManager` (game, Initialization, menus), switch fade, `AppContext` |
| `game.rs` | `Game`: `RainWorldGame` + player graphics; `start`, `tick`, `*_draw_lists`, `sync_sprite_order` (camera 0's container record after each tick) and `in_container_order` (the frame's lists in that order, jaws nested), sound events |
| `scene.rs` | `Scene`/`SceneRenderer`: what a frame shows (level, water, sprites, HUD: the HUD's first container (the map) is queued between `Bloom` and the room's `HUD` container sprites, `prepare_hud`), shared by window and screenshots; uploads a room's decal images as atlas pages when first shown; the room camera's update before the sim tick and its shake after it, the frame's micro shake |
| `visuals.rs` | room settings, palettes, shown camera image; `DarkPalette`/`ApplyFade`, `_light` from the rain cycle or the room's lightning (`num4`), the lightning's palette texels (0, 7) and (1, 7), `_waterLevel`, `LevelState` for `get_state`'s `level_params` |
| `full_screen_effect.rs`, `full_screen_effect_tests.rs` | `FullScreenEffect`: the room camera's `fullScreenEffect` (Bloom, SkyBloom, SkyAndLightBloom, LightBurn/Lightning, Fog): `ApplyPalette`'s choice, `DrawUpdate`'s alpha, the `Futile_White` sprite |
| `death_fall.rs` | `DeathFall`: the camera room's `DeathFallGraphic`: height (also for `_waterLevel`), counter and its `DeathFall` sprite in `Bloom` |
| `water_light.rs` | `WaterLight`: the room camera's water caustics sprite (`WaterLight` shader, first in `Foreground`), its alpha from `WaterReflectionAlpha` and the palette darkness |
| `window_identity.rs` | window title "Rain World" and app id `rainworld-rs` (Wayland/X11) |
| `sprites.rs` | atlas loading (`rainWorld`, `uiSprites`), overseer projection textures, the rainbow gradient, `LightMask0`, `glyphs`, `maze` |
| `shortcuts.rs` | camera-0 `ShortcutGraphics` fed from the game |
| `jaws.rs`, `jaws_tests.rs` | `Jaws`: held objects' sprites nested into their holders' jaw containers (a held lizard's holds inside it; hold cycles D20) |
| `game_background_scene.rs` | the camera room's background scenes on screen: each element's sprites (`BackgroundSceneSprites`), the images they need, the scene globals, the dust puffs |
| `sprite_order.rs`, `sprite_order_tests.rs` | `SpriteOrder`: camera 0's sprite leasers in container order as the game adds and re-adds them (new room, `AddObject`, releases, `ChangeOverlap`, jaw containers, the full-screen effect raised by `ApplyPalette`, the death fall first; D21) |
| `hud_map.rs` | `MapData`/map owner the HUD map reads |
| `game_music.rs` | game side of `MusicPlayer`: HUD state, song requests, threat tracker inputs |
| `hibernation.rs` | `RainWorldGame.Win`: save side and next-cycle state |
| `screen_package.rs` | `CommunicateWithUpcomingProcess` data for sleep/dream/death/starve screens |
| `saves.rs` | save slot paths, load for menus, story save state, importer |
| `room_catalog.rs` | rooms on disk (`list_rooms`, debug UI) |
| `dev_spawn.rs` | `spawn_creature`, `spawn_object`, `teleport_player`, the dev rain timer edit (`RainTimerEdit`) shared by MCP and debug UI |
| `mcp/mod.rs` | `#[tool]` handlers (thin) and server setup |
| `mcp/session.rs` | `Session`: all tool logic as methods; `state()` builds `get_state` |
| `mcp/params.rs`, `mcp/reply.rs` | tool parameter structs (schema docs) and JSON reply types |
| `mcp/dev_tools.rs` | dev spawns, `set_rain_timer`, `screenshot`/`get_tiles` of another realized room |
| `mcp/background_scene_reply.rs` | `get_state` `background_scene`: per realized room's scene, its kind, elements, the camera's place on it (`camera_world`, `altitude`), each `DistantLightning`'s state, the dust puffs |
| `mcp/insect_reply.rs` | `get_state` `insects`: per-room cosmetic insects |
| `mcp/particles_reply.rs` | `get_state` `particles`: per realized room, counts of dandelions, stalks, sparks, specks and spores and the spawners' state |
| `mcp/placed_visuals_reply.rs` | `get_state` `decals`, `rainbows`: the realized rooms' custom decals and rainbows |
| `mcp/gravity_reply.rs` | `get_state` `room_gravity`, `broken_anti_gravity`: each `ZeroG`/`BrokenZeroG` or gradient-gravity-script room's gravity and electric power, the world's `BrokenAntiGravity` switch |
| `mcp/projection_reply.rs`, `mcp/projection_tests.rs` | `get_state` `projectors`, `scan_lines`, `star_matrices`: the realized rooms' superstructure projections (ideal and actual glyph, scan line and cursor counts, grid state, cursors; each scan line; each star matrix's rotations and focus) |
| `mcp/vine_reply.rs`, `mcp/vine_tests.rs` | `get_state` `climbable_vines` (per realized room: vines with kind, points, total length; pending events) and the player's `vine_pos`, `vine_grab_delay`, `vine_climb_cursor`, `want_to_grab` (tested with a vine of the test's own) |
| `mcp/circuit_reply.rs`, `mcp/circuit_tests.rs` | `get_state` `coral_circuits` (per cluster: places, tile box, bits, displaced and repairing bits, links, connectors, crabs, the first displaced bit) and the player's `circuit_swim_resistance`, `circuit_jump_meter`, `swim_bits`; tested on `SS_D02` (state, swimming through a circuit, the picture) |
| `mcp/coral_reply.rs` | `get_state` `coral_systems`, `coral_neurons`, `coral_stems`, `projected_circles`, `wall_mycelia`, `neuron_sparks`: the realized rooms' `CoralNeuronSystem`s (wind, age, frozen, registry and its owners), `CoralNeuron`s (ends, segments, strands, pings, climbed), `CoralStem`s (root, segments' chunks, neurons made, climbed), `ProjectedCircle`s, `WallMycelia` (strands, culled, pairs, first strand) and `NeuronSpark`s |
| `mcp/effects_reply.rs` | `get_state` `electric_death`, `border_push_back`: the realized rooms' `ElectricDeath`s (intensity, sin, colour, flashes, loop volumes, recent strikes) and `RoomBorderPushBack`s (edges, `pushPower`) |
| `mcp/fixtures_reply.rs` | `get_state` `light_fixtures`, `waterfalls`: the realized rooms' `Redlight`s, `HolyFire`s, `SlimeMoldLight`s (lights, flicker state, embers, discs) and `WaterFall`s (tile, flow, ends, strike level, contact, bubbles) |
| `mcp/ss_grab_reply.rs` | `get_state` `deep_processing`, `deep_processing_lights`, `gravity_disruptors`: the realized rooms' `DeepProcessing` quads (power), their lights (rad, alpha) and `GravityDisruptor`s (power, streaks, `roomSpecks`, their `DisruptorSpeck`s) |
| `mcp/ss_electric_reply.rs` | `get_state` `light_rods`, `zap_coils`, `zap_coil_lights`, `zap_flashes`, `fuses`: the realized rooms' superstructure rods (vessels, their lights), zap coils (disruption, power, spells, volumes), flashes, coil lights and fuse grids (modes, malfunction, power) |
| `mcp/placed_light_reply.rs` | `get_state` `light_beams`, `spot_lights`: the realized rooms' placed light beams (quad, data, sun factor, draw alpha, colours) and spot lights |
| `mcp/placed_cosmetics_reply.rs` | `get_state` `slime_molds`, `token_stalks`: the realized rooms' cosmetic slime molds and dead token stalks |
| `mcp/music_reply.rs`, `mcp/tiles.rs`, `mcp/window.rs` | music/triggers replies, `get_tiles` text, `--mcp --window` mirror |
| `mcp/*_tests.rs`, `mcp/tests.rs`, `mcp/shortcut_shots.rs` | MCP-level tests (installed game) |
| `debug_ui/` | `mod.rs` (ImGui host, F1), `tabs.rs` (tabs), `actions.rs` (`DebugAction`s), `view.rs` (read model) |

## Where is X?

- CLI flags: `main.rs` `Args::parse` (`--mcp --window --windowed --mute --process --resolution --spawn --script --screenshot --slugcat --seed --save-dir --game-saves --debug-view`).
- Main loop / frame pacing: `app.rs` `about_to_wait`, `window_event`; ticks run via `frontend.rs`.
- One sim tick: `game.rs` `Game::tick`; drawing: `scene.rs` `SceneRenderer`.
- MCP tool registration: `mcp/mod.rs` (`list_rooms`, `load_room`, `new_game`, `step`, ...); bodies in `mcp/session.rs` methods of the same name.
- `get_state` fields: `mcp/session.rs` `Session::state`, types in `mcp/reply.rs`.
- `screenshot`: `Session::screenshot` (`session.rs`); other-room variant `mcp/dev_tools.rs` `room_screenshot`.
- `get_tiles`: `session.rs` `tiles`, text in `mcp/tiles.rs`.
- `load_process` / `step_process`: `session.rs` and `frontend.rs`.
- `import_save`: `session.rs` `import_save`, logic in `saves.rs`.
- `spawn_creature` / `spawn_object` / `teleport_player`: `mcp/dev_tools.rs` -> `dev_spawn.rs`.
- Debug UI tabs (Region, Teleport, Rain timer, Creatures, Spawn creature, Spawn object): `debug_ui/tabs.rs`, one `DebugTab` impl each.
- `get_hud`, `hud_message`: `session.rs` `hud_state`, `hud_message`.
- Sleep/death/starve screens' data: `screen_package.rs`, `hibernation.rs`.
- Music requests from the sim: `game_music.rs`.
- Room full-screen effects (which, strength, container order): `full_screen_effect.rs`, raised in `scene.rs` (`apply_palette`), drawn by `rw-render`'s `full_screen.wgsl`; `get_state`'s `full_screen_effect`. The death fall at an open bottom: `death_fall.rs` (drawn by `death_fall.wgsl`), `get_state`'s `death_fall`. The water's caustics: `water_light.rs` (its mask texture `rw_assets::RainMask`, made in `visuals.rs`; drawn by `sprite.wgsl`'s `fs_water_light`), `get_state`'s `water_light`.

## Cross-links

- Divergences: `../../docs/divergences/app.md` (D18 debug UI and dev spawns, D20 jaw hold cycles); the import button is D13 (`../../docs/divergences/menu.md`).
- Notes (`../../../rainworld-re/notes/`): `menu.md`, `saves.md`, `shelter.md`, `hud.md`, `music.md`, `sound.md`, `room-rendering.md`, `unity-assets.md`.
- Surveys (`../../../rainworld-re/surveys/`): `menus.md`, `sleep-death-screens.md`, `campaign.md`.
