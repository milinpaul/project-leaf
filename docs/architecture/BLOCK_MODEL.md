# Block Data Model

## Concepts

- **Workspace:** a set of pages stored in one directory.
- **Page:** a titled document that contains blocks and can contain child pages.
- **Block:** a unit of content (paragraph, heading, list item…) with a stable ID. Blocks can have child blocks (nesting).
- **Command:** the only way to change a document. One command = one Loro commit = one undo step.

## Documents (Loro)

The workspace is split into one **workspace doc** plus one **page doc per page**. Per-page docs keep loading fast, keep sync granular and let large workspaces scale.

### Workspace doc

```
LoroDoc (workspace)
└── "pages": LoroTree          # page hierarchy, fractional index ordering enabled
    └── node (TreeID)
        meta: LoroMap
          "id":         String  # PageId (uuid v7), stable forever
          "title":      String
          "icon":       String? # single emoji
          "cover":      String? # cover name, e.g. "builtin:dawn"; unknown names kept (ADR 0045)
          "full_width": bool?   # true = title and blocks use the whole page area; missing = reading column (E18-S06)
          "created_at": i64     # unix ms
          "edited_at":  i64?    # unix ms, last change to blocks/title/icon/cover; missing = created_at (ADR 0059)
          "trashed_at": i64?    # set = in trash (subtree hidden)
          "database":   String? # DatabaseId: this page is a database (Phase 1, ADR 0066)
          "row_of":     String? # DatabaseId: this page is a row of that database; left out of the sidebar
└── "favourites": LoroMap      # PageId -> position (i64); listed by position, then id (ADR 0060)
```

Titles live here, not in page docs, so the sidebar renders without opening pages.

### Page doc

```
LoroDoc (page, keyed by PageId)
└── "blocks": LoroTree         # block hierarchy, fractional index ordering enabled
    └── node (TreeID: internal handle, may change on undo/redo)
        meta: LoroMap
          "id":    String      # BlockId (uuid v7), stable forever; see ADR 0006
          "type":  String      # see Block types
          level, checked, language, asset, width, alt, header_row   # type-specific properties, directly in meta (ADR 0007)
          "text":  LoroText    # present for text-bearing types; create with ensure_mergeable_text
```

Root nodes of the `"blocks"` tree are the page's top-level blocks, in order.

### Database doc (Phase 1, ADR 0066)

```
LoroDoc (database, keyed by DatabaseId)
├── "columns": LoroTree (flat)  column order; node meta "id" = ColumnId
├── "schema":  LoroMap          ColumnId -> LoroMap
│     "name":    String
│     "type":    String         text | number | select | multi_select | date | checkbox | url; others kept, read-only
│     "hidden":  bool?
│     "width":   f64?
│     "options": LoroMovableList of LoroMap { "id": OptionId, "name", "color" }   # made with the column
│     ...unknown keys kept
├── "rows":    LoroTree (flat)  row order; node meta "id" = the row's PageId, "deleted": true once deleted
├── "cells":   LoroMap          PageId -> mergeable LoroMap: ColumnId -> value
│     text, url: String · number: f64 · date: i64 (unix ms) · checkbox: bool · select: OptionId as String
│     multi_select: mergeable LoroMap OptionId -> true
└── "view":    LoroMap          "sort": [{column, descending}], "filters": [{column, op, value}] (plain values)
```

Order and existence are kept apart (ADR 0066, "Correction"):
- the trees only order; a column exists while its schema entry does, and a row until its node is marked `deleted`;
- nodes never leave their trees, so a delete on one device survives a move of the same node on another;
- cells of deleted rows and columns stay, unread, so undo brings them back whole.

`Database` (`leaf-model/src/database.rs`) reads cells by what is stored. A value written under an earlier type reads
back as itself, and the editor converts it for display (ADR 0066 answer 3).

## IDs

