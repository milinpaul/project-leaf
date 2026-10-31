# Decision records

One short file per significant decision, so we can see later what we chose and why.
Records cover two kinds of decision:

- **Architecture:** crates, libraries, data model, storage, testing approach.
- **Feature:** product behaviour that is not obvious from the spec, or that changes it (for example how a key behaves in an edge case).

## How to add one

1. Copy [ADR-000-EXX-SYY-template.md](ADR-000-EXX-SYY-template.md) to `ADR-XXX-EXX-SYY-short-title.md`: `XXX` stays
   as it is, then the story the decision belongs to (its epic, e.g. `E11`, if it spans one), then a short title. Head
   it `# ADR-XXX. Title`, add it to the index below as `[ADR-XXX](ADR-XXX-EXX-SYY-short-title.md)`, and mention it
   elsewhere as `ADR-XXX-EXX-SYY` (one new record per story). When the PR merges, the `ADR numbers` workflow
   ([ADR-XXX-E00-S12](ADR-XXX-E00-S12-adr-numbers-at-merge.md)) gives it the next number, three digits so the files
   list in the order decisions were taken, and updates every mention to `ADR-NNN`.
2. Fill it in. Keep it to one screen.
3. Add a row to the index below in the same PR as the change it describes.
4. Never rewrite an accepted record. If a decision changes, write a new one and mark the old one `Superseded by ADR-NNN`.

## Index

