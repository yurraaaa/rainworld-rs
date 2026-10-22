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
| `scene.rs` | `Scene`/`SceneRenderer`: what a frame shows (level, water, sprites, HUD), shared by window and screenshots; the room camera's update before the sim tick and its shake after it, the frame's micro shake |
| `visuals.rs` | room settings, palettes, shown camera image; `DarkPalette`/`ApplyFade`, `_light` from the rain cycle, `LevelState` for `get_state`'s `level_params` |
| `window_identity.rs` | window title "Rain World" and app id `rainworld-rs` (Wayland/X11) |
| `sprites.rs` | atlas loading (`rainWorld`, `uiSprites`), overseer projection textures |
| `shortcuts.rs` | camera-0 `ShortcutGraphics` fed from the game |
| `jaws.rs`, `jaws_tests.rs` | `Jaws`: held objects' sprites nested into their holders' jaw containers (a held lizard's holds inside it; hold cycles D20) |
| `sprite_order.rs`, `sprite_order_tests.rs` | `SpriteOrder`: camera 0's sprite leasers in container order as the game adds and re-adds them (new room, `AddObject`, releases, `ChangeOverlap`, jaw containers; D21) |
| `hud_map.rs` | `MapData`/map owner the HUD map reads |
| `game_music.rs` | game side of `MusicPlayer`: HUD state, song requests, threat tracker inputs |
| `hibernation.rs` | `RainWorldGame.Win`: save side and next-cycle state |
| `screen_package.rs` | `CommunicateWithUpcomingProcess` data for sleep/dream/death/starve screens |
| `saves.rs` | save slot paths, load for menus, story save state, importer |
| `room_catalog.rs` | rooms on disk (`list_rooms`, debug UI) |
| `dev_spawn.rs` | `spawn_creature`, `spawn_object`, `teleport_player` shared by MCP and debug UI |
| `mcp/mod.rs` | `#[tool]` handlers (thin) and server setup |
| `mcp/session.rs` | `Session`: all tool logic as methods; `state()` builds `get_state` |
| `mcp/params.rs`, `mcp/reply.rs` | tool parameter structs (schema docs) and JSON reply types |
| `mcp/dev_tools.rs` | dev spawns, `screenshot`/`get_tiles` of another realized room |
| `mcp/insect_reply.rs` | `get_state` `insects`: per-room cosmetic insects |
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
- Debug UI tabs (Region, Teleport, Creatures, Spawn creature, Spawn object): `debug_ui/tabs.rs`, one `DebugTab` impl each.
- `get_hud`, `hud_message`: `session.rs` `hud_state`, `hud_message`.
- Sleep/death/starve screens' data: `screen_package.rs`, `hibernation.rs`.
- Music requests from the sim: `game_music.rs`.

## Cross-links

- Divergences: `../../docs/divergences/app.md` (D18 debug UI and dev spawns, D20 jaw hold cycles); the import button is D13 (`../../docs/divergences/menu.md`).
- Notes (`../../../rainworld-re/notes/`): `menu.md`, `saves.md`, `shelter.md`, `hud.md`, `music.md`, `sound.md`, `room-rendering.md`, `unity-assets.md`.
- Surveys (`../../../rainworld-re/surveys/`): `menus.md`, `sleep-death-screens.md`, `campaign.md`.