| Thing | ID | Stability |
|---|---|---|
| Page | `PageId(Uuid)` v7 | Global, never changes. |
| Block | `BlockId(Uuid)` v7, stored in the block's meta `id` | Never changes: survives moves, merges, undo and redo. Unique within a page; paste and duplicate mint new ones. |
| Block reference (Phase 1 links) | `(PageId, BlockId)` | |
| Asset | `AssetId` = blake3 hash of bytes | Content-addressed, deduplicated. |

Loro's `TreeID` is **not** an ID: undo and redo re-create nodes under new `TreeID`s
([ADR 0006](../adr/0006-stable-block-ids.md)). It never leaves `leaf-model`, which maps `BlockId → TreeID`
and rebuilds that index after undo, redo and import.

## Block types

| `type` | `props` | `text` | Children allowed |
|---|---|---|---|
| `paragraph` | — | yes | yes |
| `heading` | `level`: 1–3 | yes | no |
| `bulleted_list` | — | yes | yes |
| `numbered_list` | — | yes | yes |
| `todo` | `checked`: bool | yes | yes |
| `quote` | — | yes | yes |
| `code` | `language`: String | yes (plain, no marks) | no |
| `divider` | — | no | no |
| `image` | `asset`: AssetId (hex), `width`: f64?, `alt`: String | caption | no |
| `table` | `header_row`: bool | no | `table_row` blocks only (at least one) |
| `table_row` | — | no | `table_cell` blocks only (at least one) |
| `table_cell` | — | yes | no |
| `task` | `done`: bool, `due`: `YYYY-MM-DD`?, `due_time`: `HH:MM`? (only with `due`), `priority`: `high` \| `medium` \| `low`? | yes | yes |
| `callout` | `tone`: `info` \| `success` \| `warning` \| `error` \| `note` (another name is kept; missing reads as `info`) | yes | no |

**Callouts (E21, ADR 0069).** A `callout` is text with marks in a tinted box with an icon; its lines are separated by
`\n` in its one text, like a code block, and nothing nests inside. The `tone` is stored by name; a tone from a newer
version reads as `CalloutTone::Other` and is written back unchanged.

**Action items (E19, ADR 0068).** A `task` is a to-do with a due date, an optional wall-clock time and a priority.
Dates and times have no time zone: due Friday stays due Friday. An absent prop means "not set"; a time needs a date.

**Tables (E16, ADR 0061).** A simple table is a `table` block holding `table_row` blocks, each holding `table_cell`
blocks in column order; a cell is a text block, so its text and marks change with the usual text commands and the
caret sits in it like in any block. Columns are positions within rows. Rows and cells exist only inside their table
and change only through the table commands. Commands keep every table a full grid (every row the same length, at
least one row and one column); merging two copies may leave rows of different lengths (one added a column while the
other added a row), which is allowed: a missing cell reads as empty, and the next table command evens the rows out.
A table is not a database (E09): no column types, sorting or filtering, and rows are not pages.

Rust view of the schema (in `leaf-model`):

```rust
pub enum BlockKind {
    Paragraph,
    Heading { level: u8 },          // 1..=3
    BulletedList,
    NumberedList,
    Todo { checked: bool },
    Quote,
    Code { language: String },
    Divider,
    Image { asset: AssetId, width: Option<f64>, alt: String }, // f64: Loro stores numbers as 64-bit
    Table { header_row: bool },     // children: TableRow; built with the table commands
    TableRow,                       // children: TableCell
    TableCell,                      // text with marks
    Unknown { type_name: String },  // from a newer version: read-only, never rewritten
}
```

Unknown `type` values (from a newer app version) are preserved and rendered as a read-only placeholder, never dropped.

## Inline marks (LoroText styles)

| Mark | Value | Expand behaviour |
|---|---|---|
| `bold` | `true` | after |
| `italic` | `true` | after |
| `strike` | `true` | after |
| `code` | `true` | none |
| `link` | `href: String` | none |

Configure once per doc with `config_text_style` before any edit (on create and on every load). Marks this version
doesn't know (from a newer version) get a default style that doesn't expand, so their text stays editable.
`code` blocks never carry marks.

