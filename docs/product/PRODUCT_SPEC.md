# Leaf — Product Spec

> "Leaf" is a codename. Rename freely; crate names use the `leaf-` prefix.

## Vision

A fast, native, local-first workspace for pages and structured data.
Notion's flexibility, Obsidian's speed and ownership, no browser in the way.

## Principles

1. **Native speed.** Every interaction feels instant. No webview, no Electron.
2. **Local-first.** Works fully offline. Data lives on the device; cloud is optional and comes later.
3. **Pages all the way down.** There are no folders. Any page can contain pages.
4. **Collaboration-ready from day one.** The document model is a CRDT (Loro) even before sync ships.
5. **You own your data.** Open, documented format plus markdown export.
6. **Keyboard-first.** Everything is reachable without a mouse.
7. **Own identity.** Notion-like interaction patterns, not a pixel copy of Notion's UI.

## Target users

Solo builders, writers and engineers who want Notion's structure without lag or lock-in.
Small teams later, with cloud sync.

## Platforms

macOS, Windows, Linux (desktop). Mobile and web are later phases.

---

## Phase 0 — Pre-MVP: the page editor (current)

**Goal:** write a long, real document (a blog post) with zero friction and zero lost text.

### Workspace and pages
- Create or open a workspace (a local directory).
- Sidebar page tree: nested pages, expand/collapse, create, rename, move (drag), reorder, delete to trash, restore.
- Page has a title and an optional emoji icon.
- Quick switcher (`Cmd/Ctrl+P`): fuzzy search by page title.

### Search
- **Search bar** (`Cmd/Ctrl+K` with no text selected, or `Cmd/Ctrl+P`): one box that finds pages by title and
  by the text inside them, across the whole workspace (Notion-style).
- Results are grouped by page, title matches first, each with a short excerpt and the matched words highlighted.
- Choosing a result opens the page and scrolls to the matching block.
- Pages in the trash are not searched. Matching ignores case and accents; partial words match as you type.
- Works fully offline; the search index is rebuilt from the pages if it is lost.

### Block editor
- **Block types:** paragraph, heading 1–3, bulleted list, numbered list, to-do, quote, code (language label; syntax highlighting optional), divider, image (local file).
- **Inline marks:** bold, italic, strikethrough, inline code, link.
- **Slash menu** (`/`): insert or convert blocks, fuzzy filtered.
- **Markdown shortcuts** at the start of a block: `# `, `## `, `### `, `- `, `* `, `1. `, `[] `, `> `, ```` ``` ````, `---`.
- **Inline shortcuts:** `Cmd/Ctrl+B/I/E`, `Cmd/Ctrl+Shift+S` (strike), `Cmd/Ctrl+K` with text selected (link; without a selection it opens search).
- **Block handle** (⋮⋮) on hover: drag to reorder or nest; menu with Turn into, Duplicate, Delete.
- **Nesting:** `Tab` / `Shift+Tab` indent and outdent.
- **Selection:** within a block and across blocks (Shift+arrows, mouse drag). `Cmd/Ctrl+A` once selects the block's text, twice selects all blocks.
- **Clipboard:** copy/cut/paste rich blocks inside the app; paste plain text and markdown from outside.
- **Undo/redo** for every edit.
- **Autosave.** No save button. Nothing lost on crash or force-quit.

### Out of scope for Phase 0
Databases, page links/backlinks, sync, export, templates, comments, mobile.

---

## Phase 1 — MVP

- **Database (single view type: Table)**
  - Property types: text, number, select, multi-select, date, checkbox, URL.
  - Add, rename, reorder, hide columns. Sort and filter. Inline cell editing.
  - Each row is a page (open it to write content).
  - Database as a full page and inline inside a page.
- `[[page links]]` with autocomplete; backlinks panel.
- Markdown export (page and workspace) and import.
- Trash: restore and permanent delete.

## Phase 2+

Board, calendar and gallery views; relations and rollups; toggles, callouts, columns; templates; graph view;
cloud sync and team workspaces; web client; MCP server; mobile; P2P collaboration (maybe).

---

## Editor behaviour (canonical rules)

| Situation | Result |
|---|---|
| `Enter` mid-text | Split block at cursor; new block keeps the type for lists/to-dos, becomes paragraph for headings/quote. |
| `Enter` on empty list item / to-do | Outdent one level; at root, convert to paragraph. |
| `Enter` in code block | Newline inside the block. `Cmd/Ctrl+Enter` or `Enter` on two trailing empty lines exits. |
| `Backspace` at start of non-paragraph block | Convert to paragraph (keep text). |
| `Backspace` at start of paragraph | Merge text into previous text block; children move to previous block. If previous is a divider or image, select it instead. |
| `Delete` at end of block | Merge next block into this one. |
| `Tab` / `Shift+Tab` | Indent under previous sibling / outdent to parent's level. |
| Arrow up/down at first/last visual line | Move to previous/next block, keeping horizontal position. |
| Paste multi-line text | Each line becomes a paragraph; markdown syntax is converted. |
| Deleting a block with children | Children are deleted too (one undo step restores all). |

Numbered list numbers are computed when rendering, never stored.

## Non-functional requirements

- **Performance budgets**
  - Keystroke to paint: < 16 ms on a 10,000-block page.
  - Open a 5,000-block page: < 100 ms.
  - Cold start to editable: < 500 ms.
- **Reliability:** every edit is durable within 1 s; crash recovery loses nothing older than that.
- **Privacy:** no telemetry without explicit opt-in. No network calls in Phase 0.
- **Accessibility:** full keyboard navigation, respects system font scaling and light/dark mode.

## Non-goals (for now)

- Pixel-copying Notion's UI.
- Markdown as the storage format (it is import/export only).
- Plugins.
- Live web embeds (show link preview cards instead).

## Phase 0 acceptance checklist

- [ ] Write and edit a 3,000-word document with mixed blocks without a single editing bug.
- [ ] Every rule in "Editor behaviour" has an automated test.
- [ ] Undo/redo restores exact state for every command, including block moves.
- [ ] Kill the process mid-typing; reopening shows all text older than 1 s.
- [ ] Page tree with 500 pages, 6 levels deep, stays smooth.
- [ ] `Cmd/Ctrl+K` finds a word inside any page of a 5,000-page workspace and jumps to it; results appear in < 50 ms.
- [ ] Performance budgets met on a mid-range laptop.
- [ ] CI green on macOS, Windows, Linux with ≥ 90% coverage on core crates.
