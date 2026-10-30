# Architecture

How Leaf is put together, in plain English. For the libraries and the rules behind them see
[TECH_STACK.md](TECH_STACK.md); for the data model see [BLOCK_MODEL.md](BLOCK_MODEL.md); for why each big
choice was made see the [decision records](../adr/README.md).

## The big picture

Leaf is one desktop program built from eight Rust crates (libraries). They are split so that everything
that decides *what happens* when you edit can be tested without opening a window, and the part that draws
the screen stays thin.

```
                 ┌──────────────────────────────────────────────┐
  keyboard,      │  leaf-app      the program: opens windows,   │
  mouse   ─────▶ │                wires everything together     │
                 │  leaf-ui       draws the screen (GPUI)       │
                 └───────────────────────┬──────────────────────┘
                     key presses ▼       ▲ view models (what to draw)
                 ┌───────────────────────┴──────────────────────┐
                 │  leaf-editor   cursor, selection, key rules  │
                 └───────────────────────┬──────────────────────┘
                        commands ▼       ▲ documents
                 ┌───────────────────────┴──────────────────────┐
                 │  leaf-model    documents (Loro), commands,   │
                 │                undo, invariants              │
                 └───────────────────────┬──────────────────────┘
                   changes to save ▼     ▲ saved documents
                 ┌───────────────────────┴──────────────────────┐
                 │  leaf-store    SQLite file on disk           │
                 └──────────────────────────────────────────────┘
  leaf-core: IDs and error types shared by all.   Also: leaf-search (search index). Phase 1: leaf-export.
```

## The crates

| Crate | Job | Must not |
|---|---|---|
| `leaf-core` | Shared IDs, error types, small helpers. | Depend on any other Leaf crate. |
| `leaf-model` | The documents. The only crate that changes them, and only through `Command`s. Undo lives here. | Know about the screen, files or SQLite. |
| `leaf-editor` | All editing behaviour: what Enter, Backspace, Tab, paste, the slash menu do. Turns input into commands and documents into view models. | Draw anything, or change documents except through commands. |
| `leaf-store` | Saves and loads documents in a SQLite file; autosave and crash recovery. | Know about the screen. |
| `leaf-export` | Markdown import/export (Phase 1). | Touch files or the screen. |
| `leaf-search` | Full-text search index: a disposable SQLite FTS5 file kept in step with saved pages (ADR 0019). | Hold data that exists nowhere else. |
| `leaf-fixtures` | Big, repeatable test documents (a 10,000-block page, a 5,000-page workspace) for tests and benchmarks, fixed by a seed (ADR 0055). Test support only. | Be used by the app. |
| `leaf-bench` | Benchmarks for the performance budgets (open a page, type, update the view, save, search), run by `just bench` and on every `main` push in CI (ADR 0056, 0058). Test support only. | Be used by the app. |
| `leaf-ui` | Draws view models with GPUI and passes input back. Wraps every gpui-component widget. Holds Leaf's look (colours, bundled fonts, sizes) in one `theme` module (ADR 0027). | Contain editing logic. |
| `leaf-app` | The program itself: starts up, opens windows, connects the crates. | Contain logic that a testable crate could hold. |

The first six are **core crates**. They, `leaf-fixtures` and `leaf-bench` never depend on GPUI or `leaf-ui`; a test
(`crates/leaf-app/tests/architecture.rs`) fails if one does. `leaf-core`, `leaf-model`, `leaf-editor` and
`leaf-export` also build for WebAssembly, which keeps a web version possible later.

## How a keystroke becomes a saved change

