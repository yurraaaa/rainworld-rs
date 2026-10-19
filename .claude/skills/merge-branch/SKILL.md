---
name: merge-branch
description: Merge a finished agent worktree branch into main for rainworld-rs. Use when an agent reports a porting branch done and reviewed, or the user says to merge a worktree branch. Not for starting new work.
---

# Merge an agent's branch

Run the checks in the agent's worktree (`<wt>` = `.claude/worktrees/<id>`,
`<branch>` = its branch), then merge on main. Do not skip a step; report the
real output of each.

1. In `<wt>`: `git merge main`. Resolve conflicts; re-run what the conflict touched.
   Then `python3 scripts/check_mirrors.py` must exit 0 (new files carry a
   `//! Mirrors:` header); a new crate or module must be in `docs/MAP.md`
   and its crate `README.md`. Send violations back to the agent.
2. In `<wt>`: `cargo fmt --all -- --check` (must print nothing).
3. In `<wt>`: `cargo clippy --workspace --all-targets` (0 warnings).
4. In `<wt>`: `cargo test --release --workspace`; sum the `test result:` lines
   (passed, failed, ignored). Installed-game tests skip silently without the
   game; see `docs/checks.md`.
5. In `<wt>`: grep the diff against main for bound closures (`let x = |`)
   and string-literal match arms: pipe `git diff main...HEAD` into
   `grep -nE '^\+.*let (mut )?[a-z_]+ = \|'` and into
   `grep -nE '^\+\s*"[^"]+"\s*(\|.*)?=>'`. Violations go back to the agent.
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
