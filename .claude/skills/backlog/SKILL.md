---
name: backlog
description: Manage the project backlog in backlog/ (epics and stories as markdown). Use whenever starting work, choosing what to do next, claiming or finishing a story, opening a PR for a story, being blocked, finding new work, when the user says a PR was merged, or when the user asks about progress, status or the board.
---

# Backlog housekeeping

The owner never runs backlog commands. You do all housekeeping yourself, every time, without being asked.
Markdown files in `backlog/` are the source of truth. `backlog/backlog.json` and `backlog/board.html` are generated locally and gitignored; never edit or commit them.

Hooks already do two things automatically:
- At session start, the backlog is regenerated and a status summary is added to your context.
- After you edit any `backlog/**/*.md` file, the backlog is validated and regenerated. If the hook reports an error, fix the file immediately.

## Commands (run them yourself)

```
python3 scripts/backlog.py next                       # next ready story
python3 scripts/backlog.py claim <id> <your-name>     # todo → planning, sets assignee
python3 scripts/backlog.py move <id> <status> "note"  # change status + dated Log line
python3 scripts/backlog.py pr <id> <url>              # link a PR (fills `pr:`, shown on the card)
python3 scripts/backlog.py summary                    # counts per status
python3 scripts/backlog.py phase <epic> <n>           # move an epic (and its stories) to phase n
python3 scripts/backlog.py serve --open               # live board (run in background)
```

Use `claude` as your name unless the owner gives you another one. Prefer these commands over hand-editing frontmatter; they also update `updated:` and the Log.

## Workflow for a story

1. **Before any coding**, check the session summary. If a story is already `planning` or `in_progress` with your name, continue it. Otherwise run `next` and `claim` it. Never work on code that is not tied to a story.
2. **Plan.** Fill in `## Plan` in the story file: approach, files to touch, tests, risks. Keep it short. If the story is unclear, too large or conflicts with the specs: `move <id> blocked "reason"`, tell the owner, and stop.
3. **Build.** `move <id> in_progress "started on branch story/<id>"`. Work on branch `story/<id>`. Tick acceptance checkboxes (`- [x]`) in the story file as each one is genuinely met.
4. **Finish.** When every box is ticked and `just check` passes: commit, open a PR titled `<id>: <title>` (use `gh pr create` if available), then `pr <id> <url>` and `move <id> review "PR opened"`. Every PR for a story gets linked this way, including follow-up fix PRs.
5. **Merged.** When the owner says the PR is merged, or `gh pr view` shows it merged: `move <id> done "merged"`. Never mark `done` otherwise.
6. **Next.** Tell the owner in one or two lines what finished and what is next. Start the next story only if the owner asked you to keep going.

## Other situations

- **New work discovered:** create `backlog/stories/<epic>-S<nn>.md` using the same format as existing stories (next free number in that epic), `status: backlog`. Mention it in the current story's Log. Do not widen the current story.
- **Splitting a story:** create the new stories, then `move <old-id> done "split into …"` only after the owner agrees.
- **Owner asks "where are we?":** answer from `summary` and the session summary, in a few lines.
- **Owner wants to see the board:** start the live board as a **background** process with `python3 scripts/backlog.py serve --open` and share `http://127.0.0.1:4317`. It auto-refreshes whenever story files change. If port 4317 is busy, it is probably already running; just share the URL. Without a server, `backlog/board.html` also opens directly in a browser.
- **Status changes in a commit:** commit backlog file changes together with the code they describe, or as their own small commit when claiming.

## Rules

- One story in `planning`/`in_progress` per agent at a time.
- Never claim a story whose dependencies are not done; `claim` refuses anyway.
- Never delete story files.
- Keep Log lines short and dated (the commands do this for you).