This is the target design; each step lands with its epic. Built so far: the window shell (E00-S04), the
IDs in `leaf-core` (E01-S01), and `leaf-model`'s `Workspace`, which loads and saves the page tree and
answers "which pages, in what order, with what titles" (E01-S02), and the first commands that change it:
create, rename and set an icon (E01-S03), move or reorder pages (E01-S04), trash,
restore and permanent delete (E01-S05), and undo/redo of all of them (E01-S06). Each page's own document, `PageDoc`, holds its blocks:
the block schema and reading text with formatting (E02-S01), inserting, deleting and moving blocks (E02-S02),
editing text and formatting (E02-S03), indent/outdent (E02-S06), and changing a
block's kind and properties (E02-S04). The page is a **virtual list** (E07-S01, ADR 0035): only the rows on screen are laid out,
and each frame's rows are compared with the last to re-measure or splice just what changed, so a 10,000-block page
scrolls smoothly. Blocks are drawn by a **rich text view** (E07-S02, ADR 0038): GPUI `StyledText` runs carry the marks,
the selection is a run background, an overlay draws the blinking caret and records the painted layout, which answers
the editor's `LineLayout` for Up/Down. The page **takes the keyboard itself** (E07-S03, ADR 0040): keys are actions
bound in the page's key context and call `PageEditor` (`shell/page_keys.rs`); typed text, input-method drafts and the
emoji picker arrive through GPUI's platform text input (`shell/ime.rs`), which sees the caret's block with the draft
spliced in; the mouse places the caret, drags select, double and triple clicks select a word and the block. (The first
version, E07-S10, used one text input per block; those are gone.) Each row has a **⋮⋮ handle** that shows on hover and
opens the **block menu** (E07-S04, ADR 0041): Turn into, Duplicate, Move up, Move down, Delete, with icons. There is no toolbar of block types above the page: the slash menu, this menu and Markdown shortcuts
change a block's type (E07-S11, ADR 0057). The menu's
state lives in `PageEditor` (`leaf-editor/src/block_menu.rs`) like the slash menu; `shell/block_menu.rs` draws it, gives
it the keyboard while open, and `Cmd/Ctrl+/` opens it for the caret's block. Dragging the handle is worked out in
`leaf-editor/src/drag.rs` (E05-S04, ADR 0049): the UI passes the rows it drew and the pointer, `PageEditor::drop_target`
returns where the line goes (a gap between rows and a depth), and `drop_blocks` moves the blocks there as one undo step.
`shell/block_drag.rs` (E07-S05, ADR 0050) uses GPUI's drag: a click on the handle still opens the menu, pressing and
moving starts a drag with a preview of the block's first line, each row records where it was drawn, the line is drawn at
the answer's depth, the dragged rows fade, and holding near the page's top or bottom edge scrolls it. `Cmd/Ctrl+K` over
selected text opens the **link popover** (E07-S07, ADR 0052, `shell/link_popover.rs`): an address field under the
selection, filled in when the selection is already one link, with Open and Remove; Enter saves through
`PageEditor::set_link`. `Cmd/Ctrl+click` on a link opens it; `leaf_editor::openable_url` decides which addresses the
system may open. A **code block** shows its language at the top right on hover; a click opens a searchable list
(E07-S09, ADR 0054, `shell/language_picker.rs` over `leaf_editor::code_block`), and Tab and Shift+Tab inside code indent
and outdent lines by four spaces instead of nesting the block. The **slash menu** is drawn as a popover
by the `/` (E07-S06, ADR 0043, `shell/slash_popover.rs`): below the line, or above it near the window's bottom edge.
The page keeps the keyboard, so typing filters and the editor's keys drive it; the mouse highlights and picks. `PageEditor`'s rules stay. Enter splits a block at the cursor and Backspace at the start merges it into the block
above (E02-S05), keeping formatting on both sides. Pages have their own undo and redo, carrying the caret with each step (E02-S08);
the editor wires Cmd+Z to it in E04-S08. The editor's caret (E04-S02) moves by
whole characters as a reader sees them (an emoji, a Hindi syllable), jumps by word, and goes to line start and end; it
is anchored to the text, so edits before it, undo and other devices' changes carry it along (ADR 0015). Up and Down
(E04-S03) ask the UI where lines wrap through the `LineLayout` trait and keep the caret's column (ADR 0016). Typing (E04-S04) replaces the selection with one
`ReplaceText` command; an input method's draft is kept in the editor and only the committed text reaches the page
(ADR 0017). Enter, Backspace, Delete and Tab (E04-S05) go through `PageEditor::press`, which applies the
"Editor behaviour" rules at the caret; each press is one undo step, and a divider or image next to the caret is
selected rather than merged (ADR 0020). A selection (E04-S06) is either text between the caret and a tail anchor, possibly
across blocks, or whole blocks (Cmd/Ctrl+A twice); typing or a key over it is still one undo step (ADR 0021). Formatting shortcuts and links (E04-S07) toggle marks over the whole selection,
or, with just a caret, for the next typed text (ADR 0022). Undo puts back the selection from just before a change and redo the one right
after it (E04-S08, ADR 0023). Typing a Markdown shortcut at the start of a block (`# `, `- `, `[] `, …) converts
it, as an undo step of its own (E05-S01, ADR 0025). Typing `/` at the start of a word opens the slash menu (`PageEditor::slash_menu`),
which filters block types by the typed query and converts the block or adds one below (E05-S02, ADR 0026). The block menu's Turn into, Duplicate and Delete (`PageEditor::block_action`) act on
the selection when the block is part of it, as one undo step each (E05-S03, ADR 0028). Copy and cut turn the selection into a `Fragment` (formatted text or whole blocks)
that the UI puts on the system clipboard as versioned JSON; paste puts one back at the caret, splitting the line for
blocks, as one undo step (E05-S05, ADR 0030). Text from other apps goes through `paste_text`, which reads Markdown into blocks;
copying also writes Markdown and plain text for them (E05-S06, ADR 0032). The window's sidebar lists real pages (`WorkspaceEditor` in `leaf-editor`): "New page" and typing a title go
through those commands (E06-S07).

