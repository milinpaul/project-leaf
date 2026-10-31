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

macOS, Windows, Linux (desktop). A read-only web viewer comes in Phase 2 and the web editor in Phase 3; mobile later.

---

## Phase 0 — Pre-MVP: the page editor (current)

**Goal:** write a long, real document (a blog post) with zero friction and zero lost text.

### Workspace and pages
- Create or open a workspace (a local directory).
- Sidebar page tree: nested pages, expand/collapse, create, rename, move (drag), reorder, delete to trash, restore.
- Page has a title, an optional emoji icon and an optional cover (built-in covers: calm colours, gradients and six drawn scenes).
- Quick switcher (`Cmd/Ctrl+P`): fuzzy search by page title.

### Search
- **Search bar** (`Cmd/Ctrl+K` with no text selected, or `Cmd/Ctrl+P`): one box that finds pages by title and
  by the text inside them, across the whole workspace (Notion-style). A "Search" field at the top of the sidebar,
  showing the shortcut, opens it too.
- Results are grouped by page, title matches first, each with a short excerpt and the matched words highlighted.
- Choosing a result opens the page and scrolls to the matching block.
- Pages in the trash are not searched. Matching ignores case and accents; partial words match as you type.
- Works fully offline; the search index is rebuilt from the pages if it is lost.

