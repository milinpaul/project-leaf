# ADR-031. Sidebar page tree: logic in the editor, expanded pages in view.json

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E06-S03

## Context

The sidebar becomes a real page tree: nested pages that expand and collapse, a right-click menu (add a page
inside, rename, move to trash) and drag-and-drop to move and reorder. It must stay smooth at 500 pages, 6 levels deep.
Three things needed deciding: where the drop logic lives, how a place on screen becomes a model position (positions
count pages in the trash, the sidebar hides them), and where "which pages are expanded" is remembered.

## Decision

- **The tree is editor logic.** `WorkspaceEditor` (in `leaf-editor`) builds the rows (depth-first, children only under
  expanded pages), keeps the expanded set, and turns every action into `WorkspaceCommand`s. A drop is a `DropPlace`:
  `Before`/`After`/`Inside` a row, or `End` (the empty space under the tree). The pointer's height in the row picks it:
  top quarter above, bottom quarter below, middle inside. Below an expanded page with children means its first child,
  because that is where the line is drawn. `can_drop` refuses a page onto or inside itself.
- **Places become indexes through the full sibling list.** The model gains two read queries, `Workspace::parent` and
  `Workspace::child_slots` (children with trashed ones included), and the editor counts positions in that list. This
  also fixed "New page" landing among trashed pages instead of at the end.
- **Expanded pages live in `view.json` in the workspace folder**, written as soon as they change. Expanding is not an
  edit: it isn't undone, doesn't mark the workspace unsaved and (later) won't sync to other devices. The file follows
  the settings file's rules (missing or damaged means defaults, unknown fields kept, atomic writes), now shared in
  `leaf-store`'s `json_file`.
- **The open page lives there too** (amended in E06-S19): `view.json` also records which page is open, written when
  another page opens, and the workspace opens on it again next time (`WorkspaceEditor::restore_open_page` skips a
  page that is gone or in the trash). Like expanded pages it is per computer, never an edit and never synced.
- **The UI draws only visible rows** with GPUI's `uniform_list` (fixed 30 px rows), replacing gpui-component's
  `Sidebar` list. Inline rename is a text field in the row: Enter or clicking away keeps the title, Escape doesn't.

## Alternatives considered

- **Store `expanded` in the workspace document:** every click would become an undoable, syncable edit, and another
  device's sidebar would open and close under the owner. No.
- **Keep it in the app settings file:** it would mix per-workspace state into an app-wide file keyed by folder path,
  which breaks when a folder is moved or renamed.
- **Let the model take "visible" indexes:** every command would need to know what the UI hides; the model's index rule
  is simpler and already documented, so the translation sits in the editor.
- **Draw every row:** fine at 50 pages, wasteful at 500; `uniform_list` costs nothing extra.

## Consequences

- All tree behaviour is tested without a window (including a property test that every drop lands where the line was
  drawn), and the UI tests drive real drags with simulated mouse moves.
- Building the 500-page row list takes about 1.6 ms in a debug build, so redrawing on every keystroke is fine.
- The trash view (E06-S06) adds restore; it uses `Workspace::trash` and needs no tree changes.
- Plain English: the sidebar now shows pages inside pages. You can open and close them (Leaf remembers), right-click to
  add, rename or bin a page, and drag a page to move it; a line or a ring shows where it will land.
