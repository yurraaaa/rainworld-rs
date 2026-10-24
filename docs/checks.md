# Fast checks

Default inner loop, per crate, from the worktree root:

```
cargo check -p <crate>
cargo test --release -p <crate> <filter>      # filter = test or module name
scripts/check.sh <crate> [filter...]         # fmt, check, clippy -D warnings, release tests, mirrors
```

`check.sh` is compact: one `ok <stage> (<secs>s)` line per stage and a test
summary summed over the `test result:` lines; a failing stage prints only its
errors (fmt diff, compiler errors with spans, failing tests' output) and
exits nonzero. Installed-game tests that skip are counted (`N skipped (game
missing)`, from their `skipping:` lines, which `check.sh` sees by running the
tests with `--nocapture`). `--verbose` gives cargo's full output.

Packages: `rw-random`, `rw-assets`, `rw-save`, `rw-sim`, `rw-graphics`,
`rw-audio`, `rw-render`, `rw-menu`, `rw-hud`, `rainworld-rs` (`crates/app`).
`cargo fmt -p <crate> -- --check` covers one crate. Tests run `--release`
(the sim is far too slow in debug). `--all-targets` in `check.sh` also
compiles the tests.

## Tests that need the installed game

Integration tests named `installed_*` (in `crates/{assets,audio,hud,menu,render,save,sim}/tests/`)
read the Steam install (default `/home/yura/.local/share/Steam/steamapps/common/Rain World`,
override with `RW_DIR`). Without it they print `skipping: ...` and pass, so a
green run on a machine without the game proves little: look for `skipping`
in the output (`-- --nocapture`). Other tests (unit tests under `src/`, `rw-random`'s, `music_player`,
`procedural_music`, `game_music`, `unity_assets`, `visual_assets`) are not
named `installed_*`; not checked individually whether any read the game.

## When to run what

- While editing: `cargo check -p <crate>`; `cargo test --release -p <crate> <filter>`.
- Before reporting: `scripts/check.sh <crate>` for every crate you changed
  (and the crates that depend on it only if you changed a public item).
- Full workspace (`cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets`, `cargo test --release --workspace`): the orchestrator's
  merge step only (skill `merge-branch`), as `scripts/merge_check.sh <worktree>`
  (same compact output; `-p <pkg>` limits the cargo stages). Agents never run it.

## Parallel worktrees and cargo's lock

State of the repo: no `.cargo/config.toml` in the repo; `~/.cargo/config.toml`
sets only `[build] jobs = 4` (added when three builds pinned the CPU).
`CARGO_TARGET_DIR` is unset, so the main checkout's `target/` is 22 GB
(`debug` 21 GB, `release` 1.2 GB) and each worktree (`.claude/worktrees/<id>/`)
gets its own `target/` inside it: worktrees do not share a lock with the main
checkout or each other, but each starts cold (a fresh worktree `target/`
after a `rw-sim` check plus `rw-random` release tests: 131 MB; a full
workspace debug build is tens of GB, so do not build the workspace per worktree).
Contention only happens between agents in the same worktree or the main
checkout, or if `CARGO_TARGET_DIR` points several at one directory.

Proposal (not set): a shared `CARGO_TARGET_DIR=/home/yura/.cache/rw-target`
would let worktrees reuse compiled dependencies (wgpu, winit, kira: the long
part of a cold build) but serialises every cargo command on one lock, which
is the contention to avoid, and cross-worktree path changes recompile the
workspace crates anyway. A per-worktree dir (`CARGO_TARGET_DIR=$PWD/target`,
i.e. the status quo) costs disk (hundreds of MB for check, tens of GB for
full builds) and a cold first build, and removing the worktree frees it.
Recommendation: keep the status quo, give
parallel agents disjoint crates, and delete worktrees after merge.

## Measured (16 cores, `jobs = 4`, fresh worktree `target/`, 2026-10-19)

| Command | Time |
|---|---|
| `cargo check -p rw-random`, first (cold) | 0.14 s |
| `cargo check -p rw-random`, warm | 0.05 s |
| `cargo check -p rw-sim`, first (builds deps) | 4.5 s |
| `cargo check -p rw-sim`, after touching one file | 0.5 s |
| `cargo check -p rw-sim`, nothing changed | 0.05 s |
| `cargo check -p rw-sim --tests`, first | 4.8 s |
| `scripts/check.sh rw-random` (fmt, check, clippy, 12 tests) | 2.5 s |
