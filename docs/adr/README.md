# Decision records

One short file per significant decision, so we can see later what we chose and why.
Records cover two kinds of decision:

- **Architecture:** crates, libraries, data model, storage, testing approach.
- **Feature:** product behaviour that is not obvious from the spec, or that changes it (for example how a key behaves in an edge case).

## How to add one

1. Copy [0000-template.md](0000-template.md) to `NNNN-short-title.md`, using the next free number.
2. Fill it in. Keep it to one screen.
3. Add a row to the index below in the same PR as the change it describes.
4. Never rewrite an accepted record. If a decision changes, write a new one and mark the old one `Superseded by NNNN`.

## Index

| # | Title | Type | Status |
|---|---|---|---|
| [0001](0001-loro-document-model.md) | Loro CRDT as the document model | Architecture | Accepted |
| [0002](0002-gpui-and-gpui-component.md) | GPUI with gpui-component for the UI (with spike findings) | Architecture | Accepted |
| [0003](0003-sqlite-storage.md) | SQLite file for workspace storage | Architecture | Accepted |
| [0004](0004-markdown-is-not-storage.md) | Markdown is for import and export, not storage | Feature | Accepted |
| [0005](0005-pages-not-folders.md) | Pages all the way down, no folders | Feature | Accepted |
| [0006](0006-stable-block-ids.md) | Stable block IDs stored in block meta | Architecture | Accepted |
| [0007](0007-block-props-in-meta.md) | Block properties stored directly in block meta | Architecture | Accepted |
| [0008](0008-schema-migrations.md) | Forward-only schema migrations and SQLite settings | Architecture | Accepted |
| [0009](0009-workspace-lock.md) | One app per workspace via an OS file lock | Architecture | Accepted |
| [0010](0010-saving-unsaved-changes.md) | Documents track their own unsaved changes | Architecture | Accepted |
| [0011](0011-autosave-throttle.md) | Autosave throttles from the first change, with a clock-free policy | Architecture | Accepted |
| [0012](0012-skip-damaged-updates.md) | Skip damaged saved updates instead of failing the load | Architecture | Accepted |
| [0013](0013-app-workspace-and-autosave-wiring.md) | How the app opens its workspace and runs autosave | Feature | Accepted |
| [0014](0014-incremental-page-view.md) | Incremental page view from Loro change events | Architecture | Accepted |
| [0015](0015-caret-anchors-and-grapheme-motion.md) | Carets anchored with Loro cursors, moved by grapheme | Architecture | Accepted |
| [0016](0016-line-layout-trait.md) | Vertical caret movement through a layout trait | Architecture | Accepted |
| [0017](0017-text-input-and-composition.md) | Typing replaces the selection in one command; compositions stay out of the document | Architecture | Accepted |
| [0018](0018-app-icon.md) | App icon: one SVG, generated per-OS files, a bundle for `just run` | Feature | Accepted |
| [0019](0019-sqlite-fts5-search.md) | Full-text search with SQLite FTS5 in a disposable file, opened with Ctrl+K | Architecture | Accepted |
| [0020](0020-structural-keys.md) | Structural keys at the caret, one undo step per press | Architecture | Accepted |
| [0021](0021-selection-model.md) | Selection: text between two anchors, or whole blocks | Architecture | Accepted |
| [0022](0022-mark-toggles-and-links.md) | Mark toggles decided over the whole selection; caret toggles wait for typing | Feature | Accepted |
| [0023](0023-undo-restores-selection.md) | Undo restores the selection before an edit; redo the one right after | Architecture | Accepted |
| [0024](0024-search-index-reconciles-with-saved-versions.md) | The search index reconciles with saved versions on a background thread | Architecture | Accepted |
| [0025](0025-markdown-shortcuts.md) | Markdown shortcuts fire on typing and undo separately | Feature | Accepted |
| [0026](0026-slash-menu.md) | Slash menu: the query is page text; a small built-in fuzzy filter | Feature | Accepted |
| [0027](0027-leaf-visual-identity.md) | Leaf's visual identity: warm paper, Inter throughout, green used sparingly | Feature | Accepted |
| [0028](0028-block-actions-and-duplicate.md) | Block actions follow the selection; Duplicate is a model command that copies everything | Feature | Accepted |
