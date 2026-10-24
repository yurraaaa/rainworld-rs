#!/usr/bin/env bash
# Merge-branch checks (skill merge-branch, steps 1-5) in an agent's worktree,
# with compact output (see scripts/check.sh). The orchestrator's tool.
# Usage: scripts/merge_check.sh <worktree> [-p <pkg>...] [--verbose]
#   -p restricts the cargo stages to the given packages (default: workspace).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=check.sh
source "$here/check.sh"

wt=""
pkgs=()
while [[ $# -gt 0 ]]; do
    case $1 in
        -p) pkgs+=("$2"); shift 2 ;;
        --verbose) VERBOSE=1; shift ;;
        *) wt=$1; shift ;;
    esac
done
[[ -n $wt ]] || { echo "usage: scripts/merge_check.sh <worktree> [-p <pkg>...] [--verbose]"; exit 2; }
cd "$wt"

scope=(--workspace)
fmt_args=(--all)
if [[ ${#pkgs[@]} -gt 0 ]]; then
    scope=()
    fmt_args=()
    for p in "${pkgs[@]}"; do
        scope+=(-p "$p")
        fmt_args+=(-p "$p")
    done
fi

# 1. merge main, mirrors
t0=$(now)
if ! git merge --no-edit main >"$CHECK_TMP/merge.out" 2>&1; then
    echo "FAIL merge ($(elapsed "$t0")s): conflicts"
    git diff --name-only --diff-filter=U
    tail -n 5 "$CHECK_TMP/merge.out"
    exit 1
fi
echo "ok merge ($(elapsed "$t0")s): $(grep -m1 -E "Already up to date|Fast-forward|Merge made" "$CHECK_TMP/merge.out" || echo merged)"
stage mirrors python3 scripts/check_mirrors.py
git checkout -- docs/MIRRORS.md docs/PORT-STATUS.md 2>/dev/null || true

# 2-4
stage fmt cargo fmt "${fmt_args[@]}" -- --check --color never
q=(-q)
if [[ $VERBOSE == 1 ]]; then q=(); fi
STAGE_WARN_FAIL=1 stage clippy cargo "${q[@]}" clippy "${scope[@]}" --all-targets --message-format short
test_stage "${scope[@]}"

# 5. diff greps (hits go back to the agent)
hits=0
t0=$(now)
closures=$(git diff main...HEAD | grep -nE '^\+.*let (mut )?[a-z_]+ = \|' || true)
literals=$(git diff main...HEAD | grep -nE '^\+\s*"[^"]+"\s*(\|.*)?=>' || true)
if [[ -n $closures || -n $literals ]]; then
    echo "FAIL diff-greps ($(elapsed "$t0")s): violations go back to the agent"
    [[ -n $closures ]] && { echo "bound closures:"; echo "$closures" | head -n 50; }
    [[ -n $literals ]] && { echo "string-literal match arms:"; echo "$literals" | head -n 50; }
    exit 1
fi
echo "ok diff-greps ($(elapsed "$t0")s): no bound closures, no string-literal match arms"
