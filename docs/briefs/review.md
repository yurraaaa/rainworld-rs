# Review brief (verification agent)

A subagent's success is a claim, not a result. Give the reviewer where the
work is, not the author's conclusion. Model: `sonnet`; `opus` when judging
fidelity to the game's code. Read-only on the branch.

## Task

Verify the work on branch `<branch>` in worktree `<path>` against
`<oracle: decompiled member / data file>`. Claims to test (not truths):
<what the author reported>.

Do, in order:

1. Read `main..HEAD` log and diff stat, then the diff.
2. Re-run the deciding command yourself (`<command>`) and paste its output;
   do not rely on the author's. Per-crate commands only (`docs/checks.md`);
   the orchestrator runs the workspace suite at merge.
3. For each new test, check its expected values come from the game's code
   or data (cite member or file), not from the port's own output.
4. Compare the port to the original function by function: branch
   conditions, tick order, every `UnityEngine.Random` call (count, order,
   condition). Report each difference with both sides excerpted, as bug or
   divergence candidate; the orchestrator decides.
5. Grep the diff for: bound closures (`let x = |`), string-literal match
   arms, free private helper fns for type behaviour, comments restating code.
6. Run `python3 scripts/check_mirrors.py` (exit 0; new files have a `Mirrors:` header; generated docs not committed in the branch). Grep docs for every name the diff touched; list stale hits (MAP.md,
   crate README, `docs/divergences/`, BUGS, OPEN-ITEMS).
7. Omissions: original behaviour the diff does not reach that the author
   did not list as "port or skip for now?".
8. Behaviour, if relevant: headless MCP tools only, at <room, seed, tiles>.

Standing rules: AGENTS.md (loaded with your instructions) applies (no
installs, no windows, never touch the game/Steam, pidof not pgrep -f). Do not
edit the branch (put findings in the report); do not merge. Use
`../rainworld-re/tools/cs.py` for decompile lookups. No progress pings.

Report with a **Verdict** heading: `accept`, `accept after fixes` (list) or
`reject` (why), each finding marked observed or inferred; then
[`report-format.md`](report-format.md) (read-only variant).