**Window, sidebar and Settings (E06-S02).** At startup `leaf-app` loads the app settings (`settings.json` in the
OS's settings folder; format in `leaf_store::settings`, ADR 0029) and opens the window where it was, if that is still
on a screen. The sidebar is resizable and can be hidden (Cmd/Ctrl+\\, or its collapse button; a ☰ button in the
page's corner, or at the left of the page's top bar, brings it back, `shell::sidebar_toggle`, E06-S18); its width, its visibility and the window
frame are written back on blur and quit. A cog at the foot of the sidebar (or Cmd/Ctrl+,) opens the Settings dialog:
Appearance (System / Light / Dark), Workspace (its folder) and About. Menus and shortcuts live in `leaf_ui::menus`.

**Page top bar (E18-S03, ADR 0065).** Above the open page, `shell::top_bar` draws the breadcrumb
(`WorkspaceEditor::breadcrumb`), "Edited 3 minutes ago" (`edited_ago`, redrawn every 30 seconds; the exact local
time on hover via `chrono::Local`), the Favourites star and the `⋯` page menu. The page menu's entries come from
`WorkspaceEditor::page_menu`; `shell::page_menu` draws them for the `⋯` and for the sidebar's right-click menu and
runs the chosen action (E18-S04). Its "Full width" switch sets the page's `full_width` (E18-S06); the shell's
`column` then drops the reading-width cap for that page's header and blocks.

**Formatting bar (E07-S13, ADR 0067).** While text is selected, `shell::format_bar` floats a bar above the selection
with Turn into and the mark buttons. `PageEditor::format_bar` says what it offers and which marks are on; clicks go
through `toggle_mark`, the link popover and `BlockAction::TurnInto`, like the keyboard and the block menu. The Turn
into list is drawn in the bar's overlay so the page keeps the keyboard and the selection.

**Sidebar page tree (E06-S03).** The sidebar lists the page tree (`WorkspaceEditor::sidebar` in `leaf-editor`):
children show under expanded pages, and which pages are expanded is saved in the workspace folder's `view.json`
(`leaf_store::ViewState`), not in a document. So is the open page, which opens again when the workspace
is next opened (E06-S19). Right-click offers Add page inside, Rename (inline) and Move to Trash;
dragging a row shows a line (above or below) or a ring (inside) and drops through `WorkspaceEditor::move_page`, which
turns the place on screen into a model index (indexes count trashed pages; ADR 0031). Only rows on screen are drawn
(`uniform_list`), so 500 pages scroll smoothly. The UI code is `leaf_ui::shell::tree`.
The sidebar's frame (`leaf_ui::shell::sidebar`, E06-S10) has a workspace header (badge, name, compose button), a
"Pages" heading with a "+", and Settings at the foot; rows show a page icon and a hover "+" that adds a page inside.
Starred pages show in a "Favourites" section above "Pages" (`leaf_ui::shell::favourites`, E18-S05), drawn from
`WorkspaceEditor::favourites` on every frame, so renames, icons, the trash and restoring show at once; hidden while
nothing is starred. Its right-click menu is the page menu for `MenuPlace::Favourites`, whose Rename edits the
Favourites row rather than the tree row.
Extra Lucide icons are bundled one by one in `leaf_ui::assets()`, not the whole set.

**Quick switcher (E06-S04).** `leaf_editor::QuickSwitcher` is a snapshot of the live pages with breadcrumbs, matched by
`leaf_editor::fuzzy_match` as the owner types, recently opened pages first when nothing is typed (ADR 0033). Since
E11-S06 it is no longer a popup of its own: the search bar uses it for title matches.

**Search bar model (E11-S05).** `leaf_editor::SearchBar` extends the switcher with full-text results while staying free
of SQLite: it matches titles at once and, after a 150 ms pause in typing, asks the app for a search
(`due(now)` → `SearchRequest { id, query }`). The app runs it against `leaf_search` and hands the hits back as plain
`SearchHit`s (`deliver(id, hits)`); hits for any but the newest query are dropped. Rows are grouped by page (title
matches first, matching blocks under each page), and Enter gives the page and block to open (ADR 0042).

**Search bar (E11-S06).** Cmd/Ctrl+P (File › Search…) and Cmd/Ctrl+K open `leaf_ui::search`, a popup over the
`SearchBar`. Cmd/Ctrl+K checks first: over selected text in the page it is the link action (E07-S07), not search.
When the bar says a search is due (a gpui timer at `next_deadline()`), the popup runs it on the background executor
with a read-only `leaf_search::Searcher` on the workspace's index, and converts the results to `SearchHit`s. With no
index yet, titles still match. Choosing a result calls `Shell::open_at`, which opens the page with the matching
block at the top, the caret at its start and a tint that fades out (ADR 0046). A "Search" field under the sidebar
header (`shell::sidebar_search`, E06-S17) opens the same popup on click; it holds no text, and its `⌘K` / `Ctrl+K`
hint comes from `menus::SEARCH_KEYS`, the keys the shortcut is bound to.

**Page header (E06-S05).** Above the title sits the page's emoji icon, or an "Add icon" button on hover
(`leaf_ui::shell::header`). Clicking it opens `leaf_ui::icon_picker` in a popover over `leaf_editor::EmojiPicker`
(emoji from the `emojis` crate, searchable by name, shortcode and group); the choice goes through
`WorkspaceEditor::set_icon` (ADR 0034).

**Inserting images (E05-S07).** "Image" in the slash menu removes the typed `/image` and asks the UI, once, to show
the file picker (`PageEditor::take_image_request`); a paste from another app brings a screenshot, a copied image or
a copied image file (`leaf_editor::paste_kind` picks Leaf blocks, then text, then a picture). The UI
(`leaf_ui::shell::image`) stores the picture in the asset store and calls `PageEditor::insert_image`, which turns an
empty block into the image or adds it below, with the caret in its caption (ADR 0051).

**Drawing images (E07-S08).** `leaf_ui::shell::image_row` draws the picture from `assets/` with GPUI's image loader,
which reads it in the background the first time its row is on screen; a frame shows while it loads. Its size comes
from `leaf_editor::image_display_size`: the stored width (or the natural one), shrunk to the room the row has. A bar on
the right edge resizes it; the width is saved once on release with `PageEditor::set_image_width`, one undo step
(ADR 0053).

**Tables (E16-S01).** A simple table is a `table` block holding `table_row` blocks holding `table_cell` blocks
(`leaf-model`, ADR 0061). Cells are text blocks, so typing, marks, the caret, undo, saving and search work on them
unchanged; the table commands (`InsertTable`, and insert, delete and move a row or column) are the only way to
reshape a table, and the general block commands refuse to touch a row or cell on its own. In the editor
(`leaf_editor::table`, E16-S02) the view lists a table's cells right after it, row by row
(`BlockStyle::Table`, `BlockStyle::TableCell { row, column, header }`); Tab moves between cells (adding a row at
the end), Enter types a line break, Backspace and Delete never merge cells, Up/Down keep to the column, and a
selection from one cell into another is `Selection::Cells`, the rectangle between them (ADR 0063). Anything that acts on blocks
(block menu, drag, nesting, a selection reaching into a table) takes the table whole. In the window
(`leaf-ui/src/shell/table_row.rs`, E16-S03) the table's row draws the whole grid and the cell rows after it draw
nothing, so the list stays in step with the view; each cell uses the same rich-text drawing as any block, so clicks,
the caret, selection and marks work unchanged. Columns are a fixed 160 px; a table wider than the page scrolls
sideways inside its own box (a horizontal swipe or Shift+wheel), never the page. Hover controls ("+" under and
beside the grid, a handle per row and column with a menu) call `PageEditor::edit_table` with a `TableEdit`: one
table command and one undo step, the caret following the edit. Tables come in and go out through `/table` (a 3 × 3
table with a header row) and the clipboard (E16-S04): pasting a Markdown table builds one with the table commands,
copying writes one, and Leaf's own fragment keeps `table`, `table_row` and `table_cell` blocks (ADR 0032's update).