## Commands

Defined as an enum in `leaf-model`. The editor (`leaf-editor`) translates key presses and mouse actions into commands; nothing else writes to Loro.

**Page commands (workspace doc)**: `WorkspaceCommand`, applied with `Workspace::apply`
- `CreatePage { parent: Option<PageId>, index, title, created_at }`: the model mints and returns the `PageId`;
  the caller supplies `created_at` (Unix ms) because the model never reads the clock (it must run in WebAssembly)
- `RenamePage { page, title }`, `SetPageIcon { page, icon: Option<String> }` (`None` clears the icon),
  `SetPageCover { page, cover: Option<String> }` (`None` clears the cover; any non-empty name is stored),
  `SetPageFullWidth { page, full_width }` (off deletes the key; a layout change, so not an edit and no `MarkEdited`)
- `MovePage { page, new_parent, index }`: `index` is the page's final position among its new siblings, counted
  without the page itself; moving a page into itself or one of its descendants is rejected
- `TrashPage { page, at }`: sets `trashed_at`; the page stays in place, hidden with everything inside it from
  `root_pages`/`children` (listed by `trash()` instead). No creating or moving into the trash, and a page in the
  trash can't be moved until restored.
- `RestorePage { page }`: clears `trashed_at`; if the page it lived in is itself in the trash, the page moves to the
  end of the top level so it doesn't stay hidden
- `MarkEdited { page, at }`: sets `edited_at`. The editor applies it after a page's blocks, title, icon or cover
  change (`WorkspaceEditor::stamp_edits`), as its own commit with origin `leaf:edited`, which workspace undo skips:
  undo is an edit too, so it never restores an older time (ADR 0059)
- `DeletePagePermanently { page }`: only for pages in the trash; deletes the subtree (and its favourites) and returns
  every deleted `PageId` (`Outcome::PagesDeleted`) so storage can drop their page docs
- `AddFavourite { page }`: stars the page, placing it last (highest position + 1); already starred → `Unchanged`;
  refused for a page in the trash. `RemoveFavourite { page }`: unstars it; not starred → `Unchanged`.
  `favourites()` hides pages in the trash (restoring brings them back in place); `DeletePagePermanently` removes the
  deleted pages' entries in the same commit. Two devices starring the same page write the same key, so it is
  listed once.
- A command that would change nothing (same title, same icon, same position) writes nothing and returns
  `Outcome::Unchanged`, so it leaves no empty undo step
- Positions (`index`) count every child, including pages in the trash, while `root_pages`/`children` hide them; `child_slots(parent)` lists
  every child (trashed included) and `parent(page)` gives a page's parent, so a caller can turn a place in a listing into
  an `index`

**Block commands (page doc)**: `PageCommand`, applied with `PageDoc::apply` (same rules as workspace commands:
validate, write, commit once; `Unchanged` when nothing would change)
- `InsertBlock { parent: Option<BlockId>, index, kind, text }`: mints and returns the `BlockId`. Kinds that have
  text get a mergeable `text` container; text on a divider, unknown kinds and heading levels outside 1–3 are rejected.
- `DeleteBlocks { ids }`: deletes subtrees and returns every deleted `BlockId` in reading order. Ids inside another
  listed block are fine.
- `MoveBlocks { ids, new_parent, index }`: the moved blocks keep their reading order whatever order they're listed in,
  and stay together; listed blocks inside another listed block travel with it. `index` counts the destination's
  children without the moved blocks.
- All: headings, code, dividers, images and unknown blocks can't contain blocks; no block moves into itself.
- `SetBlockKind { id, kind }`: keeps text when the target type has text; a divider needs empty text (otherwise
  refused, so no words are lost) and its text container is removed; code strips every mark; a kind that can't hold
  children lets them out right after the block, in order; known props the new kind doesn't use are removed, props
  from newer versions kept; unknown kinds can't be converted
- `SetProp { id, prop: BlockProp }`: typed (`Checked`, `HeadingLevel`, `Language`, `ImageWidth`, `ImageAlt`,
  `TableHeaderRow`), and must belong to the block's kind
