# Backlog

The backlog is a set of markdown files. They are the **source of truth**.
`backlog.json` and `board.html` are generated from them for the control panel. Both are local files, not
committed (so parallel PRs never conflict on them). Never edit them by hand.

```
backlog/
  epics/E01.md          one file per epic
  stories/E01-S03.md    one file per story
  backlog.json          generated locally: `just backlog` (gitignored)
```

One file per story means parallel agents never hit merge conflicts on a shared file.

## Story file

```markdown
---
id: E02-S05
title: Split and merge blocks
epic: E02
status: todo
priority: P0            # P0 must-have, P1 should-have, P2 nice-to-have
size: M                 # S ≤ half day, M ≤ 2 days, L > 2 days (split it if possible)
depends_on: [E02-S04]
labels: [leaf-model, commands]
assignee:               # agent or human name while working
updated: 2026-10-28
pr: [https://github.com/owner/repo/pull/12]   # optional; added by `backlog.py pr`
---

## Context
## Acceptance criteria   (checkboxes, ticked as they are met)
## Plan                  (written by the agent in `planning`)
## Log                   (dated one-liners)
```

Keep frontmatter values on one line. Lists use `[a, b]`.

## Statuses

| Status | Meaning | Who moves it |
|---|---|---|
| `backlog` | Not yet ready to work on | Human |
| `todo` | Ready once dependencies are done | Human |
| `planning` | Agent is writing the plan | Agent |
| `in_progress` | Implementation underway | Agent |
| `review` | PR open, waiting for human review | Agent |
| `done` | Merged | Human |
| `blocked` | Stuck; reason in Log | Agent or human |

A story is **ready** when its status is `todo` and every `depends_on` story is `done` (computed in the JSON).

## Automation

Claude Code handles all of this itself:
- `.claude/skills/backlog/SKILL.md` is the procedure Claude follows.
- `.claude/settings.json` hooks regenerate `backlog.json` and `board.html` at session start and after every backlog edit.
- Open `backlog/board.html` in a browser to see the control panel (ask Claude to open it).
- **Phases:** each epic's `phase:` (0 to 3, named in `scripts/backlog.py` after `PRODUCT_SPEC.md`) puts it and its
  stories in a phase. The board shows every phase's progress and marks the current one (the first with unfinished
  stories); picking a phase narrows the epics and stories to it. On the live board (`backlog.py serve`), each
  epic's "Move to" menu moves it to another phase by rewriting its `phase:` line; `backlog.py phase <epic> <n>` does
  the same from the command line. Only the board page itself can make that change, not other websites.

## Agent protocol (summary)

1. **Pick:** `python3 scripts/backlog.py next`. Take the story it prints. Never pick a story that is not ready or already has an assignee.
2. **Claim:** set `status: planning`, `assignee: <your name>`, `updated: <today>`; add a Log line. Commit this change alone.
3. **Plan:** fill in `## Plan` (approach, files to touch, tests to write, risks). If the story is unclear or too big, set `status: blocked`, explain in Log, and stop.
4. **Build:** set `status: in_progress`. Work on branch `story/<id>`. Tick acceptance boxes as they are met.
5. **Review:** when every box is ticked and `just check` passes, open a PR titled `<id>: <title>`, link it with `python3 scripts/backlog.py pr <id> <url>` (fills `pr:`, shown on the board card), and set `status: review`.
6. **Done:** only a human sets `status: done` after merging.
7. Hooks regenerate `backlog.json` after edits; CI fails if the backlog is invalid.
8. Found new work? Create a new story file in the same format with `status: backlog` and mention it in the Log. Do not expand the current story's scope.
