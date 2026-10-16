# AGENTS.md

## You are an orchestrator. Delegate by default

The main session's scarcest resource is its context, and investigation
burns it fastest. The main session decides, subagents find out, the main
session checks and commits.

- **Delegate**: reading and searching (decompiled `Assembly-CSharp`, the
  installed game's data files), tracing callers and layouts, surveying
  data files, full-corpus differential tests, long builds and runs,
  anything with a small answer after much looking.
- **Keep**: what to port next, whether a result is believable, whether a
  difference is a bug or an acceptable divergence, commit messages, every
  judgement about scope or design decisions.

### Write the brief as though you will not be asked a follow-up

A brief names:

- **The design decisions** that apply, and which design files to read. For
  a component without a design file, say so and list the open decisions so
  the agent asks instead of deciding.
- **Files not to touch** while other work is in flight; shared files
  (workspace `Cargo.toml`, a crate's `lib.rs`) are where agents collide.
- **What is ruled out, and by what evidence.**
- **The reference and the comparison**: which installed game files, which
  original function or data is the oracle, what counts as a match, and the
  read/rejected/matching counts expected.
- **Sources**: the game is read, never copied or transliterated.
- **The docs it must update**, and to grep for the rest. A doc left stale
  by a delegated change is the orchestrator's fault.

### Pick the model by the work

Pass the model explicitly on every agent. Read-only work (surveys,
searches): `haiku`, or `sonnet` where it needs judgement. Porting and
judgement about the original: `opus`. Never `fable`.

### Check what comes back

A subagent's success is a claim, not a result. Ask for pasted output (the
decompiled excerpt, the `cargo test` output), re-run the deciding command,
check the test's expectations come from the game's code or data, and grep and
read every doc hit for the names it touched.

In reports to the user, say exactly what was observed: "builds" is not
"matches the original"; "the tests pass" means the port agrees with the
data those tests use, on the installed build, nothing more.

Brief agents so that contradicting you is welcome: when the game's data or
code disagrees with you, the original wins.

### Testing: small and local, the user's play is acceptance

Speed matters; tests must not become a maintenance cost.

- No long scripted input routes and no whole-run expected data (regenerated
  traces, model fixtures). Their maintenance slowed development; they were
  removed on 2026-10-15.
- Each ported piece gets small local tests: one function with given
  inputs, or one creature in one room for a few dozen ticks, with expected
  values from reading the game's code or data and written in the test. A
  test starts from its own seeded state, so unrelated stream shifts don't
  break it.
- Random draws are still ported exactly (order, conditions, tick), every
  `UnityEngine.Random` call including sound and cosmetic ones, so the port
  stays close to the original. Pin them with local draw-count/order tests
  per function (e.g. "`BiteFly` with n drips makes 4n+1 draws"), and check
  them in review against the decompile.
- Python reference models are optional, throwaway checks for hard
  draw-heavy code, never committed test data.
- The user's hand review in the game is the acceptance test.
- Keep briefs short, docs minimal, screenshots only for visual work; run
  independent features in parallel worktrees.

### Surveys when behaviour is unclear

Survey only when the game's behaviour or the scope is genuinely unclear: a
read-only agent writes findings, marked observed or inferred, to a scratch
file. Re-check key facts against the original, then bring the scope and
open decisions to the user with a recommendation. Nothing new is started
before the user agrees.

### Running several at once

- Parallel agents that edit the repo each work in their own git worktree
  (the Agent tool's `isolation: "worktree"`), never in the main checkout.
  The orchestrator reviews and merges each branch. Read-only agents need
  no worktree.
- Say what else is running. Agents sharing `target/` serialise on cargo's
  lock and break each other's builds: give parallel agents disjoint crates
  or worktrees. Benchmarks run alone; say so in the brief.
- `SendMessage` to a running agent by id, telling it to reply to `main`
  (replies to unknown names are lost).
- Tell agents the machine is shared with someone who may be playing. No
  agent starts, stops or kills the game or Steam.
- No agent downloads or installs programs (`dotnet tool install`,
  `pip install`, `cargo install`, `npm -g`, binaries or scripts fetched
  with curl/wget from websites, etc.) without asking first. Put the
  request in the report or ask via `main`, naming the tool, version and
  where it would be installed. Briefs must say this. This is absolute:
  `../rainworld-re/tools/decompile_types.sh` can install `ilspycmd` itself
  if it is missing, so agents must not run it unless `ilspycmd` is already
  present (default `/tmp/ilspy`, or `$ILSPY_DIR`); otherwise ask first.
  Library crates from crates.io added as Cargo dependencies and fetched
  by cargo are fine; say in the report which crates were added.
- Agents don't open windows (no windowed or fullscreen runs, no
  `--screenshot` runs, no `--mcp --window`): the user is at the machine
  and repeated windows get in the way. Check behaviour and visuals
  headless with the MCP tools (offscreen `screenshot`). Briefs must say
  this.
- Never match your own command line: `pkill -f`/`pgrep -f` match the shell
  running them and can kill the session. Check for the game with `pidof`
  or `/proc/<pid>/exe`.

## Decisions

- Reimplement what the game does, in every situation. Diverge only where
  the user decided otherwise (below) or it is impossible, and record each
  divergence in `docs/DIVERGENCES.md`. Don't ask whether to follow the
  game; ask only when the game's behaviour is unknown.
- Renderer: `wgpu`, pinned to the Vulkan backend, behind a small trait so it can be replaced. Custom engine on `winit` plus separate audio and asset crates; the simulation is an engine-agnostic crate.
- Reverse-engineering notes live in `../rainworld-re`, never only in the conversation. Agents check it before investigating and extend it afterwards; ask in the report what was added.
- Target: base game first, with Remix (MMF) off. Remix, More Slugcats,
  Watcher and Jolly Co-op are to be supported later, so keep their
  branches documented in `../rainworld-re` notes and don't design them
  out; omitted DLC branches are listed in `docs/DIVERGENCES.md`.
  End goal: Remix fully implemented (its menu and options, enabling the
  DLCs the user owns), with each DLC taking effect exactly as in the
  game. C# code mods will never load. An end-of-project goal is a proper
  modding API with a built-in scripting language (Wren, Lua or another,
  chosen when it's time) replacing the game's C# mod loading; until then
  don't design it out (keep game logic behind types a script layer could
  hook, and ExtEnums able to grow a modded variant later). Until DLC support exists the port treats every DLC as not installed,
  even where the game decides from files on disk (Downpour's
  `dlcsInstalled` check): the base-game menu scene and intro images.
- Current phase is the player controller in a region whose only
  behaving creatures are the overseers and the bat flies, not the full
  game (campaign step 1, overseer step 1, batflies done):
  - A story starts at the campaign's start den in its region's world;
    room-exit shortcuts carry the player between the region's rooms, which
    load synchronously (D14). The population is generated with the
    game's draws but only overseers leave their dens; overseers have their
    body, AI, graphics, the guide's tutorial text and their holograms
    (those whose triggers are ported: D14). The bat flies' world
    process moves their per-node counts between rooms, and a room that
    loads realizes its flies: body, AI, flocks, hives, chains, pipe
    travel, sounds and graphics. The player catches them (reaching, the
    looker), holds, throws, carries them through pipes and eats them
    (bites, drips, food and the food meter), under the tutorial's Eat
    instruction in SU_A42; fed, the guide moves on to SU_A43's super-jump
    lesson. Rocks and spears lie in the rooms; the player picks them up,
    carries them (through pipes and room exits), switches hands, lays them
    down and swallows and regurgitates rocks; throwing them only drops
    them until it is ported (D3). Saves (`rw-save`, D16): Continue from the slugcat select, New
    Game's wipe and the start's death-and-quit save. Shelters (D17): the
    door closes on a fed, still slugcat, `Win` writes the whole save state
    and region state, the sleep screen (karma ladder, food meter, map;
    D13) continues to the next cycle in the shelter with the saved
    population; a door closing on too little food shows the starve screen.
    The screens show the passage meters and tokens; PASSAGE stays greyed
    out (D13). Dreams are a black stand-in. Gates, region changes and the
    rain are not there yet.
  - Death in a story session is the game's: the dead body keeps
    simulating, `GameOver` puts the HUD prompt in game-over mode (and
    stores the karma flower's spot), and the map button 40 ticks later
    saves the death and switches to the death screen, whose CONTINUE
    starts the next cycle. R respawns only in dev starts (`load_room`, `--room`,
    arena levels), which have no game over. A body that falls out of the
    room is destroyed, as in the game.
  - Water uses the game's full wavy surface simulation.
- Game assets inside Unity files (sprite atlases, the noise texture,
  audio) are read natively at startup from the installed game's Unity files
  by `rw_assets::unity` (`../rainworld-re/notes/unity-assets.md`); there is
  no asset cache. AssetRipper (`~/.local/share/assetripper/2.0.0/`,
  `../rainworld-re/tools/extract_atlases.sh`) remains a debugging aid and
  the oracle of the reader's installed tests; never commit game assets.
- The port's save data lives in its own directory, never in the real
  game's save storage, and the port never writes there. It can import
  (read-only, copying into its own directory) the real game's save data.
  - The save files use the game's own text format. Imported DLC, Remix
    and mod data is kept untouched and ignored, as the game does; never
    stripped.
  - Its directory is the platform data dir behind a small platform seam
    (`directories`: `~/.local/share/rainworld-rs/`, `%APPDATA%\rainworld-rs\`);
    Linux and Windows are targets. The importer reads one slot file (`sav`,
    `sav2`, `sav3`) from the game's `persistentDataPath` (on Linux the
    Proton prefix found from `RW_DIR`'s `steamapps/compatdata/312520`),
    with a flag to override. Its trigger is the options menu's IMPORT SAVE
    FROM RAIN WORLD button (a recorded divergence, D13) and MCP's `import_save`.
- Slugcat (and later creature) graphics live in `rw-graphics`: the
  cosmetic graphics simulation (tail, limbs, head, frame choice) depends
  on `rw-sim` and `rw-assets` and outputs a backend-agnostic sprite list
  that `rw-render` draws. `rw-sim` stays gameplay-only.

## Driving the app

`rainworld-rs --mcp` is an MCP server on stdio (`crates/app/src/mcp/`),
registered in `.mcp.json` as `rainworld-rs` (it runs `cargo run --release`,
so it serves the checkout's current code). Tools: `list_rooms`,
`load_room`, `new_game`, `step`, `step_sequence`, `respawn`, `set_player`,
`get_state`, `get_exits`, `get_tiles`, `get_hud`, `hud_message`,
`screenshot`, `load_process`, `step_process`, `import_save`.

- Headless and deterministic: the sim advances only in `step` /
  `step_sequence` (40 ticks per second); the same start, seed and inputs
  give the same run.
- `load_room` loads the room's region world with the player in that room
  (arena levels: a single-room world), as `slugcat` (default `White`); `new_game` starts a campaign
  (`White`, `Yellow`, `Red`) at its start den (`malnourished` starts it
  malnourished). `get_state` reports the
  camera's room, the player (with its food, quarter pips, grasps and
  pick-up candidate as `{kind, id}`, stomach, swallow counter, eat counter, malnourishment, aerobic level and exhaustion), the player's room and node, the vessel waiting in the
  lobby, the realized rooms' entry waits and `creatures` (every
  abstract creature: kind, ID, room, node, tile, in den, realized,
  alive; overseers also their AI's destination, guide state, last rooms
  and realized tiles; realized flies their room, position, velocity,
  behaviour, movement mode, grasps and holders) and `objects` (every
  abstract rock and spear: kind, ID, room, node, tile, a spear's
  stuck-in-wall cycles and explosive flag; realized ones also their
  room, position, velocity, mode, rotation and collision layer); `get_exits` lists the
  camera room's exits (entrance tile, the input that enters, destination
  room, arrival node and camera). `set_player`, `get_tiles` and
  `screenshot` act on the camera's room. To test a transition, put the
  player on an exit's entrance and hold its input.
- `screenshot` returns a PNG drawn by the window's draw path (level
  graphic or debug view, sprites, overlays), rendered offscreen;
  `crop_radius` zooms on the player (or on `crop_center`). `get_tiles` gives the geometry as
  text. In a region world the uncropped visuals view includes the HUD;
  `get_hud` reports its meters (the food meter's `quarter_pips` too, the
  text prompt's `game_over`),
  the region map (pan, layer, pending reveal, player marker, gate and
  shelter markers), the text prompt and the SU_C04 controls page,
  `hud_message` queues a text-prompt message, and the `map` input field
  (Space in the window) holds the map button (the map shows after 30
  ticks; x/y pan it, jump and throw change layers); `pickup` (left Shift) picks up, drops (with down),
  switches hands (double tap), eats, swallows and regurgitates (held),
  `throw` (X) throws.
- To check behaviour, prefer these tools over `--screenshot`/`--script`.
  Briefs for agents that check behaviour should say so.
- `load_process` boots the process manager as the game does (black,
  Initialization, fade) into a process (`intro_roll` by default, then
  `main_menu`, `slugcat_select`, `font_test`, `game`); `step_process` runs
  1/40 s frames holding Rewired key names, released after the last frame.
  The keys feed the menus (UI map: Z/Return/Space submit, Escape cancel and
  pause, arrows) and, once the game runs, the player (gameplay map: arrows
  move, Z jumps, Space holds the map button). Buttons see a press a tick after it and click on release,
  so a tap is one frame held and two released. Boot to play: any key after
  the intro roll's first second goes to the main menu; tap Return on
  STORY; hold Z on START (about 40 frames): the Survivor's intro
  slideshow plays (about 95 s; Escape skips it, or hold S with Z on START
  to go straight to the game), then a new Survivor game at `SU_C04`, black
  for 3 s while the start script holds the slugcat. Music runs silently
  (`TimedSources`), so runs stay deterministic; the slideshow follows its
  ticks. `step_process` reports the slideshow's clock and scene and, in
  the game, the pause menu (Escape opens it; its buttons, selection and
  fade); `get_hud` reads the game process's HUD; in
  game mode `get_state` reports the fade delay, the SU_C04 start script
  and the player's controller. `screenshot` then draws the process frame with its fade.
- Saves: `import_save` copies one of the game's slots (`slot` 0-2,
  `game_saves` to override where) into the port's save directory
  (`--save-dir`, default the platform data dir; tests use temporary
  directories, never the real ones), backing up the port's copy;
  `load_process`'s menus read the options' slot (`options_menu` opens the
  options menu, whose import button does the same for the selected slot;
  `step_process` reports `options`: values, screen, info label, dialog),
  so START on a saved page continues
  (`step_process` reports `save`: saved campaigns and the current save
  state, `story`: the running game's den, cycle, karma, room and food,
  and `karma_screen`: the sleep, death or starve screen's ladder phase,
  karma, buttons, map, end-game meters and passage tokens);
  the pause menu's Exit saves (a death after 30 s, a quit after 5).
- `--mcp --window` also shows the driven scene in a window.
- End of the project: a review of code cleanliness and quality across the
  workspace, then an optimisation pass, once the whole game is ported
  (not after individual milestones).
- Long-term goal: an Android port at the end of the project. Don't rule it
  out: keep platform-specific code (windowing, file paths to the game data,
  input) behind small seams, prefer crates that support Android (wgpu's
  Vulkan backend and winit do), and avoid desktop-only assumptions in the
  sim, assets and render crates. Touch input and getting the user's game
  files onto the device are open questions for later.
- Audio: `rw-audio` on `kira`. FSB5 Vorbis clips need FMOD's Vorbis setup
  headers; the port ships that header table (record its source and licence
  in `../rainworld-re/notes/sound.md`). The sim makes the game's random
  draws for every sound it plays (about 7 per one-shot), so the shared
  UnityRandom stream stays in step with the game even when audio is off.

## Code style

Briefs must repeat these rules.

- No local closures bound inside functions (e.g. `let at = |i| ...;`).
  Write a method on the type the logic belongs to instead. Closures passed
  straight to iterator adaptors or APIs (`.map(|c| c.pos)`) are fine.
- No free private helper functions for behaviour that belongs to a type
  (e.g. `fn lerp(last: Vec2, cur: Vec2, t: f32)`); implement it on that
  type (`impl Vec2 { pub fn lerp(self, to: Vec2, t: f32) -> Vec2 }`), or
  on a trait for a foreign type.
- The game's ExtEnums (`CreatureTemplate.Type`, `SlugcatStats.Name`, ...)
  are Rust enums with a variant per base-game and DLC value, plus
  `value()`/`from_value()` for the game's spelling. Never match or compare
  on string literals.
- WGSL shaders live in `crates/render/shaders/`, not among the Rust
  sources; load them with `include_str!("../shaders/<name>.wgsl")`.
- Don't over-document. A simple function or field needs no doc comment,
  or at most one short line. Document only what the name and signature
  don't already say: non-obvious behaviour, units, the game behaviour it
  mirrors, invariants. No comments that restate the code.
