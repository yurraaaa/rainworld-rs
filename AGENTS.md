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

`rg` (ripgrep) is installed. Read the decompile with `../rainworld-re/tools/cs.py`
instead of whole files (Player.cs is 474 KB); usage in `../rainworld-re/README.md`:

- `cs.py outline <Type>`: member line ranges.
- `cs.py show <Type.Member> [--n K]`: one member's source (overloads all, or the K-th).
- `cs.py calls <Type.Member> [regex]`: matching lines in order, default `Random\.`;
  a line with several matches shows `(xN)`, count those for draw-count pins.
- `cs.py callers <Name> [--in Type]`, `cs.py find <Name>`.

## Decisions

- Reimplement what the game does, in every situation. Diverge only where the
  user decided otherwise or it is impossible, and record each divergence in
  `docs/divergences/`. Don't ask whether to follow the game; ask only when
  its behaviour is unknown.
- Loading is threaded as in the game (user, 2026-10-24): rooms (`RoomPreparer`) and
  regions (`WorldLoader`) load on worker threads and hand over to the main thread
  by flags, as the game's do; the main thread makes every random draw. Absolute
  determinism is not a goal: don't add deterministic modes or schedules for it.
  Tests stay small and local on seeded state.
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
- Current phase: the player controller in a region of overseers, bat flies and green
  lizards (all 9 lizard steps done), not the full game. What exists, what does not:
  `docs/STATUS.md`.
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

`rainworld-rs --mcp` is an MCP server (`crates/app/src/mcp/`) registered in `.mcp.json` as
`rainworld-rs`. Agents check behaviour headless with its tools (not `--screenshot`/`--script`;
briefs for such agents say so) and read `docs/mcp.md` before using them. Never `--window`.
The session's `mcp__rainworld-rs__*` tools run main's code from session start, shared by all
agents: an agent in a worktree drives its own build with `scripts/mcp.py` instead.

## Code style

Briefs do not repeat these rules: every subagent loads this file. A brief's standing-rules block
carries only the rules this file does not state.

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
- Unit tests live in a `tests/` folder inside their topic folder (`player/tests/`), never
  as `*_tests.rs` beside the sources; a crate's top-level `tests/` holds integration
  tests only. (Existing `*_tests.rs` files move in the file layout cleanups.)
- Locks: `parking_lot`'s `Mutex`, `RwLock` and `Condvar`, not `std::sync`'s (user,
  2026-10-24: lighter and faster). `std::thread` and `Arc` are fine.
- WGSL shaders live in `crates/render/shaders/`, loaded with
  `include_str!("../shaders/<name>.wgsl")`.
- Don't over-document. A simple function or field needs no doc comment, or
  one short line. Document only non-obvious behaviour, units, the game
  behaviour mirrored, invariants. No comments that restate the code.