| # | Title | Type | Status |
|---|---|---|---|
| [ADR-001](ADR-001-E00-S03-loro-document-model.md) | Loro CRDT as the document model | Architecture | Accepted |
| [ADR-002](ADR-002-E00-S03-gpui-and-gpui-component.md) | GPUI with gpui-component for the UI (with spike findings) | Architecture | Accepted |
| [ADR-003](ADR-003-E00-S03-sqlite-storage.md) | SQLite file for workspace storage | Architecture | Accepted |
| [ADR-004](ADR-004-E00-S03-markdown-is-not-storage.md) | Markdown is for import and export, not storage | Feature | Accepted |
| [ADR-005](ADR-005-E00-S03-pages-not-folders.md) | Pages all the way down, no folders | Feature | Accepted |
| [ADR-006](ADR-006-E02-S10-stable-block-ids.md) | Stable block IDs stored in block meta | Architecture | Accepted |
| [ADR-007](ADR-007-E02-S08-block-props-in-meta.md) | Block properties stored directly in block meta | Architecture | Accepted |
| [ADR-008](ADR-008-E03-S01-schema-migrations.md) | Forward-only schema migrations and SQLite settings | Architecture | Accepted |
| [ADR-009](ADR-009-E03-S02-workspace-lock.md) | One app per workspace via an OS file lock | Architecture | Accepted |
| [ADR-010](ADR-010-E03-S03-saving-unsaved-changes.md) | Documents track their own unsaved changes | Architecture | Accepted |
| [ADR-011](ADR-011-E03-S04-autosave-throttle.md) | Autosave throttles from the first change, with a clock-free policy | Architecture | Accepted |
| [ADR-012](ADR-012-E03-S06-skip-damaged-updates.md) | Skip damaged saved updates instead of failing the load | Architecture | Accepted |
| [ADR-013](ADR-013-E03-S08-app-workspace-and-autosave-wiring.md) | How the app opens its workspace and runs autosave | Feature | Accepted |
| [ADR-014](ADR-014-E04-S01-incremental-page-view.md) | Incremental page view from Loro change events | Architecture | Accepted |
| [ADR-015](ADR-015-E04-S02-caret-anchors-and-grapheme-motion.md) | Carets anchored with Loro cursors, moved by grapheme | Architecture | Accepted |
| [ADR-016](ADR-016-E04-S03-line-layout-trait.md) | Vertical caret movement through a layout trait | Architecture | Accepted |
| [ADR-017](ADR-017-E04-S04-text-input-and-composition.md) | Typing replaces the selection in one command; compositions stay out of the document | Architecture | Accepted |
| [ADR-018](ADR-018-E08-S05-app-icon.md) | App icon: one SVG, generated per-OS files, a bundle for `just run` | Feature | Accepted |
| [ADR-019](ADR-019-E11-S01-sqlite-fts5-search.md) | Full-text search with SQLite FTS5 in a disposable file, opened with Ctrl+K | Architecture | Accepted |
| [ADR-020](ADR-020-E04-S05-structural-keys.md) | Structural keys at the caret, one undo step per press | Architecture | Accepted |
| [ADR-021](ADR-021-E04-S06-selection-model.md) | Selection: text between two anchors, or whole blocks | Architecture | Accepted |
| [ADR-022](ADR-022-E04-S07-mark-toggles-and-links.md) | Mark toggles decided over the whole selection; caret toggles wait for typing | Feature | Accepted |
| [ADR-023](ADR-023-E04-S08-undo-restores-selection.md) | Undo restores the selection before an edit; redo the one right after | Architecture | Accepted |
| [ADR-024](ADR-024-E11-S03-search-index-reconciles-with-saved-versions.md) | The search index reconciles with saved versions on a background thread | Architecture | Accepted |
| [ADR-025](ADR-025-E05-S01-markdown-shortcuts.md) | Markdown shortcuts fire on typing and undo separately | Feature | Accepted |
| [ADR-026](ADR-026-E05-S02-slash-menu.md) | Slash menu: the query is page text; a small built-in fuzzy filter | Feature | Accepted |
| [ADR-027](ADR-027-E06-S01-leaf-visual-identity.md) | Leaf's visual identity: warm paper, Inter throughout, green used sparingly | Feature | Accepted |
| [ADR-028](ADR-028-E05-S03-block-actions-and-duplicate.md) | Block actions follow the selection; Duplicate is a model command that copies everything | Feature | Accepted |
| [ADR-029](ADR-029-E06-S02-app-settings-and-settings-dialog.md) | App settings in one JSON file; a Settings dialog over the page | Feature | Accepted |
| [ADR-030](ADR-030-E05-S05-internal-clipboard-fragment.md) | Internal clipboard: versioned JSON fragments; pasting blocks mid-text splits the line | Feature | Accepted |
| [ADR-031](ADR-031-E06-S03-sidebar-page-tree.md) | Sidebar page tree: logic in the editor, expanded pages in view.json | Feature | Accepted |
| [ADR-032](ADR-032-E05-S06-markdown-paste-and-copy.md) | Pasting Markdown and plain text; copying writes Markdown and plain text | Feature | Accepted |
| [ADR-033](ADR-033-E06-S04-quick-switcher.md) | Quick switcher: a snapshot at open, a small fuzzy matcher, Enter through the dialog | Feature | Accepted |
| [ADR-034](ADR-034-E06-S05-page-icon-picker.md) | Page icon picker: the `emojis` crate, Unicode 13 and older, in a popover | Feature | Accepted |
| [ADR-035](ADR-035-E07-S01-virtual-page-list.md) | The page is a virtual list; rows are diffed to keep it in step | Architecture | Accepted |
| [ADR-036](ADR-036-E06-S06-trash-view-and-permanent-delete.md) | Trash view: a dialog over the sidebar, and deleting from disk after the page tree is saved | Feature | Accepted |
| [ADR-037](ADR-037-E06-S08-recent-workspaces.md) | Recent workspaces live in the app settings; a missing one is flagged, not dropped | Feature | Accepted |
| [ADR-038](ADR-038-E07-S02-rich-text-view.md) | Block text drawn with StyledText; line layout read back from the painted text | Architecture | Accepted |
| [ADR-039](ADR-039-E06-S09-welcome-screen.md) | Welcome screen: one window that switches between the welcome screen and a workspace | Feature | Accepted |
| [ADR-040](ADR-040-E07-S03-page-takes-the-keyboard.md) | The page takes the keyboard: actions for keys, the platform text input for typing and input methods | Architecture | Accepted |
| [ADR-041](ADR-041-E07-S04-block-handle-menu.md) | The block menu is editor state; Move up/down move among siblings | Feature | Accepted |
| [ADR-042](ADR-042-E11-S05-search-bar-model.md) | Search bar model: titles at once, full text after a pause, stale results dropped by id | Feature | Accepted |
| [ADR-043](ADR-043-E07-S06-slash-menu-popover.md) | The slash popover keeps the page's keyboard and opens above the line near the bottom edge | Feature | Accepted |
| [ADR-044](ADR-044-E02-S09-repair-after-merge.md) | Pages are repaired after a merge, the same way on every copy | Architecture | Accepted |
| [ADR-045](ADR-045-E06-S11-page-covers.md) | Page covers: a stored name, drawn from bundled SVG files | Feature | Accepted |
| [ADR-046](ADR-046-E11-S06-search-bar-ui.md) | Search bar UI: one popup for Cmd/Ctrl+K and P, searched off the UI thread, results opened in place | Feature | Accepted |
| [ADR-047](ADR-047-E03-S07-asset-store.md) | Asset store: images named by hash, type read from content, file before row | Architecture | Accepted |
| [ADR-048](ADR-048-E03-S05-compaction.md) | Compaction: the save past 200 updates folds them into a fresh snapshot, in one transaction | Architecture | Accepted |
| [ADR-049](ADR-049-E05-S04-dragging-blocks.md) | Dragging blocks: drops land in a gap between rows, at a depth picked by the pointer's x | Feature | Accepted |
| [ADR-050](ADR-050-E07-S05-block-drag-visuals.md) | Block drag visuals: GPUI's drag, a line at the drop's depth, faded rows, scrolling at the edges | Feature | Accepted |
| [ADR-051](ADR-051-E05-S07-inserting-images.md) | Inserting images: the editor places the block, the UI fetches the picture | Feature | Accepted |
| [ADR-052](ADR-052-E07-S07-link-popover.md) | Link popover: an address box under the selection; only web and mail addresses are opened | Feature | Accepted |
| [ADR-053](ADR-053-E07-S08-drawing-images.md) | Drawing images: natural size shrunk to fit, resized by a handle, saved once | Feature | Accepted |
| [ADR-054](ADR-054-E07-S09-code-blocks.md) | Code blocks: a language picker storing fence names, Tab indents lines with four spaces | Feature | Accepted |
| [ADR-055](ADR-055-E08-S01-test-fixtures-crate.md) | Large test fixtures: a small crate, built through real commands, fixed by a seed | Architecture | Accepted |
| [ADR-056](ADR-056-E08-S02-performance-benchmarks.md) | Performance benchmarks: divan benches in their own crate, results kept by CI on `main` | Architecture | Accepted |
| [ADR-057](ADR-057-E07-S11-no-turn-into-toolbar.md) | No "Turn into" toolbar: block types change from the slash menu, the block handle and Markdown | Feature | Accepted |
| [ADR-058](ADR-058-E11-S07-search-performance.md) | Search performance: rank only the newest matches, pick blocks before excerpts | Architecture | Accepted |
| [ADR-059](ADR-059-E18-S01-page-edited-time.md) | Page edited time: a field in the workspace doc, stamped by a command undo skips | Feature | Accepted |
| [ADR-060](ADR-060-E18-S02-favourite-pages.md) | Favourite pages: a map from page to position in the workspace doc | Feature | Accepted |
| [ADR-061](ADR-061-E16-S01-simple-tables.md) | Simple tables: rows and cells as blocks inside a table block | Architecture | Accepted |
| [ADR-062](ADR-062-E17-S01-smooth-wheel-and-edge-scrolling.md) | Mouse-wheel steps glide; dragging a selection past the page's edge scrolls | Feature | Accepted |
| [ADR-063](ADR-063-E16-S02-editing-tables.md) | Editing tables: cells as text blocks, a rectangle of cells, tables acted on whole | Feature | Accepted |
| [ADR-064](ADR-064-E06-S16-cover-gallery.md) | Cover gallery: sections of thumbnails, and colour and gradient covers as SVG files | Feature | Accepted |
| [ADR-065](ADR-065-E18-S03-page-top-bar.md) | Page top bar: breadcrumb, edited time, star and page menu above every page | Feature | Accepted |
| [ADR-066](ADR-066-E09-S01-database-data-model.md) | Databases: one Loro document per database, rows as pages | Architecture | Accepted |
| [ADR-067](ADR-067-E07-S13-formatting-bar.md) | Formatting bar: a floating bar over selected text, drawn in the page's own overlay | Feature | Accepted |
| [ADR-068](ADR-068-E19-S01-action-item-block.md) | Action items: a `task` block kind with its fields as props | Feature | Accepted |
| [ADR-069](ADR-069-E09-S05-database-page-table.md) | Database page: gpui-component's data table, editing rules in leaf-editor | Feature | Accepted |
| [ADR-070](ADR-070-E21-S01-callout-block.md) | Callout block: one text with lines, and a tone stored by name | Feature | Accepted |
| [ADR-071](ADR-071-E10-S01-page-links-and-backlinks.md) | Page links: one placeholder character with a `page` mark; backlinks in the search index | Architecture | Accepted |
| [ADR-072](ADR-072-E09-S06-database-column-menu.md) | Database columns: header menus, and each database command its own undo step | Feature | Accepted |
| [ADR-073](ADR-073-E08-S10-ci-and-supply-chain-policy.md) | CI and supply chain: read-only CI, pinned actions, deny on new duplicates and git sources | Architecture | Accepted |
| [ADR-074](ADR-074-E08-S13-workspace-files-stay-inside-the-folder.md) | Workspace files stay inside the folder: links refused on open, Windows device names refused | Architecture | Accepted |
| [ADR-075](ADR-075-E22-S01-database-views-and-board.md) | Database views: saved views as tabs, and a board layout | Architecture | Accepted |
| [ADR-XXX](ADR-XXX-E00-S12-adr-numbers-at-merge.md) | Decision records are numbered when they merge, by a workflow | Architecture | Accepted |
| [ADR-XXX](ADR-XXX-E08-S08-bundled-code-font.md) | A bundled code font (JetBrains Mono NL), set before the UI library starts, for a fast cold start | Architecture | Accepted |
