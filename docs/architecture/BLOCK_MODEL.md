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
          "created_at": i64     # unix ms
          "trashed_at": i64?    # set = in trash (subtree hidden)
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
          "props": LoroMap     # type-specific properties
          "text":  LoroText    # present for text-bearing types; create with ensure_mergeable_text
```

Root nodes of the `"blocks"` tree are the page's top-level blocks, in order.

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

Configure once per doc with `config_text_style` before any edit. `code` blocks never carry marks.

## Commands

Defined as an enum in `leaf-model`. The editor (`leaf-editor`) translates key presses and mouse actions into commands; nothing else writes to Loro.

**Page commands (workspace doc)**: `WorkspaceCommand`, applied with `Workspace::apply`
- `CreatePage { parent: Option<PageId>, index, title, created_at }`: the model mints and returns the `PageId`;
  the caller supplies `created_at` (Unix ms) because the model never reads the clock (it must run in WebAssembly)
- `RenamePage { page, title }`, `SetPageIcon { page, icon: Option<String> }` (`None` clears the icon)
- `MovePage { page, new_parent, index }`: `index` is the page's final position among its new siblings, counted
  without the page itself; moving a page into itself or one of its descendants is rejected
- `TrashPage { page, at }`: sets `trashed_at`; the page stays in place, hidden with everything inside it from
  `root_pages`/`children` (listed by `trash()` instead). No creating or moving into the trash, and a page in the
  trash can't be moved until restored.
- `RestorePage { page }`: clears `trashed_at`; if the page it lived in is itself in the trash, the page moves to the
  end of the top level so it doesn't stay hidden
- `DeletePagePermanently { page }`: only for pages in the trash; deletes the subtree and returns every deleted
  `PageId` (`Outcome::PagesDeleted`) so storage can drop their page docs
- A command that would change nothing (same title, same icon, same position) writes nothing and returns
  `Outcome::Unchanged`, so it leaves no empty undo step
- Positions (`index`) count every child, including pages in the trash, while `root_pages`/`children` hide them

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
- `SetBlockKind { id, kind }` — keeps text when the target type has text
- `SetProp { id, key, value }`
- `InsertText { id, pos, text }`, `DeleteText { id, range }`: positions are Unicode scalar offsets, checked against the
  text length; blocks without text (dividers, unknown kinds) are refused
- `SetMark { id, range, mark: Mark }`, `RemoveMark { id, range, kind: MarkKind }`: `Mark` is bold, italic, strike,
  code or `Link(href)` (href must not be empty); code blocks take no marks; expand behaviour follows the table above
- `SplitBlock { id, pos }` — Enter
- `MergeIntoPrevious { id }` — Backspace at start
- `Indent { ids }`: each block becomes the last child of the block above it (skipped if there is none, or it can't
  hold children). `Outdent { ids }`: each block moves right after its parent and the blocks that followed it become
  its last children (skipped at the top level, or for a block that can't hold children but has followers). Several
  blocks are handled one at a time in reading order; if none can move, `Unchanged`.
- `PasteBlocks { after, fragment }`

Positions are Unicode scalar offsets as used by Loro; the editor converts to and from grapheme clusters for cursor movement.

## Undo / redo

- One Loro `UndoManager` per open page doc, plus one for the workspace doc.
- Workspace: `Workspace::undo`/`redo`. The manager is created after loading, so saved history is not undoable;
  after each undo/redo the `PageId → TreeID` index is rebuilt (ADR 0006).
- Each command commits once, so it is one undo step. Continuous typing is grouped with the manager's merge interval (~500 ms).
- Cursor and selection are stored with each undo step and restored on undo/redo.

## Invariants (checked in tests after every command)

1. Every tree node has a `type` in its meta.
2. `text` exists exactly for text-bearing types.
3. Types that disallow children have no children.
4. `heading.level` ∈ 1..=3.
5. `code` text has no marks.
6. No cycles (guaranteed by LoroTree, still asserted).
7. Undo of command N restores the exact document state before N, compared by `BlockId` (`TreeID`s may differ).
8. Every block has an `id` in its meta, and no two blocks in a page share one.

## Persistence (`leaf-store`)

On disk:

```
MyWorkspace.leaf/
  workspace.sqlite
  assets/<blake3-hash>       # image files
  search/                    # tantivy index (Phase 1, disposable)
```

SQLite schema:

```sql
CREATE TABLE docs (
  doc_id      TEXT PRIMARY KEY,   -- 'workspace' or PageId
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

- **Autosave:** append Loro updates to `doc_updates` at most every 300 ms and on blur/quit (WAL mode).
- **Compaction:** when a doc has > 200 updates, write a fresh snapshot and delete them, in one transaction.
- **Loading:** snapshot + updates in `seq` order.
- **Migrations:** `meta.schema_version`, forward-only migrations, tested.

## Markdown mapping (Phase 1 export)

| Block | Markdown |
|---|---|
| paragraph | text |
| heading n | `#` × n |
| bulleted / numbered | `- ` / `1. `, children indented 2 spaces |
| todo | `- [ ] ` / `- [x] ` |
| quote | `> ` |
| code | fenced block with language |
| divider | `---` |
| image | `![alt](assets/<hash>.<ext>)` + caption as italic line |

## Testing requirements

- Unit test for every command and every editor rule in [PRODUCT_SPEC.md](../product/PRODUCT_SPEC.md).
- `proptest`: random sequences of commands keep all invariants.
- `proptest`: two peers apply random commands, exchange updates, and converge to identical state.
- Undo/redo round-trip property for every command.
- Persistence: save → load equals the original; simulated crash (drop without flush) loses ≤ 1 s of edits.
- `insta` snapshots for view models and markdown export.
