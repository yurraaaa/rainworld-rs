#!/usr/bin/env bash
# Per-crate check: fmt, check, clippy (warnings are errors), release tests,
# mirrors. Compact output by default: one `ok <stage> (<secs>s)` line per stage,
# details only for a failing stage. --verbose prints cargo's full output.
# Usage: scripts/check.sh <package> [test filter...] [--verbose]
#        e.g. scripts/check.sh rw-sim lizard
# Sourced by scripts/merge_check.sh for the helpers below.
set -euo pipefail

VERBOSE=0
CHECK_TMP=$(mktemp -d)
trap 'rm -rf "$CHECK_TMP"' EXIT

now() { date +%s.%N; }
elapsed() { awk -v a="$1" -v b="$(now)" 'BEGIN { printf "%.1f", b - a }'; }

# stage <name> <cmd...>: runs cmd; on failure prints its output (capped) and
# exits 1. STAGE_WARN_FAIL=1 also fails when the output has warnings.
stage() {
    local name=$1
    shift
    if [[ $VERBOSE == 1 ]]; then
        echo "== $name"
        "$@"
        return
    fi
    local t0 out="$CHECK_TMP/$name.out" rc=0
    t0=$(now)
    "$@" >"$out" 2>&1 || rc=$?
    if [[ $rc == 0 && ${STAGE_WARN_FAIL:-0} == 1 ]] && grep -qE '^warning|: warning' "$out"; then
        rc=1
    fi
    if [[ $rc != 0 ]]; then
        echo "FAIL $name ($(elapsed "$t0")s)"
        grep -vE '^\s*(Compiling|Checking|Finished|Fresh|Running|Blocking|Downloaded)\b' "$out" | head -n 200
        exit 1
    fi
    echo "ok $name ($(elapsed "$t0")s)"
}

# Parses a `cargo test` output file: sums the `test result:` lines, collects
# `skipping:` lines (installed-game tests; needs --nocapture) and, on failure,
# prints the output without the passing lines.
SUMMARY_PY='
import re, sys
text = open(sys.argv[1], errors="replace").read()
tot = dict(passed=0, failed=0, ignored=0, filtered=0)
for m in re.finditer(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out", text):
    for k, v in zip(tot, m.groups()):
        tot[k] += int(v)
skips = [l.strip() for l in text.splitlines() if l.startswith("skipping")]
print("SUMMARY %d passed, %d failed, %d ignored, %d filtered out" % tuple(tot.values())
      + ("; %d skipped (game missing)" % len(skips) if skips else ""))
for s in sorted(set(skips))[:3]:
    print("SKIP " + s)
noise = re.compile(r"^(test .* \.\.\. ok|test .* \.\.\. ignored.*|\s*(Compiling|Checking|Finished|Fresh|Running|Doc-tests)\b.*|running \d+ tests?|test result: ok.*|skipping.*|)$")
for l in text.splitlines()[-3000:]:
    if not noise.match(l):
        print("OUT " + l)
'

# test_stage <cargo test args...>: runs cargo test, prints a summary line.
test_stage() {
    if [[ $VERBOSE == 1 ]]; then
        echo "== test"
        cargo test --release "$@"
        return
    fi
    local t0 out="$CHECK_TMP/test.out" rc=0
    t0=$(now)
    cargo test --release "$@" -- --nocapture >"$out" 2>&1 || rc=$?
    local parsed summary
    parsed=$(python3 -I -c "$SUMMARY_PY" "$out")
    summary=$(grep '^SUMMARY' <<<"$parsed" | sed 's/^SUMMARY //' || true)
    if [[ $rc != 0 ]]; then
        echo "FAIL test ($(elapsed "$t0")s): ${summary:-no test results}"
        { grep '^OUT ' <<<"$parsed" | sed 's/^OUT //' || true; } | head -n 200
        exit 1
    fi
    echo "ok test ($(elapsed "$t0")s): $summary"
    grep '^SKIP ' <<<"$parsed" | sed 's/^SKIP /  /' || true
}

main() {
    cd "$(dirname "$0")/.."
    local args=() a
    for a in "$@"; do
        if [[ $a == --verbose ]]; then VERBOSE=1; else args+=("$a"); fi
    done
    local pkg="${args[0]:?usage: scripts/check.sh <package> [test filter...] [--verbose]}"
    local filters=("${args[@]:1}")
    local q=(-q)
    if [[ $VERBOSE == 1 ]]; then q=(); fi
    stage fmt cargo fmt -p "$pkg" -- --check --color never
    stage check cargo "${q[@]}" check -p "$pkg" --all-targets --message-format short
    stage clippy cargo "${q[@]}" clippy -p "$pkg" --all-targets --message-format short -- -D warnings
    test_stage -p "$pkg" "${filters[@]}"
    stage mirrors python3 scripts/check_mirrors.py
    git checkout -- docs/MIRRORS.md
}

if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
    main "$@"
fi