### Block editor
- **Block types:** paragraph, heading 1–3, bulleted list, numbered list, to-do, quote, code (language label; syntax highlighting optional), divider, image (local file), simple table (a grid of text cells, not a database; E16), action item (a task with a due date, optional time, priority and a round check; overdue dates in red; E19), callout (text in a tinted box with an icon, in five tones: info, success, warning, error, note; E21).
- **Inline marks:** bold, italic, strikethrough, inline code, link.
- **Slash menu** (`/`): insert or convert blocks, fuzzy filtered.
- **Markdown shortcuts** at the start of a block: `# `, `## `, `### `, `- `, `* `, `1. `, `[] `, `> `, ```` ``` ````, `---`; GitHub alerts `> [!NOTE] `, `> [!TIP] `, `> [!IMPORTANT] `, `> [!WARNING] `, `> [!CAUTION] ` make an info, success, note, warning or error callout (in a quote, `[!WARNING] ` alone does it). The slash menu offers `Callout` (info) and `Callout: Success`, `Warning`, `Error`, `Note` (also found as `/success`, `/warning`, `/error`, `/note`).
- **Inline shortcuts:** `Cmd/Ctrl+B/I/E`, `Cmd/Ctrl+Shift+S` (strike), `Cmd/Ctrl+K` with text selected (link; without a selection it opens search). `Cmd/Ctrl+click` on a link opens it in the browser.
- **Block handle** (⋮⋮) on hover: drag to reorder or nest; menu with Turn into (its list opens on hover), Duplicate, Move up, Move down, Delete (each with an icon); `Cmd/Ctrl+/` opens the menu from the keyboard.
- **Nesting:** `Tab` / `Shift+Tab` indent and outdent.
- **Selection:** within a block and across blocks (Shift+arrows, mouse drag). `Cmd/Ctrl+A` once selects the block's text, twice selects all blocks.
- **Clipboard:** copy/cut/paste rich blocks inside the app; paste plain text and markdown from outside.
- **Undo/redo** for every edit.
- **Autosave.** No save button. Nothing lost on crash or force-quit.

### Out of scope for Phase 0
Databases, page links/backlinks, sync, export, templates, comments, mobile.

---

## Phase 1 — MVP

- **Database (table first, then board and saved views)**
  - Property types: text, number, select, multi-select, date, checkbox, URL.
  - Add, rename, reorder, hide columns. Sort and filter. Inline cell editing.
  - Each row is a page (open it to write content).
  - Database as a full page and inline inside a page.
  - **Saved views** (E22, after the table): tabs above a database, each a table or a board with its own layout,
    sort, filters, hidden properties, widths and grouping. Add, rename, duplicate, reorder and delete views.
  - **Board view** (E22): cards in columns, one per option of a select property (plus "No value"), in the option
    order; drag a card to change its value or its place. Moved from Phase 2 by the owner (2026-10-31, ADR-075).
- `[[page links]]` with autocomplete; backlinks panel.
- Markdown export (page and workspace) and import.
- **Bookmarks** (after the table view): paste a link as a preview card (title, description, icon, image).
  Fetching previews is Leaf's first network access and is opt-in, off by default (E15).
- Trash: restore and permanent delete.
- **Page top bar** (E18): the page's place (breadcrumb), when it was last edited, a Favourites star and a `⋯` menu.
  The edited time counts changes to the blocks (undo and redo included), title, icon and cover, not opening,
  moving, trashing or starring. Shown as "Edited just now" (under a minute), "Edited 3 minutes ago", "Edited 2
  hours ago" or "Edited 100 days ago" (always days after the first day), with the exact date and time on hover.

## Phase 2

Calendar and gallery views; boards grouped by other property types; relations and rollups; toggles, columns; templates; graph view;
cloud sync and team workspaces; read-only web viewer (E13); MCP server.

## Phase 3

Full web editor (E14), kept in sync with the desktop app through a Leaf sync server backed by Postgres.
The desktop app stays offline-first on SQLite. Web comes before mobile.

## Later

Mobile; P2P collaboration (maybe).

---

## Editor behaviour (canonical rules)

| Situation | Result |
|---|---|
| `Enter` mid-text | Split block at cursor; new block keeps the type for lists/to-dos, becomes paragraph for headings/quote. |
| `Enter` on empty list item / to-do / action item | Outdent one level; at root, convert to paragraph. |
| `Enter` in code block | Newline inside the block. `Cmd/Ctrl+Enter` or `Enter` on two trailing empty lines exits. |
| `Enter` in a callout | Line break inside the callout. `Enter` on an empty last line (or in an empty callout) removes that line and adds a paragraph after it; `Cmd/Ctrl+Enter` leaves from anywhere. |
| `Backspace` at start of non-paragraph block | Convert to paragraph (keep text). |
| `Enter` in an action item | Split like a to-do: the new action item is not done and has no date, and keeps the priority. |
| `Cmd/Ctrl+Enter` in a to-do or action item | Tick it off, or back on; the caret stays. |
| `[!] ` typed at the start of a block | Turn it into an action item (also in the slash menu and Turn into). |
| An action item's date | Shown as Today / Tomorrow / Yesterday, else e.g. "Fri 31 Oct" (with the year when not this year), plus the time; overdue (red) when not done and due on an earlier day, or today at a time already passed. |
| `Cmd/Ctrl+Shift+D` / `Cmd/Ctrl+Shift+P` in an action item | Open its date picker (calendar, Today / Tomorrow / Next week, a typed time, Clear) or its priority menu (arrow keys, Enter, Escape). |
| `Backspace` at start of paragraph | Merge text into previous text block; children move to previous block. If previous is a divider or image, select it instead. |
| `Delete` at end of block | Merge next block into this one. |
| `Tab` / `Shift+Tab` | Indent under previous sibling / outdent to parent's level. Inside a code block: indent / outdent the caret's or selected lines by four spaces instead. |
| Arrow up/down at first/last visual line | Move to previous/next block, keeping horizontal position. |
| Paste multi-line text | Each line becomes a paragraph; markdown syntax is converted. |
| Paste a Markdown table (header, `|---|` line, rows) | Becomes a table with a header row; cells keep their formatting. Rows separated by blank lines still join it when they start with `|`. Pasted into a cell, its cells go in as lines of text. |
| Copy or paste an action item as Markdown | Written as `- [ ] Send invoice (due 2026-10-31 14:00, high)` (`(action item)` when it has no date or priority); pasting such a line makes an action item. Other brackets stay text, and plain `- [ ]` lines stay to-dos. |
| Copy a table, or selected cells | Other apps get a Markdown table (plain text: tab-separated rows); pasted back into Leaf it is a table again. |
| Paste a lone web address (`https://…`, `www.…`) | It goes in as a link whose text is the address; over selected text, that text becomes a link to it. Into a code block it stays plain text. |
| `[[` typed in text (not code) | Opens a list of pages under it, filtered as you type (fuzzy, by title, with each page's icon and where it lives); pages in the trash and the open page aren't offered. Up/Down choose, `Enter` or a click links the page in place of the `[[` and what was typed (one undo step), `Escape` closes it and leaves the text. When no title is exactly what was typed, the last entry is "New page", which makes that page inside the open one and links it. The slash menu's "Link to page" opens the same list. |
| A link to a page in the text (E10) | Shown as the page's icon and current title, underlined in the link colour; a page in the trash shows greyed as "Title (in trash)", a deleted one as "Deleted page". A plain click opens the page (not for a deleted one). The caret steps over it in one press, `Backspace`/`Delete` remove it whole, and typing beside it never joins it. Copied out as `[[Title]]` (Markdown) or the title (plain text); pasting `[[Title]]` links it when exactly one page outside the trash has that title. Search leaves links out. |
| Copy with all of one block's text selected, or with only the caret in a block | Copies the whole block with its type and formatting. Pasted on an empty line it comes back as the same block; pasted into text, just its text goes in. `Cut` with only the caret does nothing. |
| Deleting a block with children | Children are deleted too (one undo step restores all). |
| `Tab` / `Shift+Tab` in a table cell | Move to the next / previous cell (caret at the end of its text). `Tab` in the last cell adds a row and moves into it. |
| Arrow keys in a table cell | Move within the cell's text; at its edge, Left/Right go to the neighbouring cell and Up/Down to the cell above/below in the same column. Past the table's edge, to the block before/after it. Coming into a table, Up/Down pick the column under the caret. |
| `Enter` in a table cell | Line break inside the cell. |
| `Backspace` at start of a table cell / `Delete` at its end | Nothing (cells never merge). If every cell is empty, `Backspace` deletes the table (one undo step). |
| `Backspace` / `Delete` next to a table, or with it selected | The table is selected first; pressed again, it is deleted (one undo step). |
| Text selected (mouse or keyboard), in one block or across blocks | A floating bar appears just above the selection (below it when there is no room): Turn into, Bold, Italic, Strikethrough, Code, Link, and Link to page (a page icon; for words in one block, E10-S10: it opens the `[[` page list searching for the selected words, and the chosen page's link replaces them in one undo step; Escape gives the words back). A mark button is on when all the selected text has the mark; a click toggles it like its shortcut and keeps the selection. Link opens the link popover; Turn into changes every block the selection touches. No mark buttons in a code block, no Turn into when a selected block can't change kind. The bar goes when the selection collapses, while dragging, or when a menu or the link popover is open. |
| Selecting from one cell into another | Selects whole cells (the rectangle between them). `Backspace`/`Delete`/typing empty them, copy gives them as a table, formatting applies to all of it. A selection reaching from outside into a table takes the table whole. |

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

Evidence for each item: [PHASE0_ACCEPTANCE.md](PHASE0_ACCEPTANCE.md).

- [ ] Write and edit a 3,000-word document with mixed blocks without a single editing bug.
- [x] Every rule in "Editor behaviour" has an automated test.
- [x] Undo/redo restores exact state for every command, including block moves.
- [ ] Kill the process mid-typing; reopening shows all text older than 1 s.
- [x] Page tree with 500 pages, 6 levels deep, stays smooth.
- [x] `Cmd/Ctrl+K` finds a word inside any page of a 5,000-page workspace and jumps to it; results appear in < 50 ms.
- [ ] Performance budgets met on a mid-range laptop.
- [x] CI green on macOS, Windows, Linux with ≥ 90% coverage on core crates.
