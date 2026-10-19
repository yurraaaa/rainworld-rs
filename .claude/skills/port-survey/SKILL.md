---
name: port-survey
description: Run a survey of the original Rain World's behaviour before porting, in rainworld-re. Use when the game's behaviour or the scope of the next port step is unclear, before writing a porting brief, or when asked to survey or investigate a creature, system or data format.
---

# Survey before porting

Only when behaviour or scope is genuinely unclear. Otherwise go straight to
`port-step`.

1. Check `../rainworld-re/notes/`, `../rainworld-re/surveys/` and
   `docs/OPEN-ITEMS.md`, `docs/BUGS.md` for what is already known.
2. Fill `docs/briefs/research.md`: question, the decision it feeds, files
   to look in, what is ruled out and by what evidence, output path
   `../rainworld-re/surveys/<topic>.md`, DLC branches listed not dropped.
   Model `haiku` for plain lookups, `sonnet` for judgement. Read-only: no
   worktree, no installs, no windows. Split a topic one agent cannot finish
   in a context into several surveys; independent ones run in parallel.
3. When it returns, re-check the key facts against the decompile
   (`/home/yura/.local/share/ilspy/split/<Type>.cs`) yourself or with a
   cheap second agent that is not told the first one's answer. Facts marked
   `inferred` stay inferred until checked.
4. Bring the user: scope, the open decisions, each omission as "port or
   skip for now?", and a recommendation. Append the user's decisions to the
   end of the survey file ("Decisions" section) and commit it in `../rainworld-re`.
5. Start nothing new until the user agrees explicitly.

The next step is `port-step`, which reads the survey.
