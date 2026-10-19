# Report format

Every agent ends with exactly these headings, in order. Say what was
observed, not what was intended: "builds" is not "matches the original".
Paste output; do not paraphrase it. Write "none" for an empty heading.

1. **Summary**: two or three lines: what was done, what state it is in
   (branch, worktree path, last commit hash).
2. **Files changed**: absolute paths, one line each on what changed.
3. **Commands run**: each command with its pasted result lines
   (`test result:` lines from `cargo test`, clippy and fmt status, the
   deciding command's output). Mark anything not run.
4. **Comparison** (if the work compared against the game): oracle used
   (file or decompiled member), counts read / rejected / matching.
5. **Notes added to `../rainworld-re`**: paths, and whether committed.
6. **Crates added**: name and version, or none.
7. **Omissions, as questions**: each piece of game behaviour left out, as
   "port or skip for now? <what, where in the game, why it was out of reach>".
   Never skip silently; never decide a skip yourself.
8. **Open questions**: things only the user or orchestrator can decide.
9. **Divergences recorded**: ID and file in `docs/divergences/`, or none.
10. **Docs updated**: files touched, and greps run for stale names.
11. **Left over**: if context ran short, what is committed and what remains.

Read-only agents (research, review) keep headings 1, 3, 7 and 8 and add
**Findings** (each marked observed or inferred) or **Verdict** (see `review.md`).
