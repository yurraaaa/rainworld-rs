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

- **Boot and menus:** the intro roll, main menu and slugcat select (a saved campaign's page
  shows its region, cycle, karma and food), with the game's fonts, sounds and music. The main menu and options backgrounds and the
  intro slideshows are the game's layered depth scenes (parallax, drifting camera or the
  slides' camera paths, focus blur); the slugcat select, sleep and death screens still
  show the game's flat images.
- **Starting a game:** a new game plays the Survivor or Monk intro slideshow, then the SU_C04
  opening script. A saved game continues in its shelter with its food, karma and cycle.
- **Saves:** the game's own save format, kept in the port's own directory; exiting from the
  pause menu saves as the game does.
- **Shelters:** with enough food, staying still in a shelter closes its door and hibernates;
  the cycle is saved, the sleep screen shows the karma ladder, food and map, and the next
  cycle starts there with food, karma and the region's creatures carried over. A door
  closing on some food, but less than the meter's line, hibernates starving: the next
  cycle the slugcat is malnourished (thin and grey, slower, easily exhausted) until a
  full meter cures it; a door closing on no food (malnourished: on less than a full
  meter) saves the cycle as a death and shows the starve screen.
  Saves from the real game can be imported (see below).
- **Death:** dying shows the game-over prompt; Space then saves the death (karma lost, a
  karma flower's spot kept for reinforced karma and the Monk) and goes on to the death
  screen, where the karma ladder drops a notch.
- **Movement:** the slugcat's movement and physics, corridors and shortcuts, poles, water
  and swimming. The slugcat is drawn as the game draws it.
- **Regions:** a whole region loads, and room-exit pipes take you between rooms, with the
  camera switching between screens.
- **Sound:** sound effects and room ambience; menu and title music.
- **HUD:** the food, karma and rain meters, text prompts, the controls page and the
  region map (hold Space; arrows pan, jump and throw change layers), revealed where you
  have been; what you discovered is saved.
- **Creatures:** the creature framework is in place. Overseers spawn and move between
  rooms, and the guide overseer follows the player with its tutorial text and holograms.
  Bat flies spread between rooms, fly, swarm, flee from the slugcat, burrow, hang in
  chains and travel the pipes. The slugcat catches, carries, throws and eats them, and
  the guide's tutorial teaches it to (the Eat instruction, then the super jump once fed).
  Lizards leave their dens and roam the region's unrealized rooms abstractly, without
  bodies yet (one that reaches a loaded room waits there, unseen, until it unloads).
- **Items:** rocks and spears lie in the rooms, roll, stick into the ground, and spears
  stuck in walls are poles; shelters keep them across cycles. The slugcat picks them
  up, carries them (through pipes too), switches hands, lays them down, and swallows
  and coughs up rocks.

Not yet ported:

- the rain cycle's rain, flood and death;
- dreams (a black stand-in follows hibernation when one is due), and the screens
  behind the sleep screen's PASSAGE button (it shows, greyed out; passages are still
  earned and shown on the meters);
- region gates;
- creatures other than overseers and bat flies (lizards roam abstractly, but have no
  body yet);
- throwing rocks and spears (a throw drops them for now).

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
| `--windowed` | Use a window instead of borderless full screen for this run (the game's full-screen option off: the system cursor shows, the menus' cursor sprite does not) |
| `--resolution WxH` | One of the game's screen sizes for this run (otherwise the options', default `1366x768`) |
| `--mute` | No sound (the simulation still makes the sounds' random draws) |
| `--seed N` | Seed of the shared random stream |
| `--spawn X,Y` | Place the player on tile X,Y |
| `--debug-view` | Start in the debug tile view |
| `--screenshot FILE.ppm` | Save a frame after one second and quit |
| `--mcp` | Run headless as an MCP server (see below) |
| `--save-dir DIR` | Keep saves in DIR instead of the platform data directory |
| `--game-saves DIR` | Where the importer reads the game's saves |

## Saves

The port keeps its saves in `~/.local/share/rainworld-rs/` on Linux and
`%APPDATA%\rainworld-rs\` on Windows, in the game's format, and never writes the game's
saves. To continue a game from the real game, import a slot (`sav`, `sav2` or `sav3`)
from OPTIONS > IMPORT SAVE FROM RAIN WORLD (the selected save slot) or with the MCP
tool `import_save`. It reads the
game's saves (on Linux, Steam's Proton prefix next to the game folder; on Windows,
`AppData\LocalLow\Videocult\Rain World`) and copies the slot, after moving the port's
previous copy to `backup/`. Data from the DLCs, Remix and mods is kept as it is.
The options (volumes, screen, save slot, ...) are kept there too, in the game's
`options` format; the OPTIONS menu changes them.

## Controls

These are the game's default keyboard bindings.

| Key | In game | In menus |
| --- | --- | --- |
| Arrows | Move | Select |
| Z | Jump | Confirm |
| Left Shift | Pick up; double tap to switch hands; hold down and Shift to drop; hold to eat, swallow or cough up | |
| X | Throw | |
| Space | Map button (the HUD meters and region map) | Confirm |
| Enter | | Confirm |
| Esc | Pause menu (again to continue) | Back |

Development keys: R respawns (only in a room started directly, not in a campaign), V
toggles the level graphic and the debug view, G shows the tile grid, and F12 saves a
screenshot.

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
| `crates/hud` (`rw-hud`) | The in-game HUD, and the menus that own one (slugcat select, sleep and death screens) |
| `crates/save` (`rw-save`) | The game's save format, the save directories and the importer |
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
