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
