# CLAUDE.md

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
decompiled excerpt, the `cargo test` output with corpus counts, the diff of
mismatches), re-run the deciding command, check the test compares against
the original and not a second copy of the agent's reading, and grep and
read every doc hit for the names it touched.

In reports to the user, say exactly what was observed: "builds" is not
"matches the original"; "the tests pass" means the port agrees with the
data those tests use, on the installed build, nothing more.

Brief agents so that contradicting you is welcome: when the game's data or
code disagrees with you, the original wins.

### Surveys before milestones

A milestone starts with a survey: a read-only agent (no repo edits, no
cargo) writes findings, marked observed or inferred, to a scratch file.
Re-check three or four key facts against the original, then bring the
scope, "done when" and each open decision to the user with a
recommendation. Nothing is ported before the user agrees.

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
  game. Code mods are out of scope until the port has its own modding
  API. Until DLC support exists the port treats every DLC as not installed,
  even where the game decides from files on disk (Downpour's
  `dlcsInstalled` check): the base-game menu scene and intro images.
- Current phase is the player controller in a region whose only
  behaving creatures are the overseers, not the full game (campaign
  step 1, overseer step 1):
  - A story starts at the campaign's start den in its region's world;
    room-exit shortcuts carry the player between the region's rooms, which
    load synchronously (D14). The population is generated with the
    game's draws but only overseers leave their dens (abstract AI only;
    a realized overseer is a frozen placeholder). Gates, region changes,
    the rain and saves are not there yet.
  - Death freezes the body until R respawns it (revisit once the real
    game loop exists). A body that falls out of the room is destroyed,
    as in the game.
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
- Slugcat (and later creature) graphics live in `rw-graphics`: the
  cosmetic graphics simulation (tail, limbs, head, frame choice) depends
  on `rw-sim` and `rw-assets` and outputs a backend-agnostic sprite list
  that `rw-render` draws. `rw-sim` stays gameplay-only.

## Driving the app

`rainworld-rs --mcp` is an MCP server on stdio (`crates/app/src/mcp/`),
registered in `.mcp.json` as `rainworld-rs` (it runs `cargo run --release`,
so it serves the checkout's current code). Tools: `list_rooms`,
`load_room`, `new_game`, `step`, `step_sequence`, `respawn`, `set_player`,
`get_state`, `get_exits`, `get_tiles`, `screenshot`, `load_process`,
`step_process`.

- Headless and deterministic: the sim advances only in `step` /
  `step_sequence` (40 ticks per second); the same start, seed and inputs
  give the same run.
- `load_room` loads the room's region world with the player in that room
  (arena levels: a single-room world); `new_game` starts a campaign
  (`White`, `Yellow`, `Red`) at its start den. `get_state` reports the
  camera's room, the player's room and node, the vessel waiting in the
  lobby, the realized rooms' entry waits and `creatures` (every
  abstract creature: kind, ID, room, node, tile, in den, realized,
  alive; overseers also their AI's destination, guide state, last rooms
  and realized tiles); `get_exits` lists the
  camera room's exits (entrance tile, the input that enters, destination
  room, arrival node and camera). `set_player`, `get_tiles` and
  `screenshot` act on the camera's room. To test a transition, put the
  player on an exit's entrance and hold its input.
- `screenshot` returns a PNG drawn by the window's draw path (level
  graphic or debug view, sprites, overlays), rendered offscreen;
  `crop_radius` zooms on the player. `get_tiles` gives the geometry as
  text.
- To check behaviour, prefer these tools over `--screenshot`/`--script`.
  Briefs for agents that check behaviour should say so.
- `load_process` boots the process manager as the game does (black,
  Initialization, fade) into a process (`intro_roll` by default, then
  `main_menu`, `slugcat_select`, `font_test`, `game`); `step_process` runs
  1/40 s frames holding Rewired key names, released after the last frame.
  The keys feed the menus (UI map: Z/Return/Space submit, Escape cancel and
  pause, arrows) and, once the game runs, the player (gameplay map: arrows
  move, Z jumps). Buttons see a press a tick after it and click on release,
  so a tap is one frame held and two released. Boot to play: any key after
  the intro roll's first second goes to the main menu; tap Return on
  STORY; hold Z on START (about 40 frames): the Survivor's intro
  slideshow plays (about 95 s; Escape skips it, or hold S with Z on START
  to go straight to the game), then a new Survivor game at `SU_C04`, black
  for 3 s while the start script holds the slugcat. Music runs silently
  (`TimedSources`), so runs stay deterministic; the slideshow follows its
  ticks. `step_process` reports the slideshow's clock and scene; in
  game mode `get_state` reports the fade delay, the SU_C04 start script
  and the player's controller. `screenshot` then draws the process frame with its fade.
- `--mcp --window` also shows the driven scene in a window.
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
- WGSL shaders live in `crates/render/shaders/`, not among the Rust
  sources; load them with `include_str!("../shaders/<name>.wgsl")`.
- Don't over-document. A simple function or field needs no doc comment,
  or at most one short line. Document only what the name and signature
  don't already say: non-obvious behaviour, units, the game behaviour it
  mirrors, invariants. No comments that restate the code.
