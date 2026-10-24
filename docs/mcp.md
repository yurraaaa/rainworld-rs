# Driving the app (MCP)

Moved from AGENTS.md. Read this before using the `rainworld-rs` MCP tools.

`rainworld-rs --mcp` is an MCP server on stdio (`crates/app/src/mcp/`),
registered in `.mcp.json` as `rainworld-rs` (runs `cargo run --release` when the
session starts, so it serves the main checkout's code of that moment). Tools: `list_rooms`, `load_room`,
`new_game`, `step`, `step_sequence`, `respawn`, `set_player`, `get_state`,
`get_exits`, `get_tiles`, `get_hud`, `hud_message`, `screenshot`,
`load_process`, `step_process`, `import_save`, `set_destination`,
`spawn_creature`, `spawn_object`, `teleport_player`, `set_rain_timer`. To check behaviour,
prefer these over `--screenshot`/`--script`; briefs for such agents say so.

## Agents in worktrees: `scripts/mcp.py`

The session's `mcp__rainworld-rs__*` tools talk to the one server the main session started from
the main checkout: main's code at session start, one game shared by every agent. An agent in a
worktree uses `scripts/mcp.py` from its worktree root instead, which builds and runs that
worktree's server (saves in `target/mcp-saves`, never the real save directory):

```
scripts/mcp.py serve                                  # build --release and start in the background
scripts/mcp.py call load_room '{"name": "SU_A22", "seed": 1}'
scripts/mcp.py --max 300 batch '[{"tool": "step", "args": {"ticks": 40}},
  {"tool": "get_state", "args": {"fields": ["tick", "player"]}},
  {"tool": "screenshot", "args": {"crop_radius": 200}}]'   # screenshot: prints a PNG path
scripts/mcp.py stop                                   # before finishing
```

`load_room`, `new_game` and `step` reply with the full state (tens of KB): cut them with
`--max N` and read what you need with `get_state`'s `fields`. Arguments are as in the tool
schemas (`scripts/mcp.py tools` lists them).

- Headless: the sim advances only in `step` / `step_sequence` (40 ticks/s).
  Runs are not required to repeat exactly: loading is threaded as in the game
  (user, 2026-10-24), so the tick a room or region becomes ready can differ
  between runs. While a room loads, each tick (`step`, `step_process`) waits
  until 25 ms have passed since the last (the game's 40 Hz frame; port-only,
  D14); otherwise ticks run unpaced. `load_room` and `new_game` start a fresh music player (it draws
  from the game's random stream).
- `load_room` loads a room's region world with the player in it (arena
  levels: a single-room world) as `slugcat` (default `White`); `new_game`
  starts a campaign (`White`, `Yellow`, `Red`; `malnourished` option) at its
  start den. Both (and `respawn` with a seed) step the game without input
  until the start room is `readyForAI` before replying (at least 3 ticks; the
  room loads on a thread, so the reply's `tick` can vary).
- `get_state` reports the following. Pass `fields` (e.g. `["tick", "player"]`)
  to get only those top-level fields; prefer it, the full reply is large.
  Requested fields that are empty or unknown come back in `absent`. Empty fields are left
  out; the field list is in `crates/app/src/mcp/`. New sim state must be
  added there. `get_exits` lists the camera room's exits (entrance tile, the
  input that enters, destination room, arrival node, camera).
  - Camera and loading: the camera room (`room` is the room the camera shows;
    during a pending move `camera_move` names the room it is moving to; the
    camera's unapplied move is applied in its next update), the vessel in the
    lobby, realized rooms' entry waits, `loading_progress` and `preparer` (a
    loading room's `RoomPreparer`: status, calls, `thread_started`,
    `thread_finished`, `requested_stage`, done), `orphaned_preparers` (rooms
    abstracted while loading), `screen_size` (the sim's
    `rainWorld.screenSize`), `shortcut_graphics` (camera 0's
    `ShortcutGraphics`: room, `waiting`), polled only when a frame is drawn,
    so headless `step`s leave `waiting` true until a `screenshot`.
  - Player: food, grasps, pick-up candidate, stomach, counters,
    sneak/loudness, `held_by`, `danger_grasp` and its time, struggle buttons,
    `can_struggle`, `cant_be_grabbed_counter`, ...; the player's `owned_items`
    on the ground; `vine_pos`, `vine_grab_delay`, `vine_climb_cursor`,
    `want_to_grab`; `circuit_swim_resistance`, `circuit_jump_meter` and
    `swim_bits` (in `player`).
  - Creatures and objects: every abstract creature (kind, ID, room, node,
    tile, den, realized, alive, abstract AI; realized flies, lizards with
    their movement, tracker, relationships, noise, decision (`social`: fight
    target and anger, `submitted_to`, last signal, missions,
    `taming_difficulty`, `gift`) and, in a viewed room, graphics and voice;
    overseers with AI and guide state; stun/kill tag, `rain_death`, `social`,
    `danger`, `carries`), every abstract object (and realized state), `dens`,
    `quantified`, `flies_world`, `heavy_ai`, `shortcut_helper` (per room: the
    pushers with tile, direction, wrong hole, floor, swell and neighbours, and
    the objects being pushed out of entrances), `border_push_back` (per room:
    the edges, `pushPower`), `session`, `region_state`.
  - Rain and water: ceiling drips, each room's `rumble_intensity`, `water`
    (`default_water_level`, `original_water_level`, `f_water_level`,
    `last_f_water_level`, `flood_level`, `added_water`) and `room_rain`
    (intensity, push, splash counts, `sprite_count`, its five to seven sound
    loops' volume group, volume and whether playing, and `bullet_drips`: each
    drip's column, positions, `falling`, `fall_speed`, `delay`), `rain` (the
    cycle, its `ShaderLight`, `RainDarkPalette`,
    `ScreenShake`/`MicroScreenShake`, `death_rain_has_hit` and `global`: rain
    direction, `Intensity`, the shakes, `RumbleSound`, bullet density, flood
    and its speed, the death rain's mode, length, progression and sunlight),
    dripping sound, water surface sound, bubbles and bubble emitters,
    splashes, `rain_masks` (per realized room: `rain_mask_made`),
    `water_light`.
  - Effects, lighting and scenery: `orphan_emitters`, `lightning` (a room's
    two flash sources, thunder queue, `background_noise`; `level_params` shows
    `num4` and the sky/fog texels), `triggers`, `music`, `holo_lights`,
    `background_scene` (per scene: kind, elements, the camera's
    `camera_world`/`altitude`, each `DistantLightning`, dust puffs),
    `camera_shake`, `full_screen_effect`, `death_fall` (per realized room:
    counter, height), `lights`, `level_params`, `insects`, `slime_molds`,
    `token_stalks`, `light_beams`, `spot_lights`, `light_fixtures`,
    `waterfalls`, `light_rods`, `decals`, `rainbows`, `particles`,
    `climbable_vines` (per room: each vine's kind, points, total length),
    `zap_coils` (with `zap_coil_lights`, `zap_flashes`), `fuses`,
    `electric_death` (per room: `Intensity`, `sin`, colour, the flashes, loop
    volumes, recent strikes).
  - Superstructure and coral: `deep_processing`, `deep_processing_lights`,
    `gravity_disruptors` (with their specks), `coral_neurons` (per room: each
    `CoralNeuron`'s length, segments, ends, strands, pings, `being_climbed`),
    `coral_stems` (per room: each `CoralStem`'s root, size, frozen,
    `being_climbed`, the neurons its segments made and each `StemSegment`'s
    chunks, rod, dots, entity ID, strands), `projected_circles` (each
    `ProjectedCircle`'s radius, blink, depth, connections), `projectors` (a
    room's `SuperStructureProjector`: ideal and actual glyph, scan line and
    cursor counts, grid state, cursors), `scan_lines` (`ProjectedScanLines`),
    `star_matrices` (`ProjectedStars`: rotations, focus), `coral_systems` (per
    room: wind, age, not-viewed counter, frozen, the registry's strands and
    owners), `wall_mycelia` (strands, culled, pairs linked, linked to other
    owners, the first strand), `neuron_sparks`, `coral_circuits` (per room
    cluster: places, tile box, bit counts, displaced and repairing bits,
    links, connectors, crabs, the first displaced bit).
  - Gravity: `room_gravity` (a `ZeroG`/`BrokenZeroG` or SS_E08 gradient-script
    room's `gravity`, `electric_power`, `gradient_script`) and
    `broken_anti_gravity` (the world's switch: `on`, `counter`, `progress`,
    `lights`).
- `set_player`, `get_tiles` and `screenshot` act on the camera's room, or
  another realized room given as `room`. To test a transition, put the player
  on an exit's entrance and hold its input.
- Dev tools (not game behaviour): `set_destination` gives a realized lizard
  (by EntityID) a destination tile through the abstract AI's
  `SetDestination` and a run speed (default 0.5) its AI keeps, its
  behaviours' bodies skipped, until `release: true` hands run speed and
  destination back to them. `spawn_creature` adds a
  base-game creature (`kind` in the game's spelling) at `tile` of `room` as
  the game makes one (`GetNewID` or `EntityID(-1, seed_id)`, with their draws)
  and realizes it (`RealizeInRoom`) when the room is ready and the port has
  its body; it returns the ID and says why it stayed abstract. `spawn_object`
  adds a `Rock`/`Spear` (`explosive`) realized once the room's shortcuts are
  ready, or another loose item (`DEV_SPAWN_OBJECTS`) as a plain abstract
  object, with a note. `teleport_player` sends the player to exit `node`
  (default first) of another `room` through a vessel in the shortcut lobby,
  keeping world and session; step until `room` changes. `set_rain_timer`
  edits the rain timer: `cycle_length`, `restart`/`stop` (timer 0, running/held),
  `timer`, `shift`, `paused` (a dev flag holding only the timer; not the game's
  `RainCycle.pause`), `speed` (timer ticks per tick); `get_state.rain` reports
  the cycle, the dev flags and the global rain (D44). An edit of `cycle_length`, `restart`,
  `stop`, `timer` or `shift` that leaves the timer before the cycle's end also rewinds the rain
  (global rain, rooms' rain intensity, flood level, added water, `rainDeath`, creatures the rain kept in dens released; port-only, not the
  game's `ResetRain`, D44). While the overseer tutorial's `pauseRain` is set
  (story starts) it pins the timer at 2000 each tick, as the game does.
- `screenshot` returns a PNG from the window's draw path, offscreen. The
  debug view outlines every realized body's chunks and rods in the camera's
  room; `crop_radius` zooms on the player (or `crop_center`). With `room` it
  draws another realized room from camera `camera` (default 0), reading the
  world only; creatures there are debug bodies (no graphics modules), items
  draw sprites, the room's full-screen effect, death fall and water light draw as
  camera 0 viewing it would; water surface sprites and HUD are left out. `get_tiles` gives geometry as
  text. In a region world the uncropped view includes the HUD; `get_hud`
  reports meters, region map, text prompt (`game_over`) and the SU_C04
  controls page; `hud_message` queues a prompt message. Input fields: `map`
  (Space; map shows after 30 ticks; x/y pan, jump/throw change layers),
  `pickup` (left Shift: pick up, drop with down, switch hands by double tap,
  eat/swallow/regurgitate held), `throw` (X).
- `load_process` boots the process manager as the game does into a process
  (`intro_roll` default, `main_menu`, `slugcat_select`, `font_test`,
  `game`); `step_process` runs 1/40 s frames holding Rewired key names,
  released after the last frame. Keys feed the menus (Z/Return/Space submit,
  Escape cancel/pause, arrows) and, once the game runs, the player (arrows
  move, Z jumps, Space holds the map). Buttons see a press a tick late and
  click on release: a tap is one frame held, two released. Boot to play: any
  key after the intro roll's first second; tap Return on STORY; hold Z on
  START (~40 frames): the intro slideshow plays (~95 s; Escape skips, or hold
  S with Z on START to go straight to the game), then a new Survivor game at
  `SU_C04`, black 3 s while the start script holds the slugcat. Music runs
  silently (`TimedSources`), independent of the audio device. `step_process`
  reports slideshow clock/scene, `music`, `triggers`, the pause menu, and in
  game mode the fade delay, start script and player controller; `get_hud`
  reads the game process's HUD; `screenshot` draws the frame with its fade.
- Saves: `import_save` copies a game slot (`slot` 0-2, `game_saves` to
  override the source) into the port's save directory (`--save-dir`, default
  the platform data dir; tests use temporary directories, never the real
  ones), backing up the port's copy. The process menus read the options'
  slot (`options_menu` opens the options menu, whose import button does the
  same), so START on a saved page continues. `step_process` reports
  `options`, `save`, `story` and `karma_screen` (sleep/death/starve screen).
  The pause menu's Exit saves (death after 30 s, quit after 5).
- `--mcp --window` also shows the driven scene (agents must not use it).
- The windowed app (not MCP) has a debug UI (Dear ImGui via `rw-render`'s
  overlay; D18), F1 toggles it. Tabs: Region (stub), Teleport, Rain timer, Creatures,
  Spawn creature, Spawn object (the same actions as the MCP tools). Actions
  apply before the next frame's ticks; a focused text field takes key and
  mouse presses. A tool is a `DebugTab` in
  `crates/app/src/debug_ui/tabs.rs`.
