---
name: merge-branch
description: Merge a finished agent worktree branch into main for rainworld-rs. Use when an agent reports a porting branch done and reviewed, or the user says to merge a worktree branch. Not for starting new work.
---

# Merge an agent's branch

Run the checks in the agent's worktree (`<wt>` = `.claude/worktrees/<id>`,
`<branch>` = its branch), then merge on main. Do not skip a step; report the
real output of each.

1-5. Run `scripts/merge_check.sh <wt>` (compact output, `--verbose` for
   cargo's full output). It runs, in `<wt>`: `git merge main` (stops and lists
   conflicts: resolve them and re-run what they touched), `check_mirrors.py`
   (must exit 0: new files carry a `//! Mirrors:` header; it also fails when a
   file is missing from its crate `README.md`), `cargo fmt --all -- --check`,
   `cargo clippy --workspace --all-targets` (0 warnings), `cargo test --release
   --workspace` (summed `test result:` lines; installed-game tests skip without
   the game, see `docs/checks.md`) and the two greps of the diff against main
   (bound closures `let x = |`, string-literal match arms). Also read the
   README lines for the names the diff touched and send stale ones back; a new
   crate must be in `docs/MAP.md`. Violations go back to the agent. Report the
   script's real output.
6. Look at the screenshots the agent produced (visual work only).
7. Confirm the review (`docs/briefs/review.md`) verdict and that every
   "port or skip for now?" omission was brought to the user.
8. On main (main checkout): `git merge --no-ff <branch>`. Commits are
   GPG-signed; if pinentry fails, ask the user, never bypass signing.
9. Remove the worktree and branch: `git worktree remove <wt>`,
   `git branch -d <branch>`. On main regenerate and commit the generated
   docs: `python3 scripts/check_mirrors.py && python3 scripts/port_status.py`,
   and in `../rainworld-re` `python3 tools/index_decompile.py --rust-repo
   ../rainworld-rs` (commit its `index/` with the notes). Then `git push`.
10. In `../rainworld-re`: commit only the notes files the agent named in
    its report ("Notes added" heading).

New work starts only on the user's explicit go-ahead; an answer to another
question is not one.
