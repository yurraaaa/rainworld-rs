# Porting brief

Fill the `<...>` fields. Paste the standing-rules block unedited.

## Task

Port <behaviour> from `<Type.cs>` (`/home/yura/.local/share/ilspy/split/`)
into <crate/module>. Step <n of m>: <scope in one sentence>.

- Design decisions that apply: <D-ids, AGENTS.md "Decisions" lines>.
  Design files to read: <paths>. No design file for <component>? Open
  decisions: <list>. Ask `main` instead of deciding.
- Read first: `docs/MAP.md`, <crate README>, <`../rainworld-re/notes/*.md`, survey>.
- Files not to touch (other work in flight): <shared files, other agents' crates>.
- Ruled out, and by what evidence: <claim: evidence>.
- Oracle: <decompiled member / installed data file>. A match is <criterion>.
  Expected counts: <read / rejected / matching>, or "no corpus comparison".
- Tests: small and local, per ported function; expected values from the
  game's code or data, written in the test; seeded state; random-draw
  count/order tests per function. No scripted routes or whole-run fixtures.
- MCP fields to add for the new state: <names>, in `crates/app/src/mcp/`.
  Known-good spots: <room, seed, tiles, creature spawn>.
- Docs to update: <MAP.md / crate README / divergences / OPEN-ITEMS / BUGS>;
  grep for the names you touched.
- Size: if this will not fit your context, commit a coherent part and report
  what is left. Planned split: <5a/5b or none>.
- Running now: <agents, crates, worktrees>. Your crates: <list>.
- Check with `docs/checks.md` (per-crate loop); the full workspace is the
  orchestrator's merge step.

## Standing rules (paste verbatim)

```
Rules for this task:
- Read the game's decompile; never copy or transliterate it. Where the game
  disagrees with the brief, the game wins; say so in the report.
- Reimplement what the game does in every situation. Port every
  UnityEngine.Random call (sound and cosmetic too) in order and place.
- Omissions: list every piece of game behaviour you leave out as a question
  "port or skip for now?" in the report. Never decide to skip on your own.
- Divergences: record each in docs/divergences/ with a global ID; cite it in code.
- Code style: no local closures bound with let (write a method on the type);
  closures passed straight to iterator adaptors are fine. No free private
  helper functions for behaviour that belongs to a type. ExtEnums are Rust
  enums with value()/from_value(); never match on string literals. WGSL
  shaders live in crates/render/shaders/. Doc comments only for what the
  name and signature do not say; none that restate the code.
- No installs or downloads of any kind (cargo install, pip, npm -g,
  dotnet tool, curl/wget). Do not run ../rainworld-re/tools/decompile_types.sh
  unless ilspycmd already exists. Crates.io dependencies via cargo are fine;
  report which. If you need a tool, ask `main`: tool, version, location.
- The machine is shared with someone who may be playing. Never start, stop
  or kill the game or Steam. Never open windows: no windowed or fullscreen
  run, no --screenshot, no --mcp --window. Check behaviour headless with the
  MCP tools (offscreen screenshot). Never pkill -f / pgrep -f; use pidof.
- Work only in your own worktree. Commit there (GPG-signed; if signing
  fails, report it, never bypass). Do not merge to main.
- For decompile lookups with a small answer after much reading, spawn
  read-only agents with model haiku (plain lookups) or sonnet (judgement).
  Do not spawn opus or fable agents.
- Run only the per-crate commands in docs/checks.md, not the workspace suite.
- Send no progress updates or status pings. One final report; a message to
  `main` (SendMessage) only when blocked on a decision you cannot make.
```

End with the report in [`report-format.md`](report-format.md).