- `InsertText { id, pos, text }`, `DeleteText { id, range }`: positions are Unicode scalar offsets, checked against the
  text length; blocks without text (dividers, unknown kinds) are refused
- `ReplaceText { id, range, text }`: one commit replacing `range` (typing over a selection, committing an input-method
  composition); the new text takes the marks of the first replaced character; an empty range is `InsertText` (ADR 0017)
- `SetMark { id, range, mark: Mark }`, `RemoveMark { id, range, kind: MarkKind }`: `Mark` is bold, italic, strike,
  code or `Link(href)` (href must not be empty); code blocks take no marks; expand behaviour follows the table above;
  setting a mark the whole range already has, or removing one it doesn't have, is `Unchanged` and writes nothing
- `SplitBlock { id, pos }` (Enter mid-text): the text after `pos` moves, with its marks, into a new block right after
  (or as its first child if it has children); lists and to-dos continue, anything else becomes a paragraph
- `MergeIntoPrevious { id }` (Backspace at start): appends the text, with marks, to the previous block in reading order
  and moves the children there (or into the block's place if it can't hold them); returns `BlockMerged { into, at }`.
  A divider, image or unknown block above is refused with `MergeTargetNotText { target }` so the editor can select it
- `DuplicateBlocks { ids }` ("Duplicate" in the block menu): copies each outermost listed block, with everything nested
  in it, right after the original; every stored entry is copied (newer versions' properties, marks and containers
  included), only IDs are new; returns `BlocksCreated(ids)` (ADR 0028)
- `Indent { ids }`: each block becomes the last child of the block above it (skipped if there is none, or it can't
  hold children). `Outdent { ids }`: each block moves right after its parent and the blocks that followed it become
  its last children (skipped at the top level, or for a block that can't hold children but has followers). Several
  blocks are handled one at a time in reading order; if none can move, `Unchanged`.
- `PasteBlocks { after, fragment }`
- **Tables:** `InsertTable { parent, index, rows, columns, header_row }` (at least 1 × 1; returns the table);
  `InsertTableRow { table, index }` (returns the row), `DeleteTableRow { table, index }`,
  `MoveTableRow { table, from, to }`; `InsertTableColumn { table, index }` (returns the new cells, top to bottom),
  `DeleteTableColumn { table, index }`, `MoveTableColumn { table, from, to }`. Positions count rows or columns;
  `to` is the final position. Removing the last row or column is refused (`EmptyTable`; delete the table instead).
  The header row is `SetProp { id: table, prop: TableHeaderRow(bool) }`. Cell text: `InsertText`, `DeleteText`,
  `ReplaceText`, `SetMark`, `RemoveMark` with the cell's `BlockId`. `PageDoc::table_cells(table)` reads the grid.
- **Rows and cells are guarded:** `DeleteBlocks`, `MoveBlocks`, `DuplicateBlocks`, `Indent`, `Outdent`,
  `SetBlockKind`, `SplitBlock` and `MergeIntoPrevious` naming a row or cell are refused (`TablePart`), unless it is
  inside a listed table, which deletes, moves or duplicates whole. `InsertBlock` and `SetBlockKind` can't make table
  kinds, a table can't become another kind, and nothing else can be inserted or moved into a table or row.
  Backspace at the start of the block below a table is refused with `MergeTargetNotText { target: table }`, so the
  editor selects the table.

Positions are Unicode scalar offsets as used by Loro; the editor moves the caret between grapheme cluster
boundaries so it never stops inside an emoji or a conjunct. A caret that must follow edits is an `Anchor`
(`PageDoc::anchor` / `PageDoc::resolve`): a Loro cursor plus the original position, used (clamped) when undo re-created
the block under a new container (ADR 0015). `PageDoc::import_changes` merges another copy's changes (tests only in
Phase 0), then repairs anything the merge broke (invariants 2–5 and 9), the same way on every copy and outside the undo
history (ADR 0044).