**Databases (Phase 1, E09).** A database is one Loro document of its own (`leaf_model::Database`, ADR 0066): column
order and row order in two flat trees, the columns' schema, every cell, and the table view's sort and filters. It
changes only through `DatabaseCommand`s, each one commit and one undo step, like pages. Each row is a page whose title
and body live in the workspace and the page's own document (E09-S03 stores them); the database document holds only
row IDs, order and property values, so a table view never opens a document per row. See BLOCK_MODEL.md "Database
doc".

Databases are saved like pages, under `database:<id>` (`DocKey::Database`, E09-S03). A database page's meta names its
database, and each row page's meta names the database it belongs to. Row pages sit under the database page and the
sidebar leaves them out, but search finds them like any page. `WorkspaceEditor` (`workspace_editor/databases.rs`) adds
and deletes rows. The database document decides which rows exist, and each row page's trash state follows it:
- the row pages' part is written outside the workspace's undo (`Workspace::apply_unrecorded`), so adding or deleting a
  row is one undo step, the database's;
- after a database undo or redo, `reconcile_rows` trashes or restores row pages to match;
- restoring a row page from the Trash restores its row where it was.

**Page cover (E06-S11).** A page can have a cover: a full-width band above the header, with the icon overlapping
its bottom edge (`leaf_ui::shell::cover`). The page stores a name such as `builtin:dawn` next to its title and icon;
`leaf_editor::PageCover` reads it as a built-in cover or an unknown one, which is kept and drawn as a plain band.
Six built-in covers (Dawn, Forest, Ocean, Paper, Night, Travel; E06-S12) are files in
`crates/leaf-ui/assets/covers/` (`leaf_ui::covers`): SVG, except Travel, a WebP made with an image model (E06-S15).
Sixteen calm solid colour and gradient covers (E06-S16) are SVG files too; `BuiltinCover::section()` groups them
for the picker, a gallery of thumbnails four to a row with the current cover outlined (ADR 0064).
GPUI's `img()` draws both; the band's height follows its width
(3.5:1, 160–320 px) so wide and narrow windows crop little of the picture.
"Add cover" sits beside "Add icon"; "Change cover" opens `leaf_ui::cover_picker` and "Remove" clears it, both through
`WorkspaceEditor::set_cover` (ADR 0045).

