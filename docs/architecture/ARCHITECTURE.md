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
  leaf-core: IDs and error types shared by all.   Phase 1: leaf-export (markdown), leaf-search (search).
```

## The crates

| Crate | Job | Must not |
|---|---|---|
| `leaf-core` | Shared IDs, error types, small helpers. | Depend on any other Leaf crate. |
| `leaf-model` | The documents. The only crate that changes them, and only through `Command`s. Undo lives here. | Know about the screen, files or SQLite. |
| `leaf-editor` | All editing behaviour: what Enter, Backspace, Tab, paste, the slash menu do. Turns input into commands and documents into view models. | Draw anything, or change documents except through commands. |
| `leaf-store` | Saves and loads documents in a SQLite file; autosave and crash recovery. | Know about the screen. |
| `leaf-export` | Markdown import/export (Phase 1). | Touch files or the screen. |
| `leaf-search` | Full-text search index (Phase 1). | Hold data that exists nowhere else. |
| `leaf-ui` | Draws view models with GPUI and passes input back. Wraps every gpui-component widget. | Contain editing logic. |
| `leaf-app` | The program itself: starts up, opens windows, connects the crates. | Contain logic that a testable crate could hold. |

The first six are **core crates**. They never depend on GPUI or `leaf-ui`; a test
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
block's kind and properties (E02-S04). The window can edit a page's blocks with an **interim block editor**
(E07-S10): one plain text input per block, drawn from `PageEditor`'s rows in `leaf-editor`, with Enter, Backspace,
Tab and Shift+Tab caught before the input sees them. The rich text element (E07-S02) replaces the inputs later;
`PageEditor`'s rules stay. Enter splits a block at the cursor and Backspace at the start merges it into the block
above (E02-S05), keeping formatting on both sides. Pages have their own undo and redo, carrying the caret with each step (E02-S08);
the editor wires Cmd+Z to it in E04-S08. The editor's caret (E04-S02) moves by
whole characters as a reader sees them (an emoji, a Hindi syllable), jumps by word, and goes to line start and end; it
is anchored to the text, so edits before it, undo and other devices' changes carry it along (ADR 0015). Up and Down
(E04-S03) ask the UI where lines wrap through the `LineLayout` trait and keep the caret's column (ADR 0016). Typing (E04-S04) replaces the selection with one
`ReplaceText` command; an input method's draft is kept in the editor and only the committed text reaches the page
(ADR 0017). The window's sidebar lists real pages (`WorkspaceEditor` in `leaf-editor`): "New page" and typing a title go
through those commands (E06-S07).

**Startup and saving (E03-S08).** `leaf-app` opens the workspace folder given as its first argument, or
`~/Documents/Leaf/My Workspace.leaf`, creating it if missing (`leaf_ui::workspace_root`). It loads the workspace
document; each page's blocks load from disk the first time the page is opened. A 100 ms timer in the window calls
`Autosave::tick` from `leaf-store`, which polls every open document for unsaved changes and saves those that are
due. Window blur, quit (Cmd+Q / Ctrl+Q, or the app menu) and window close save everything at once. If the folder
can't be opened (another Leaf has it open, it is damaged), the window shows why instead of the editor.

**App icon and identity (E08-S05).** `crates/leaf-app/resources/` holds the icon: `icon.svg` is the master, and
`just icons` generates the PNG files, `Leaf.icns` and `leaf.ico` from it (committed, so builds need no image tools). On
macOS `just run` launches Leaf from a `Leaf.app` bundle so the Dock shows its icon; on Windows `build.rs` builds the
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
5. **The change is saved.** The document hands `leaf-store` one small update with everything changed since the last
   save (`unsaved_changes`), which is appended to the SQLite file; then the document is marked saved. `Autosave`
   decides when: at most 300 ms after the first unsaved change, and at once on blur or quit, so a crash loses at
   most about a second of typing.

Undo runs the same path backwards: the model reverses the last step and the editor redraws.

## Where things live

```
crates/            the eight crates above
docs/product/      what we build and why (PRODUCT_SPEC.md)
docs/architecture/ how it is built (this file, TECH_STACK.md, BLOCK_MODEL.md)
docs/adr/          decision records
backlog/           epics and stories (the work)
```

Keep this file current: update it in the same PR as any change to crates, data flow or saving.