## Undo / redo

- One Loro `UndoManager` per open page doc, plus one for the workspace doc.
- Page: each step remembers the editor's selection (`SavedSelection`: caret, the other end of selected text, selected
  blocks) as it was **before** the change and right **after** it. The editor calls `set_selection` before each action
  and `record_after` after it; `PageDoc::undo` returns the "before", `redo` the "after", in `History::Done { selection }`.
  Loro's step value holds a step number; the selections live in a table in `PageDoc`, by `BlockId` + position, since
  `TreeID`s change on undo. Merged steps keep the first "before" and the last "after" (ADR 0023).
- Workspace: `Workspace::undo`/`redo`. The manager is created after loading, so saved history is not undoable;
  after each undo/redo the `PageId → TreeID` index is rebuilt (ADR 0006).
- Each command commits once, so it is one undo step. Continuous typing is grouped with the manager's merge interval (~500 ms).
- A key that needs several commands (Enter over a selection: delete, then split; leaving a code block: trim, then add
  a paragraph) wraps them in `PageDoc::as_one_step`, a Loro undo group, so it is still one undo step (ADR 0020).
- An automatic change right after typing (a Markdown shortcut converting `# ` into a heading) uses
  `PageDoc::as_separate_step`: one step that never merges with the typing before or after it, so undo gives back the
  typed characters (ADR 0025).
- `PageEditor::undo`/`redo` restore the whole selection, not just the caret.

## Invariants (checked in tests after every command)

`PageDoc::check_invariants()` checks 1–6, 8 and 9 on the raw Loro tree (unknown kinds are exempt from 2 and 3); the
property test `crates/leaf-model/tests/page_invariants.rs` runs random sequences of every page command and calls it
after each step (256 cases locally and on PRs, 10,000 in CI on `main`). Invariant 7 is checked by the page undo
property test (E02-S08). Two copies editing at once and swapping changes must end identical with every invariant
holding (`crates/leaf-model/tests/page_convergence.rs`, E02-S09); merges that break a rule are repaired (ADR 0044).

1. Every tree node has a `type` in its meta.
2. `text` exists exactly for text-bearing types.
3. Types that disallow children have no children; a `table` holds only `table_row`s and a `table_row` only
   `table_cell`s.
4. `heading.level` ∈ 1..=3.
5. `code` text has no marks.
6. No cycles (guaranteed by LoroTree, still asserted).
7. Undo of command N restores the exact document state before N, compared by `BlockId` (`TreeID`s may differ).
8. Every block has an `id` in its meta, and no two blocks in a page share one.
9. A `table_row` sits in a `table` and a `table_cell` in a `table_row`. (Commands also keep every table a full grid;
   the page property test checks that, but merges may leave rows uneven, so it is not a state invariant.)

Database documents have their own, checked by `Database::check_invariants()` after every command in
`crates/leaf-model/tests/database/` (random commands, two-peer convergence, undo and redo round-trips):

- D1. Every column and row node has a valid ID in its meta. A merge may leave one ID on two nodes; readers use the
  first in order.
- D2. The column and row trees are flat.
- D3. Every schema entry is keyed by a column ID and is a map with a `type` name.
- D4. Option IDs are valid and unique within their column.

## Persistence (`leaf-store`)

On disk:

```
MyWorkspace.leaf/
  workspace.sqlite
  assets/<blake3-hash>       # image files, written atomically before their `assets` row (ADR 0047)
  search/index.sqlite        # FTS5 search index (disposable, ADR 0019)
  leaf.lock                  # OS file lock held by the app that has it open (ADR 0009)
  view.json                  # sidebar view state on this computer: expanded pages (not a document, ADR 0031)
```

A page deleted for good has its document removed with `Store::delete` after the workspace document is saved (ADR 0036).

`Workspace::create` makes this layout, in a new path or an empty folder only. `Workspace::open` needs
`workspace.sqlite` to be present, recreates missing subfolders, and refuses a workspace another Leaf has open.

