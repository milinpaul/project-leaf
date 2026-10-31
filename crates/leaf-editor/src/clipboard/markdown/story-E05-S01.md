---
id: E05-S01
title: Markdown shortcuts
epic: E05
status: done
priority: P0
size: S
depends_on: [E04-S05]
labels: [leaf-editor]
assignee: claude
updated: 2026-10-29
pr: [https://github.com/milinpaul/project-leaf/pull/43]
---

## Context

Typing `# `, `- `, `1. `, `[] `, `> `, ``` and `---` converts the block.

## Acceptance criteria

- [x] Each shortcut tested
- [x] Undo right after conversion restores the typed characters
- [x] Only triggers at block start
- [x] Tests added; `just check` passes; docs updated if structure changed

## Plan

- New `leaf-editor/src/shortcuts.rs`: after `insert_text`, if the text before the caret is exactly a shortcut
  (`# `..`### `, `- `/`* `, `N. `, `[] `, `> `, ```` ``` ````, `---` on an empty block), delete it and set the kind;
  a divider gets a paragraph below for the caret. Not in code or captions; not into the same kind.
- `leaf-model`: `PageDoc::as_separate_step`, an undo step that never merges with neighbouring typing, so undo
  right after the conversion gives back the typed characters.
- Tests: `shortcuts/tests.rs` (each shortcut, undo, only at block start, kept text, code/captions, IME) and a
  model test for the separate step. ADR-025.

## Log

- 2026-10-28: created
- 2026-10-29: claimed by claude
- 2026-10-29: started on branch story/E05-S01
- 2026-10-29: PR https://github.com/milinpaul/project-leaf/pull/43
- 2026-10-29: PR opened
- 2026-10-29: merged
