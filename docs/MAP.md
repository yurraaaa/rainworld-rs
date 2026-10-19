# Codebase map

Start here to find code. Each crate has a `README.md` with its file map and a
"where is X?" list; this page says which crate to open.

## Crates

| Crate | Holds | Original game side |
|---|---|---|
| [`rw-sim`](../crates/sim/README.md) | Gameplay: 40 Hz tick, worlds, rooms, player, abstract and realized creatures (lizards, flies, overseers), items, water, shortcuts, shelters, noise, sound events | `Room`, `Player`, `Creature`s and their AI, `AbstractRoom`, `World` |
| [`rw-graphics`](../crates/graphics/README.md) | Cosmetic simulation (tails, limbs, cosmetics) and the backend-agnostic `DrawList` | `*Graphics` classes |
| [`rw-render`](../crates/render/README.md) | wgpu renderer, WGSL shaders, level image and palette, offscreen screenshots | `RoomCamera`, shaders |
| [`rw-assets`](../crates/assets/README.md) | Readers for the installed game's files and Unity assets (atlases, fonts, audio) | `StreamingAssets`, `Futile` loaders |
| [`rw-audio`](../crates/audio/README.md) | `kira` playback, music player, ambient loops | `VirtualMicrophone`, `MusicPlayer` |
| [`rw-hud`](../crates/hud/README.md) | In-game HUD plus the sleep, death, starve, pause and slugcat select screens | `HUD.*`, `Menu.SleepAndDeathScreen`, `SlugcatSelectMenu` |
| [`rw-menu`](../crates/menu/README.md) | Front end: menu framework, intro roll, main and options menus, process manager | `Menu.*`, `ProcessManager` |
| [`rw-save`](../crates/save/README.md) | Save format, progression, importer of the real game's slots | `PlayerProgression`, `SaveState`, `WinState` |
| [`rw-random`](../crates/random/README.md) | `UnityRandom` (xorshift128) | `UnityEngine.Random` |
| [`rainworld-rs`](../crates/app/README.md) | Binary: window, main loop, process glue, MCP server, debug UI | `RainWorld`, `RainWorldGame` |

Dependency direction: `rw-random`, `rw-assets` at the bottom; `rw-sim` above
them (gameplay only, no I/O); `rw-graphics`, `rw-render`, `rw-audio`, `rw-hud`,
`rw-menu`, `rw-save` on top; `app` glues all of them. Check a crate's
`Cargo.toml` before adding a dependency.

## Other places

- Divergences from the game: [`docs/divergences/`](divergences/README.md), one file per crate, global IDs (D1, D14, ...) cited in code.
- Bugs and open items: [`BUGS.md`](BUGS.md), [`OPEN-ITEMS.md`](OPEN-ITEMS.md).
- Reverse-engineering notes (behaviour of the original): `../rainworld-re/notes/`, surveys in `../rainworld-re/surveys/`.
- Decompiled game: `/home/yura/.local/share/ilspy/split/` (one file per type; see `AGENTS.md`).
- Driving the app headless: the MCP server (`crates/app/src/mcp/`), see `AGENTS.md`.

## Quick routing

- Creature behaviour or body: `rw-sim` (`creature/`), its drawing: `rw-graphics`, its voice: `rw-sim` sound events plus `rw-audio`.
- Something on screen in a menu: `rw-menu`; on a screen with the karma ladder, food meter or slugcat select: `rw-hud`.
- Anything read from the game's files: `rw-assets`; anything written: `rw-save`.
- A random draw: `rw-random` provides the stream; the draws themselves are in the code that ports each game call.
- New MCP field or debug tab: `crates/app/src/mcp/`, `crates/app/src/debug_ui/tabs.rs`.

## Keeping it current

A new crate or top-level module updates this page and its crate README in the
same change. Per-file detail belongs in the crate README, not here.