**Trash (E06-S06).** The Trash row at the foot of the sidebar opens `leaf_ui::trash_dialog` over
`leaf_editor::TrashList` (pages trashed on their own, newest first, filterable). Restore and delete forever go through
`WorkspaceEditor`; a permanent delete saves the workspace first, then removes the pages' documents with
`Store::delete`, and search forgets them on its next pass (ADR 0036). A trashed page opened from the dialog shows a
banner with Restore page and Delete forever.

**Startup and saving (E03-S08).** `leaf-app` opens a window whose top view (`leaf_ui::AppView`, E06-S09) opens the
workspace folder given as its first argument, else the one opened last; if there is none (first launch) or it can't
be opened, it shows the welcome screen (`leaf_ui::welcome`) with the reason, where workspaces are created, opened
and switched (ADR 0039). The workspace name at the top of the sidebar also opens a menu for switching to a recent
workspace or making a new one; it sends the same actions as the File menu. Opening a workspace adds it to the recent list in the app settings (ADR 0037). It loads the workspace
document; each page's blocks load from disk the first time the page is opened. A 100 ms timer in the window calls
`Autosave::tick` from `leaf-store`, which polls every open document for unsaved changes and saves those that are
due. Window blur, quit (Cmd+Q / Ctrl+Q, or the app menu) and window close save everything at once. If the folder
can't be opened (another Leaf has it open, it is damaged), the window shows why instead of the editor.

