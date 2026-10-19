# Research brief (read-only)

Model: `haiku` for lookups with a small answer, `sonnet` where it needs
judgement. No worktree; the agent edits only its output file.

## Task

Find out: <question>. Why it matters: <the decision it feeds>.

- Look in: `../rainworld-re/notes/` and `../rainworld-re/index/` first, then
  the decompile `/home/yura/.local/share/ilspy/split/<Type>.cs` (one file
  per type; nested types in the parent's file), then installed data under
  <path>. Already known irrelevant: <list>.
- Ruled out, and by what evidence: <claims>.
- Output: `../rainworld-re/surveys/<topic>.md` (a lookup with a short answer:
  the report only, no file). Decompiled excerpts are welcome in the file.
  Mark every claim `observed` (seen in code or data; cite file and member)
  or `inferred`. End the file with an empty "Decisions" section for the user.
- Cover: <control flow, fields, random draws in order, tick conditions,
  base game versus DLC branches (list DLC branches, do not drop them)>.
- Scope: every behaviour you think the port could leave out, as
  "port or skip for now?" with its size. Do not decide.
- Data surveys: what you read and what you rejected, with the reason; the
  totals must add up.
- Contradicting the brief is welcome; the game's code and data win.
- Write nothing under `crates/`.

## Standing rules

```
- Read-only on the port. No installs or downloads (cargo install, pip,
  npm -g, dotnet tool, curl/wget); do not run decompile_types.sh unless
  ilspycmd already exists. Ask `main` if you need a tool.
- Shared machine: never start, stop or kill the game or Steam; open no
  windows; no cargo builds beyond what the brief names.
- Never pkill -f / pgrep -f.
```

End with the report in [`report-format.md`](report-format.md) (read-only
variant: headings 1, 3, 7, 8, plus Findings).
