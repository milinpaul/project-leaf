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
| `image` | `asset`: AssetId, `width`: f32?, `alt`: String | caption | no |

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
    Image { asset: AssetId, width: Option<f32>, alt: String },
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
- `MovePage { page, new_parent, index }`
- `TrashPage`, `RestorePage`, `DeletePagePermanently`

**Block commands (page doc)**
- `InsertBlock { parent: Option<BlockId>, index, kind, text }`
- `DeleteBlocks { ids }` — deletes subtrees
- `MoveBlocks { ids, new_parent, index }`
- `SetBlockKind { id, kind }` — keeps text when the target type has text
- `SetProp { id, key, value }`
- `InsertText { id, pos, text }`, `DeleteText { id, range }`
- `SetMark { id, range, mark }`, `RemoveMark { id, range, mark }`
- `SplitBlock { id, pos }` — Enter
- `MergeIntoPrevious { id }` — Backspace at start
- `Indent { ids }`, `Outdent { ids }`
- `PasteBlocks { after, fragment }`

Positions are Unicode scalar offsets as used by Loro; the editor converts to and from grapheme clusters for cursor movement.

## Undo / redo

- One Loro `UndoManager` per open page doc, plus one for the workspace doc.
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
