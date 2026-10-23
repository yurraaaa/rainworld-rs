# AGENTS.md

## You are an orchestrator. Delegate by default

Find code via `docs/MAP.md` (crate table, links to each crate's README with
its file map). Adding a crate or module updates the map and that README.

The main session's scarcest resource is its context. The main session
decides, subagents find out, the main session checks and commits.

- **Delegate**: reading and searching (decompiled `Assembly-CSharp`, the
  installed game's data files), tracing callers and layouts, surveying
  data, full-corpus differential tests, long builds and runs, anything with
  a small answer after much looking.
- **Keep**: what to port next, whether a result is believable, bug vs.
  acceptable divergence, commit messages, all scope and design judgement.

### Briefs: write as though you will not be asked a follow-up

Copy the template from `docs/briefs/` (`porting.md`, `research.md`,
`review.md`; index in `docs/briefs/README.md`), fill every field and paste
the standing-rules block verbatim. A field you cannot fill is a decision for
the user: ask before spawning. Every agent ends with
`docs/briefs/report-format.md`. Skills: `port-survey`, `port-step`,
`merge-branch`.

Brief agents so that contradicting you is welcome: when the game's data or
code disagrees with you, the original wins.

### Models

Pass the model explicitly on every agent. Never `fable`.

- **Research** (reading/searching the decompile or game data, tracing
  callers, surveys, finding where something is defined): `haiku` (Haiku 5.5),
  or `sonnet` (Sonnet 5.5) where it needs judgement. Depend on these first.
- **Porting and judgement about the original**: `opus`.
- **Porting agents do the same**: they spawn their own `haiku`/`sonnet`
  research agents for decompile and game-data lookups instead of reading
  large decompiled files themselves, and keep their own context for writing
  and checking the port. They ask for pasted excerpts back and verify the
  key ones against the decompile before relying on them.

### Check what comes back

A subagent's success is a claim, not a result. Ask for pasted output
(decompiled excerpt, `cargo test` output), re-run the deciding command,
check that test expectations come from the game's code or data, and grep
and read every doc hit for the names it touched.

Verify with a `docs/briefs/review.md` agent given only the branch and the
oracle, not the author's conclusion.

Report exactly what was observed: "builds" is not "matches the original";
"tests pass" means the port agrees with the data those tests use, nothing
more.

### Bugs and open items

Bugs seen in play: `docs/BUGS.md`. Approved-but-unstarted work and things
to look into: `docs/OPEN-ITEMS.md`. Check both when planning.

### Document length

A document (here or in `../rainworld-re`) that would pass 1000 lines is
split into several by topic, in a folder with a `README.md` index, as
`docs/divergences/` is. Briefs repeat this for agents that write docs.

### Merging a branch

Use the `merge-branch` skill (`.claude/skills/merge-branch/SKILL.md`).
Commits are GPG-signed; if pinentry fails, ask the user, never bypass
signing. New work starts only on the user's explicit go-ahead; an answer to
another question is not one.

### Testing: small and local, the user's play is acceptance

- No long scripted input routes, no whole-run expected data (removed
  2026-10-15: too costly to maintain).
- Each ported piece gets small local tests: one function with given
  inputs, or one creature in one room for a few dozen ticks, expected values
  from the game's code or data written in the test, from its own seeded state.
- Random draws are ported exactly (order, conditions, tick), every
  `UnityEngine.Random` call including sound and cosmetic ones. Pin them with
  local draw-count/order tests per function (e.g. "`BiteFly` with n drips
  makes 4n+1 draws") and check them in review against the decompile.
- Python reference models are optional throwaway checks, never committed.
- Keep briefs short, docs minimal, screenshots only for visual work.
- Commands per crate (inner loop, which tests need the installed game, when
  the workspace suite runs): `docs/checks.md` and `scripts/check.sh <crate>`.
  Agents never run the workspace suite; the orchestrator does at merge.

### Surveys when behaviour is unclear

Only when the game's behaviour or scope is genuinely unclear: a read-only
agent writes findings, marked observed or inferred, to
`../rainworld-re/surveys/<topic>.md` (decompiled excerpts welcome) and the
user's decisions are appended at its end. Re-check key facts against the
original, bring scope and open decisions to the user with a recommendation.
Nothing new starts before the user agrees.

### Running several at once

- Agents that edit the repo each work in their own git worktree (Agent
  `isolation: "worktree"`), never the main checkout; the orchestrator
  reviews and merges. Read-only agents need none.
- Say what else is running. Cargo-lock contention: `docs/checks.md` (Parallel
  worktrees). Benchmarks run alone.
- `SendMessage` a running agent by id, telling it to reply to `main`.
- Agents send no progress updates, status pings or "waiting on X" messages:
  each costs the orchestrator context and usage. An agent sends one final
  report, or a message only when it is blocked on a decision it cannot make
  (a question for the user, a needed install). Briefs state this.
- The machine is shared with someone who may be playing. No agent starts,
  stops or kills the game or Steam.
- No agent downloads or installs programs (`dotnet tool install`, `pip
  install`, `cargo install`, `npm -g`, curl/wget binaries or scripts)
  without asking first, naming tool, version and install location. Absolute.
  `../rainworld-re/tools/decompile_types.sh` can install `ilspycmd` itself,
  so don't run it unless `ilspycmd` is already present (default
  `~/.local/share/ilspy`, or `$ILSPY_DIR`). Crates.io Cargo dependencies are
  fine; list added crates in the report.
- Agents open no windows (no windowed/fullscreen runs, no `--screenshot`,
  no `--mcp --window`). Check behaviour and visuals headless with the MCP
  tools (offscreen `screenshot`).
- Never `pkill -f`/`pgrep -f` (they match your own shell). Check for the
  game with `pidof` or `/proc/<pid>/exe`.

### The decompiled game

Read (never copy) from `/home/yura/.local/share/ilspy/split/`: one file per
top-level type (`Lizard.cs`), namespaces as folders (`HUD/`, `Menu/`,
`OverseerHolograms/`, `MoreSlugcats/`); nested types are in their parent's
file. If missing, regenerate with `../rainworld-re/tools/decompile_split.sh`
(needs the existing `~/.local/share/ilspy/ilspycmd`, never installs it).
`/home/yura/.local/share/ilspy/full/Assembly-CSharp.decompiled.cs` remains
for older notes' line numbers; new notes cite file and member.

## Decisions

- Reimplement what the game does, in every situation. Diverge only where the
  user decided otherwise or it is impossible, and record each divergence in
  `docs/divergences/`. Don't ask whether to follow the game; ask only when
  its behaviour is unknown.
- Renderer: `wgpu` pinned to Vulkan behind a small trait. Custom engine on
  `winit` plus separate audio and asset crates; the simulation is an
  engine-agnostic crate.
- Reverse-engineering notes live in `../rainworld-re`, never only in the
  conversation. Agents check it first and extend it afterwards, and say in
  the report what was added.
- Target: base game first, Remix (MMF) off. Remix, More Slugcats, Watcher
  and Jolly Co-op come later: keep their branches documented in
  `../rainworld-re`, don't design them out, list omitted DLC branches in
  `docs/divergences/`. End goal: Remix fully implemented (menu, options,
  enabling the DLCs the user owns), each DLC exactly as in the game. C# mods
  never load; the end-of-project goal is a modding API with a built-in
  scripting language (Wren, Lua, ...) replacing C# mod loading. Until then
  keep game logic behind types a script layer could hook and ExtEnums able
  to grow a modded variant. Until DLC support exists every DLC counts as not
  installed, even where the game checks files on disk (Downpour's
  `dlcsInstalled`).
- Current phase: the player controller in a region whose behaving creatures
  are overseers, bat flies and green lizards (all 9 lizard steps
  done: decisions, calm behaviours, hunting and fleeing, the player's side of a
  bite, fights, communication, missions, carrying and eating, lineages), not the full game. Details
  live in `docs/` and `../rainworld-re`; in short:
  - A story starts at the campaign's start den; room-exit shortcuts carry the
    player between the region's rooms (synchronous loads, D14). The
    population is generated with the game's draws; every creature but the
    scavengers leaves its den and runs the general or its type's abstract AI
    (D14), unrealized without a body. Green lizards, flies and overseers
    have bodies when their room is realized.
  - Lizards: path finder and movement, tracker, vision, relationships,
    noise hearing, social events, graphics, voice (D12), rock/spear hits,
    death and kills. Their AI decides as the game's (step 7a: utility
    comparer, prey/threat/aggression/rain trackers, den finder) and they walk
    on their own (idle, travel, den-going, frustrated, investigating sounds),
    hunt (approach, lounge, bite and hold) and
    flee (7b); rivals fight, signal dominance and submission, and a lizard
    that submits leaves the room on a mission (7c). Their deaths register with
    their spawners (lineages progress on the slugcat's kills; respawn and wait
    lists carried across cycles and saves); lizards stay in shelters and are
    saved there (step 9).
  - Overseers: body, AI, graphics, guide tutorial text, Monk's guide light,
    holograms whose triggers are ported (D14). Bat flies: world process,
    realization, flocks, hives, chains, pipe travel, sounds, graphics.
  - Player: catches, holds, throws, eats flies (SU_A42's Eat lesson, then
    SU_A43's super-jump); picks up, carries, swaps, drops, swallows,
    regurgitates and throws rocks and spears; the Hunter carries one spear on its
    back (D3); drags dead lizards (through shortcuts, into shelters), which
    the Hunter eats (D3). Bitten by a lizard (step 8,
    D19): held in its jaws (stunned, `dangerGrasp`), struggles free with a
    throw (a weapon at the grabber) or grabs a weapon, dies of a hurting bite;
    60 ticks held is `GameOver(dangerGrasp)`, cancelled if it gets free.
  - Saves (`rw-save`, D16): Continue, New Game wipe, start's death-and-quit
    save. Shelters (D17): door closes on a fed, still slugcat, `Win` writes
    the save and region state, the sleep screen (D13) continues the next
    cycle; too little food shows the starve screen. PASSAGE stays greyed out
    (D13). Dreams are a black stand-in. Gates, region changes and the rain
    are not there yet.
  - Death in a story session is the game's: the body keeps simulating,
    `GameOver` puts the HUD prompt in game-over mode, the map button 40 ticks
    later saves the death and shows the death screen, whose CONTINUE starts
    the next cycle. Dev starts (`load_room`, `--room`, arena levels) game
    over the same way on an unsaved state (D4, D16), and only there R
    respawns, leaving game-over mode. A body that falls out of the room is destroyed.
  - Water uses the game's full wavy surface simulation.
- Game assets inside Unity files (atlases, noise texture, audio) are read
  natively at startup by `rw_assets::unity`
  (`../rainworld-re/notes/unity-assets.md`); no asset cache. AssetRipper
  (`~/.local/share/assetripper/2.0.0/`,
  `../rainworld-re/tools/extract_atlases.sh`) is a debugging aid and the
  oracle of the reader's tests; never commit game assets.
- Save data lives in the port's own directory (`directories`:
  `~/.local/share/rainworld-rs/`, `%APPDATA%\rainworld-rs\`, behind a small
  platform seam; Linux and Windows are targets); the port never writes to
  the real game's save storage. Files use the game's text format; imported
  DLC, Remix and mod data is kept untouched, never stripped. The importer
  copies one slot (`sav`, `sav2`, `sav3`) read-only from the game's
  `persistentDataPath` (on Linux the Proton prefix under `RW_DIR`'s
  `steamapps/compatdata/312520`, overridable by flag), triggered by the
  options menu's IMPORT SAVE FROM RAIN WORLD button (divergence D13) or
  MCP's `import_save`.
- Creature graphics live in `rw-graphics` (cosmetic simulation: tail, limbs,
  head, frame choice; depends on `rw-sim` and `rw-assets`; outputs a
  backend-agnostic sprite list that `rw-render` draws). `rw-sim` stays
  gameplay-only.
- Audio: `rw-audio` on `kira`. FSB5 Vorbis clips need FMOD's Vorbis setup
  headers; the port ships that table (source and licence in
  `../rainworld-re/notes/sound.md`). The sim makes the game's random draws
  for every sound (about 7 per one-shot) so the shared UnityRandom stream
  stays in step even with audio off.
- Android port at the end: keep platform-specific code (windowing, game-data
  paths, input) behind small seams, prefer Android-capable crates, avoid
  desktop-only assumptions in sim, assets and render. Touch input and
  getting game files onto the device are open.
- Cleanup, review and optimisation is its own milestone (M14), after Remix
  and the DLCs (M13): a code cleanliness/quality review across the workspace,
  then an optimisation pass. Milestones: `docs/MILESTONES.md`.

## Driving the app

`rainworld-rs --mcp` is an MCP server on stdio (`crates/app/src/mcp/`),
registered in `.mcp.json` as `rainworld-rs` (runs `cargo run --release`, so
it serves the checkout's current code). Tools: `list_rooms`, `load_room`,
`new_game`, `step`, `step_sequence`, `respawn`, `set_player`, `get_state`,
`get_exits`, `get_tiles`, `get_hud`, `hud_message`, `screenshot`,
`load_process`, `step_process`, `import_save`, `set_destination`,
`spawn_creature`, `spawn_object`, `teleport_player`. To check behaviour,
prefer these over `--screenshot`/`--script`; briefs for such agents say so.

- Headless and deterministic: the sim advances only in `step` /
  `step_sequence` (40 ticks/s); same start, seed and inputs give the same run.
  `load_room` and `new_game` start a fresh music player (it draws from the game's
  random stream), so runs repeat within one server process too.
- `load_room` loads a room's region world with the player in it (arena
  levels: a single-room world) as `slugcat` (default `White`); `new_game`
  starts a campaign (`White`, `Yellow`, `Red`; `malnourished` option) at its
  start den.
- `get_state` reports the camera room, the player (food, grasps, pick-up
  candidate, stomach, counters, sneak/loudness, `held_by`, `danger_grasp` and
  its time, struggle buttons, `can_struggle`, `cant_be_grabbed_counter`, ...), the vessel in the
  lobby, realized rooms' entry waits, ceiling drips, dripping sound, water surface sound, bubbles and bubble emitters, splashes, the player's `owned_items` on the ground, every abstract creature (kind, ID, room,
  node, tile, den, realized, alive, abstract AI; realized flies, lizards
  with their movement, tracker, relationships, noise, decision (`social`:
  fight target and anger, `submitted_to`, last signal, missions, `taming_difficulty`, `gift`) and, in a
  viewed room, graphics and voice; overseers with AI and guide state; stun/kill tag,
  `social`, `danger`, `carries`), every abstract object (and realized
  state), `orphan_emitters`, `lightning` (a room's two flash sources, thunder queue, `background_noise`; `level_params` shows `num4` and the sky/fog texels), `triggers`, `music`, `holo_lights`, `background_scene` (per scene: kind, elements, the camera's `camera_world`/`altitude`, each `DistantLightning`, dust puffs), `camera_shake`, `full_screen_effect`, `death_fall`, `water_light`, `lights`, `level_params`, `insects`, `slime_molds`, `token_stalks`, `light_beams`, `spot_lights`, `light_fixtures`, `waterfalls`, `electric_death` (per room: `Intensity`, `sin`, colour, the flashes, loop volumes, recent strikes), `border_push_back` (per room: the edges, `pushPower`), `decals`, `rainbows`, `particles`, `session`,
  `dens`, `quantified`, `flies_world`, `region_state`, `heavy_ai`, `room_gravity` (a `ZeroG`/`BrokenZeroG` room's `gravity` and `electric_power`) and `broken_anti_gravity` (the world's switch: `on`, `counter`, `progress`, `lights`). Empty fields are left
  out; the field list is in `crates/app/src/mcp/`. New sim state must be
  added there. `get_exits` lists the camera room's exits (entrance tile, the
  input that enters, destination room, arrival node, camera).
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
  keeping world and session; step until `room` changes.
- `screenshot` returns a PNG from the window's draw path, offscreen. The
  debug view outlines every realized body's chunks and rods in the camera's
  room; `crop_radius` zooms on the player (or `crop_center`). With `room` it
  draws another realized room from camera `camera` (default 0), reading the
  world only; creatures there are debug bodies (no graphics modules), items
  draw sprites, water and HUD are left out. `get_tiles` gives geometry as
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
  silently (`TimedSources`) so runs stay deterministic. `step_process`
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
  overlay; D18), F1 toggles it. Tabs: Region (stub), Teleport, Creatures,
  Spawn creature, Spawn object (the same actions as the MCP tools). Actions
  apply before the next frame's ticks; a focused text field takes key and
  mouse presses. A tool is a `DebugTab` in
  `crates/app/src/debug_ui/tabs.rs`.

## Code style

Briefs must repeat these rules.

- No local closures bound inside functions (`let at = |i| ...;`): write a
  method on the owning type. Closures passed straight to iterator adaptors
  or APIs (`.map(|c| c.pos)`) are fine.
- No free private helper functions for behaviour that belongs to a type
  (`fn lerp(last: Vec2, ...)`): implement it on that type, or on a trait for
  a foreign type.
- The game's ExtEnums (`CreatureTemplate.Type`, `SlugcatStats.Name`, ...) are
  Rust enums with a variant per base-game and DLC value, plus
  `value()`/`from_value()` for the game's spelling. Never match or compare on
  string literals.
- Every source file's module header has a `//! Mirrors: <Type>.cs[, ...]
  [(Member, ...)]` line naming the decompiled file(s) it ports, or `//!
  Mirrors: - (port-only: reason)`. A new or renamed file keeps it right;
  `python3 scripts/check_mirrors.py` must exit 0 (it also regenerates
  `docs/MIRRORS.md`; `python3 scripts/port_status.py` regenerates
  `docs/PORT-STATUS.md`). The same script fails when a source file is not
  named in its crate's `README.md`; a change that makes a README line untrue
  corrects it in the same change.
- WGSL shaders live in `crates/render/shaders/`, loaded with
  `include_str!("../shaders/<name>.wgsl")`.
- Don't over-document. A simple function or field needs no doc comment, or
  one short line. Document only non-obvious behaviour, units, the game
  behaviour mirrored, invariants. No comments that restate the code.