**App icon and identity (E08-S05).** `crates/leaf-app/resources/` holds the icon: `icon.svg` is the master, and
`just icons` generates the PNG files, `Leaf.icns` and `leaf.ico` from it (committed, so builds need no image tools). On
macOS `just run` launches Leaf from a `Leaf.app` bundle so the Dock shows its icon, and `just install` builds an
optimised Leaf and puts that bundle in `/Applications` (E08-S07); on Windows `build.rs` builds the
icon into `leaf.exe`; on Linux the window carries the `app_id` `io.github.milinpaul.Leaf` and, on X11, the icon
itself (ADR 0018).

1. **You press a key.** GPUI delivers it to `leaf-ui`, which forwards it untouched to `leaf-editor`.
2. **The editor decides what it means.** For example, Enter in the middle of a paragraph means "split this
   block here". It produces a `Command` such as `SplitBlock { id, pos }`.
3. **The model applies the command.** `leaf-model` changes the Loro document in one step (one commit), checks
   the document's rules still hold, and records one undo step.
4. **The screen updates.** The page reports which blocks the change touched (`PageDoc::take_changes`, from Loro's
   change events). The editor's `PageView` re-reads only those blocks and re-walks the outline only when blocks
   moved or changed kind, then `leaf-ui` draws the rows. Typing therefore costs one block's read however long
   the page is; the goal is under 16 ms from key press to pixels (ADR 0014).
5. **The change is saved.** First the window records when each changed page was edited
   (`WorkspaceEditor::stamp_edits`, ADR 0059): any page whose blocks changed since the last tick, or whose title,
   icon or cover changed, gets a `MarkEdited` command in the workspace doc, which undo skips. Then the document hands `leaf-store` one small update with everything changed since the last
   save (`unsaved_changes`), which is appended to the SQLite file; then the document is marked saved. `Autosave`
   decides when: at most 300 ms after the first unsaved change, and at once on blur or quit, so a crash loses at
   most about a second of typing. When a document has collected more than 200 updates, that save also folds them
   into one fresh snapshot, in one transaction, so loading never replays a long tail (`Store::compact`, ADR 0048).

6. **Search catches up.** When autosave wrote something, the window pokes the search indexer (`SearchWorker`
   in `leaf-search`), which runs on its own thread so typing never waits for it. It asks the store which
   version of each document is saved (`Store::saved_versions`), compares that and the page titles and trash with
   what it indexed, and re-reads only the pages that changed into `search/index.sqlite`. An edit is searchable
   well within a second (ADR 0024).

Undo runs the same path backwards: the model reverses the last step and the editor redraws.

**Images (E03-S07).** `Workspace::import_asset` copies an image into `assets/`, named by the BLAKE3 hash of its
bytes, and records its type (read from its first bytes, not its name) and size in the `assets` table. The file is
written atomically before its row, and the same picture is stored once (ADR 0047). Image blocks refer to it by
that hash (`AssetId`).

**Search (E11).** The index is a separate, throwaway SQLite file (ADR 0019). `leaf-search` reads pages through a
`PageSource` (the UI supplies one over a second `leaf-store` connection, `Workspace::reader`), so it never sees
unsaved text and can always be rebuilt from disk. On open, a new, damaged or half-built index is rebuilt in the
background, and pages saved but not yet indexed (for example just before a crash) are caught up. Any index error
is logged and repaired by resetting the index and rebuilding; it never stops editing or saving. Searching
(`Searcher`, a read-only connection) matches every typed word as a prefix, ignoring case and accents; pages whose
title matches come first, then the best match, then the most recently edited; each result has highlighted
excerpts of up to three matching blocks (E11-S03). The search bar that shows them is E11-S05/06.

**Search speed (E11-S07).** On 5,000 pages / 100,000 blocks a search takes about 20 ms and re-indexing an edited
page about 5 ms (`just bench-search`). To stay there, only the newest 10,000 matching blocks are ranked, each page's
best three blocks are picked before excerpts are made, prefix indexes start at one letter, and the indexer keeps the
list of searchable pages until titles or the page tree change (ADR 0058).

## Where things live

```
crates/            the eight crates above
docs/product/      what we build and why (PRODUCT_SPEC.md)
docs/architecture/ how it is built (this file, TECH_STACK.md, BLOCK_MODEL.md)
docs/adr/          decision records
backlog/           epics and stories (the work)
```

Keep this file current: update it in the same PR as any change to crates, data flow or saving.
