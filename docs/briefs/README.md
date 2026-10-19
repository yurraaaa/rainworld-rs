# Agent brief templates

The orchestrator copies a template, fills every `<...>` field, deletes what
does not apply, and passes the result as the agent prompt. Pass `model`
explicitly (AGENTS.md "Pick the model by the work"; never `fable`).

| Template | Agent | Model | Worktree |
|---|---|---|---|
| [`porting.md`](porting.md) | ports one step of the game | `opus` | yes, one per agent |
| [`research.md`](research.md) | read-only survey or lookup, writes `../rainworld-re/surveys/<topic>.md` | `haiku` (lookups), `sonnet` (judgement) | no |
| [`review.md`](review.md) | verifies another agent's work | `sonnet`; `opus` for fidelity to the game | reads the branch's worktree |
| [`report-format.md`](report-format.md) | the report every agent ends with | | |

Rules for filling a template:

- Short. Name files to read instead of pasting their text.
- The standing-rules block in `porting.md` is pasted verbatim, unedited.
  The read-only templates carry the subset that applies to them.
- A field you cannot fill is a decision for the user: ask before spawning.
- Say what else is running (crates and worktrees in use) in every brief.
- Contradicting the orchestrator is welcome: the game's code and data win.

Related: [`../checks.md`](../checks.md) (which commands to run, when);
project skills `port-step`, `port-survey`, `merge-branch` in `.claude/skills/`.
