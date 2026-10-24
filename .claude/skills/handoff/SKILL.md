---
name: handoff
description: Write the orchestrator handoff document for the next rainworld-rs session, quickly. Use when the user says /handoff, asks for a handoff, or says usage is running out and the next session should take over.
---

# Write a handoff

Fast: use what is already in the conversation; run only the few commands below. No agents.

1. Gather (one Bash call):
   ```
   cd /home/yura/Documents/experiments/rainworld-rs
   git log --oneline -1; git status --short; git worktree list; git branch --list 'worktree-*'
   git -C ../rainworld-re status --short; git -C ../rainworld-re log --oneline -1
   grep -n '^\*\*M' docs/MILESTONES.md; grep -n '^## ' docs/OPEN-ITEMS.md
   grep -ohE 'D[0-9]+' docs/divergences/*.md | sort -t D -k1.2 -n | tail -1
   ```
2. Overwrite `/home/yura/Documents/experiments/RAINWORLD-ORCHESTRATOR-HANDOFF.md` (not committed
   anywhere; it replaces the previous one). Sections, short bullets, no repetition of AGENTS.md:
   1. **State at handover:** commits of both repos, clean or not, worktrees and branches, uncommitted
      `rainworld-re` notes, the registered MCP server (leave it), highest divergence ID and the next free
      one, last workspace suite result.
   2. **In flight:** each running or unmerged agent: task, branch, worktree, model, its status (porting,
      in review, review verdict), the exact fix list it was sent, what is left before merge. Agent IDs die
      with the session: say how to resume (a fresh agent on the same worktree with the list).
   3. **Next:** what is approved and in which order; what needs the user's go-ahead.
   4. **Where the project stands:** point to `docs/STATUS.md`; milestones done/current, what this session merged (one line each). If this session changed the phase, update and commit STATUS.md.
   5. **Plans and decisions:** the current milestone's step plan and the user's decisions (point to the
      survey files that hold them).
   6. **Unanswered questions:** every question asked of the user without an answer, with the
      recommendation given; open OPEN-ITEMS worth a look.
   7. **Preferences learned this session:** new rules (and whether they are in AGENTS.md or memory),
      game-bug rulings, model choices, process lessons.
   8. **Useful pointers:** merge procedure deviations, files that conflict, measurements, facts that took
      effort to find.
3. Tell the user the path and, in two or three lines, what the next session should do first.