SQLite schema:

```sql
CREATE TABLE docs (
  doc_id      TEXT PRIMARY KEY,   -- 'workspace', a PageId, or 'database:' + DatabaseId
  snapshot    BLOB NOT NULL,      -- Loro snapshot
  updated_at  INTEGER NOT NULL
);

CREATE TABLE doc_updates (
  seq         INTEGER PRIMARY KEY AUTOINCREMENT,
  doc_id      TEXT NOT NULL REFERENCES docs(doc_id),
  data        BLOB NOT NULL,      -- Loro incremental update
  created_at  INTEGER NOT NULL
);
CREATE INDEX doc_updates_by_doc ON doc_updates(doc_id, seq);

CREATE TABLE assets (
  asset_id    TEXT PRIMARY KEY,   -- blake3 hex
  mime        TEXT NOT NULL,
  bytes       INTEGER NOT NULL,
  created_at  INTEGER NOT NULL
);

CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL); -- schema_version, etc.
```

- **Autosave (`Autosave`):** each document is saved at most 300 ms after its *first* unsaved change (a throttle,
  so steady typing can't postpone it), and everything pending is saved on blur/quit. The policy takes the time
  as an argument and runs no timer itself; the app does (ADR 0011).
- **Compaction (`Store::compact`, ADR 0048):** the save that takes a doc past 200 updates folds them into a fresh
  snapshot (`leaf_model::fold_saved`, bytes only, so unknown data and history are kept) and deletes them, in one
  transaction; the doc's last-saved time is unchanged. A failed compaction is logged and leaves everything as it
  was; the next save tries again. An update that can't be read, or one waiting for a lost one, stops compaction,
  so it is never folded away (ADR 0012).
- **Saving (`Store::save`):** a document's first save writes its snapshot; each later save appends one update with
  every commit since the previous save (`unsaved_changes` / `mark_saved` on `Workspace` and `PageDoc`). Nothing is
  written when nothing changed. Keys: `workspace` or the page's `PageId` (ADR 0010).
- **Loading (`Store::load`):** snapshot + updates in `seq` order (`from_saved`).
- **Recovery:** if loading fails, each update is imported on its own and damaged ones are skipped, with a warning
  in the log (`Store::recover` reports their `seq`s). Skipped rows stay on disk. A damaged snapshot is still an
  error (ADR 0012).
- **Opening:** `Store::open` sets `journal_mode=WAL`, `synchronous=NORMAL` and `foreign_keys=ON`, then migrates.
- **Migrations:** `meta.schema_version` (0 for an empty file). `leaf-store/src/migrations.rs` holds a forward-only,
  append-only list; each step runs in one transaction with its version bump. A file newer than the app is refused
  untouched (ADR 0008).

## Markdown mapping (copy and paste now; page export in Phase 1)

| Block | Markdown |
|---|---|
| paragraph | text |
| heading n | `#` × n |
| bulleted / numbered | `- ` / `1. `, children indented 2 spaces |
| todo | `- [ ] ` / `- [x] ` |
| task | `- [ ] ` / `- [x] ` for now; its date and priority get a Markdown form in E19-S04 |
| quote | `> ` |
| code | fenced block with language |
| divider | `---` |
| image | `![alt](assets/<hash>.<ext>)` + caption as italic line |
| table | GitHub-style Markdown table, first row as the header; `\|` for a pipe and `<br>` for a line break in a cell |

Pasting reads the same table back (`leaf-editor/src/clipboard/markdown.rs`, ADR 0032); plain lines become paragraphs.

## Testing requirements

- Unit test for every command and every editor rule in [PRODUCT_SPEC.md](../product/PRODUCT_SPEC.md).
- `proptest`: random sequences of commands keep all invariants.
- `proptest`: two peers apply random commands, exchange updates, and converge to identical state.
- Undo/redo round-trip property for every command.
- Persistence: save → load equals the original; simulated crash (drop without flush) loses ≤ 1 s of edits.
- `insta` snapshots for view models and markdown export.
