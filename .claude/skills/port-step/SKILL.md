---
name: port-step
description: Orchestrate one porting step of rainworld-rs end to end, from brief to merge. Use when the user gives the go-ahead to port the next piece of game behaviour (a creature step, a system, a screen), or asks to delegate a port.
---

# One porting step

You decide scope and judge results; agents find out and write. Needs the
user's explicit go-ahead first.

1. Read the survey/notes for the step (`../rainworld-re/surveys/`,
   `notes/`), `docs/MAP.md`, `docs/OPEN-ITEMS.md`, `docs/BUGS.md`. If the
   behaviour is unclear, run `port-survey` first.
2. Size it: a step one agent cannot finish in one context is split
   (5a graphics, 5b cosmetics). Pick the crates so parallel steps do not
   share one; shared files (workspace `Cargo.toml`, a crate's `lib.rs`) go
   to one agent at a time. Check what is running.
3. Brief: copy `docs/briefs/porting.md`, fill every field, paste the
   standing-rules block verbatim. Open decisions without a design file
   are listed for the agent to ask about. Model `opus`, passed explicitly;
   `isolation: "worktree"`.
4. While it runs, do not duplicate its investigation. Answer its
   `SendMessage` questions; brief it to reply to `main`.
5. On return, treat the report as a claim (`docs/briefs/report-format.md`
   headings). Check pasted `cargo test` lines, that test expectations
   come from the game, omissions and divergences recorded.
6. Review: copy `docs/briefs/review.md` for a `sonnet` (or `opus` for
   fidelity) agent given only the branch and oracle, not the author's
   conclusion. Re-run the deciding command yourself.
7. Bring each omission to the user as "port or skip for now?"; add
   approved leftovers to `docs/OPEN-ITEMS.md`, bugs to `docs/BUGS.md`.
8. Merge with skill `merge-branch`. Report to the user exactly what was
   observed ("the tests pass" means the port agrees with the data those
   tests use, nothing more); their play is the acceptance test.
