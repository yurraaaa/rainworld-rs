# rainworld-rs

A faithful reimplementation of [Rain World](https://store.steampowered.com/app/312520/Rain_World/)
(v1.11.8) in Rust. The goal is to do what the game does, tick for tick, and to record every
place where the port differs in [`docs/DIVERGENCES.md`](docs/DIVERGENCES.md).

The port ships no game assets. It reads them, read-only, from your installed copy of Rain
World at startup: rooms, world files, palettes, sprite atlases, fonts, illustrations, sound
effects and music, including the Unity asset files, which a built-in reader opens directly.

## Status

Work in progress. The base game comes first, with Remix and the DLCs off. Remix and the DLCs
are planned for later.

Working now:

- **Boot and menus:** the intro roll, main menu and slugcat select, with the game's fonts,
  sounds and music. Scenes are drawn as the game's flat images for now; the layered depth
  scenes are not ported yet.
- **Starting a game:** a new game plays the Survivor or Monk intro slideshow, then the SU_C04
  opening script.
- **Movement:** the slugcat's movement and physics, corridors and shortcuts, poles, water
  and swimming. The slugcat is drawn as the game draws it.
- **Regions:** a whole region loads, and room-exit pipes take you between rooms, with the
  camera switching between screens.
- **Sound:** sound effects and room ambience; menu and title music.
- **HUD:** the food, karma and rain meters, text prompts and the controls page. There is
  no map yet.
- **Creatures:** the creature framework is in place. Overseers spawn and move between
  rooms, and the guide overseer follows the player with its tutorial text and holograms.
  Bat flies spread between rooms, fly, swarm, flee from the slugcat, burrow, hang in
  chains and travel the pipes. The slugcat catches, carries, throws and eats them.

Not yet ported:

- the rain cycle's rain, flood and death;
- shelters, hibernation, and food and karma carried between cycles;
- saves, continuing a game, and importing saves from the real game;
- region gates;
- creatures other than overseers and bat flies;
- the pause menu, the region map, and depth scenes.

## Building and running

You need a Rust toolchain (edition 2024), a GPU with Vulkan, and an installed copy of Rain
World. Point the port at the game folder with `RW_DIR`:

```sh
RW_DIR="$HOME/.local/share/Steam/steamapps/common/Rain World" cargo run --release
```

Useful flags:

| Flag | Effect |
| --- | --- |
| `ROOM` | Start a game directly in a room, e.g. `SU_B04`, with its region loaded |
| `--process NAME` | Start in `intro-roll` (default), `main-menu`, `slugcat-select` or `game` |
| `--windowed` | Use a window instead of borderless full screen (the game's full-screen option off: the system cursor shows, the menus' cursor sprite does not) |
| `--resolution WxH` | One of the game's screen sizes (default `1366x768`) |
| `--mute` | No sound (the simulation still makes the sounds' random draws) |
| `--seed N` | Seed of the shared random stream |
| `--spawn X,Y` | Place the player on tile X,Y |
| `--debug-view` | Start in the debug tile view |
| `--screenshot FILE.ppm` | Save a frame after one second and quit |
| `--mcp` | Run headless as an MCP server (see below) |

## Controls

These are the game's default keyboard bindings.

| Key | In game | In menus |
| --- | --- | --- |
| Arrows | Move | Select |
| Z | Jump | Confirm |
| Left Shift | Pick up, drop, eat (hold) | |
| X | Throw | |
| Space | Map button (shows the HUD meters) | Confirm |
| Enter | | Confirm |
| Esc | Quit (until the pause menu exists) | Back |

Development keys: R respawns, V toggles the level graphic and the debug view, G shows the
tile grid, and F12 saves a screenshot.

## Workspace

| Crate | Contents |
| --- | --- |
| `crates/sim` (`rw-sim`) | Engine-agnostic gameplay: player, physics, water, shortcuts, world and creatures, sound events |
| `crates/assets` (`rw-assets`) | Game data loaders: rooms, world files, palettes, `sounds.txt`, fonts, the Unity asset reader and FSB5 audio decoding |
| `crates/random` (`rw-random`) | Unity's random number generator, shared by the whole game |
| `crates/graphics` (`rw-graphics`) | Cosmetic graphics: the slugcat and creatures, as backend-agnostic sprite lists |
| `crates/render` (`rw-render`) | Drawing on wgpu (Vulkan); WGSL shaders live in `crates/render/shaders/` |
| `crates/audio` (`rw-audio`) | Sound and music playback on kira |
| `crates/menu` (`rw-menu`) | Process manager, menus, intro roll and slideshows |
| `crates/hud` (`rw-hud`) | The in-game HUD |
| `crates/app` (`rainworld-rs`) | The binary: window, input, the MCP server |

## Driving it headless

`rainworld-rs --mcp` serves an MCP server on stdio, registered in `.mcp.json`. Its tools load
rooms or start games, step the simulation deterministically at 40 ticks per second, report
state, and take offscreen screenshots. [`CLAUDE.md`](CLAUDE.md) lists the tools.

## Testing

```sh
RW_DIR=/path/to/Rain\ World cargo test --workspace
```

Many tests read the installed game's files and compare the port against the game's own
data and the behaviour of its decompiled code.

## Notes

Reverse-engineering notes on the game's formats and behaviour live in a separate
repository, `rainworld-reversing`.

Rain World is made by Videocult. This project is not affiliated with Videocult or its publishers, and
you need your own copy of the game to run it.

## License

rainworld-rs is free software under the GNU General Public License, version 3 or (at your
option) any later version; see [`LICENSE`](LICENSE). The FMOD Vorbis setup table in
`crates/assets/data/` comes from vgmstream and keeps its own licence
(`crates/assets/data/LICENSE-vgmstream`).
