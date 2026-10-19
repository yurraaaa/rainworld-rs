#!/usr/bin/env bash
# Per-crate check: fmt, check, clippy (warnings are errors), release tests.
# Usage: scripts/check.sh <package> [test filter]   e.g. scripts/check.sh rw-sim lizard
set -euo pipefail
cd "$(dirname "$0")/.."
pkg="${1:?usage: scripts/check.sh <package> [test filter]}"
shift
cargo fmt -p "$pkg" -- --check
cargo check -p "$pkg" --all-targets
cargo clippy -p "$pkg" --all-targets -- -D warnings
cargo test --release -p "$pkg" "$@"
python3 scripts/check_mirrors.py >/dev/null
git checkout -- docs/MIRRORS.md
