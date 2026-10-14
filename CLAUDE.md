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
- Never match your own command line: `pkill -f`/`pgrep -f` match the shell
  running them and can kill the session. Check for the game with `pidof`
  or `/proc/<pid>/exe`.

## Decisions

- Renderer: `wgpu`, pinned to the Vulkan backend, behind a small trait so it can be replaced. Custom engine on `winit` plus separate audio and asset crates; the simulation is an engine-agnostic crate.
- Reverse-engineering notes live in `../rainworld-re`, never only in the conversation. Agents check it before investigating and extend it afterwards; ask in the report what was added.
