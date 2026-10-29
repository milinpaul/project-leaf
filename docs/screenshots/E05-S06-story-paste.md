# Paste test: backlog story E05-S01

Generated from `crates/leaf-editor/src/clipboard/markdown/story-E05-S01.md` by the E05-S06 tests.

## 1. The story file, as pasted

````markdown
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
  model test for the separate step. ADR 0025.

## Log

- 2026-10-28: created
- 2026-10-29: claimed by claude
- 2026-10-29: started on branch story/E05-S01
- 2026-10-29: PR https://github.com/milinpaul/project-leaf/pull/43
- 2026-10-29: PR opened
- 2026-10-29: merged
````

## 2. The blocks Leaf makes from it

One line per block, indented by nesting; «text»[code] is inline code.

```
divider: 
paragraph: id: E05-S01
paragraph: title: Markdown shortcuts
paragraph: epic: E05
paragraph: status: done
paragraph: priority: P0
paragraph: size: S
paragraph: depends_on: [E04-S05]
paragraph: labels: [leaf-editor]
paragraph: assignee: claude
paragraph: updated: 2026-10-29
paragraph: pr: [https://github.com/milinpaul/project-leaf/pull/43]
divider: 
h2: Context
paragraph: Typing «# »[code], «- »[code], «1. »[code], «[] »[code], «> »[code], ``` and «---»[code] converts the block.
h2: Acceptance criteria
todo[x]: Each shortcut tested
todo[x]: Undo right after conversion restores the typed characters
todo[x]: Only triggers at block start
todo[x]: Tests added; «just check»[code] passes; docs updated if structure changed
h2: Plan
bulleted_list: New «leaf-editor/src/shortcuts.rs»[code]: after «insert_text»[code], if the text before the caret is exactly a shortcut («# »[code]..«### »[code], «- »[code]/«* »[code], «N. »[code], «[] »[code], «> »[code], «```»[code], «---»[code] on an empty block), delete it and set the kind; a divider gets a paragraph below for the caret. Not in code or captions; not into the same kind.
bulleted_list: «leaf-model»[code]: «PageDoc::as_separate_step»[code], an undo step that never merges with neighbouring typing, so undo right after the conversion gives back the typed characters.
bulleted_list: Tests: «shortcuts/tests.rs»[code] (each shortcut, undo, only at block start, kept text, code/captions, IME) and a model test for the separate step. ADR 0025.
h2: Log
bulleted_list: 2026-10-28: created
bulleted_list: 2026-10-29: claimed by claude
bulleted_list: 2026-10-29: started on branch story/E05-S01
bulleted_list: 2026-10-29: PR https://github.com/milinpaul/project-leaf/pull/43
bulleted_list: 2026-10-29: PR opened
bulleted_list: 2026-10-29: merged
```

## 3. Copied back out of Leaf as Markdown

Same as the original except: wrapped lines are joined into one, and the lone ```` ``` ```` is written escaped.

````markdown
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

Typing `# `, `- `, `1. `, `[] `, `> `, \`\`\` and `---` converts the block.

## Acceptance criteria

- [x] Each shortcut tested
- [x] Undo right after conversion restores the typed characters
- [x] Only triggers at block start
- [x] Tests added; `just check` passes; docs updated if structure changed

## Plan

- New `leaf-editor/src/shortcuts.rs`: after `insert_text`, if the text before the caret is exactly a shortcut (`# `..`### `, `- `/`* `, `N. `, `[] `, `> `, ```` ``` ````, `---` on an empty block), delete it and set the kind; a divider gets a paragraph below for the caret. Not in code or captions; not into the same kind.
- `leaf-model`: `PageDoc::as_separate_step`, an undo step that never merges with neighbouring typing, so undo right after the conversion gives back the typed characters.
- Tests: `shortcuts/tests.rs` (each shortcut, undo, only at block start, kept text, code/captions, IME) and a model test for the separate step. ADR 0025.

## Log

- 2026-10-28: created
- 2026-10-29: claimed by claude
- 2026-10-29: started on branch story/E05-S01
- 2026-10-29: PR https://github.com/milinpaul/project-leaf/pull/43
- 2026-10-29: PR opened
- 2026-10-29: merged
````
