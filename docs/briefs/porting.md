# Porting brief

Fill the `<...>` fields. Paste the standing-rules block unedited.

## Task

Port <behaviour> from `<Type.cs>` (`/home/yura/.local/share/ilspy/split/`)
into <crate/module>. Step <n of m>: <scope in one sentence>.

- Design decisions that apply: <D-ids, AGENTS.md "Decisions" lines>.
  Design files to read: <paths>. No design file for <component>? Open
  decisions: <list>. Ask `main` instead of deciding.
- Read first: `docs/MAP.md`, `docs/mcp.md` (if you check behaviour: in a worktree use `scripts/mcp.py`, not the session's MCP tools), <crate README>, <`../rainworld-re/notes/*.md`, survey>.
- Files not to touch (other work in flight): <shared files, other agents' crates>.
- Ruled out, and by what evidence: <claim: evidence>.
- Oracle: <decompiled member / installed data file>. A match is <criterion>.
  Expected counts: <read / rejected / matching>, or "no corpus comparison".
- Tests: small and local, per ported function; expected values from the
  game's code or data, written in the test; seeded state; random-draw
  count/order tests per function. No scripted routes or whole-run fixtures.
- MCP fields to add for the new state: <names>, in `crates/app/src/mcp/`.
  Known-good spots: <room, seed, tiles, creature spawn>.
- Docs to update: <MAP.md / crate README file map / `Mirrors:` headers / divergences / OPEN-ITEMS / BUGS>;
  grep for the names you touched.
- Size: if this will not fit your context, commit a coherent part and report
  what is left. Planned split: <5a/5b or none>.
- Running now: <agents, crates, worktrees>. Your crates: <list>.
- Check with `docs/checks.md` (per-crate loop); the full workspace is the
  orchestrator's merge step.

## Standing rules (paste verbatim)

```
Rules for this task:
- AGENTS.md (loaded with your instructions) applies in full: Code style, no
  installs, no windows, never touch the game/Steam, pidof not pgrep -f,
  parking_lot locks.
- Read the game's decompile; never copy or transliterate it. Where the game
  disagrees with the brief, the game wins; say so in the report. Use cs.py
  for decompile lookups before spawning a lookup agent.
- Port every UnityEngine.Random call (sound and cosmetic too) in order and
  place.
- Omissions: list every piece of game behaviour you leave out as a question
  "port or skip for now?" in the report. Never decide to skip on your own.
- Divergences: record each in docs/divergences/ with a global ID; cite it in code.
- Work only in your own worktree. Commit there (GPG-signed; if signing
  fails, report it, never bypass). Do not merge to main.
- For lookups with a small answer after much reading, spawn read-only agents
  with model haiku (plain lookups) or sonnet (judgement). Do not spawn opus
  or fable agents.
- Keep the navigation docs true: a file you add, rename or move gets a
  `//! Mirrors: <Type>.cs` header (or `Mirrors: - (port-only: reason)`);
  `python3 scripts/check_mirrors.py` must exit 0. Every file you add, rename,
  move or delete is reflected in the README file map that covers it (crate
  or topic folder), and a README line you made untrue is corrected in the
  same change. A new crate or top-level module also updates `docs/MAP.md`.
  Do not commit the generated `docs/MIRRORS.md` / `docs/PORT-STATUS.md` (the
  orchestrator regenerates them at merge); `git checkout` them if changed.
- Run only the per-crate commands in docs/checks.md, not the workspace suite.
- Send no progress updates or status pings. One final report; a message to
  `main` (SendMessage) only when blocked on a decision you cannot make.
```

End with the report in [`report-format.md`](report-format.md).
