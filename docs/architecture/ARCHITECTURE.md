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
create, rename and set an icon (E01-S03). The window's sidebar now lists real pages from an in-memory workspace
(`WorkspaceEditor` in `leaf-editor`): "New page" and typing a title go through those commands (E06-S07).

1. **You press a key.** GPUI delivers it to `leaf-ui`, which forwards it untouched to `leaf-editor`.
2. **The editor decides what it means.** For example, Enter in the middle of a paragraph means "split this
   block here". It produces a `Command` such as `SplitBlock { id, pos }`.
3. **The model applies the command.** `leaf-model` changes the Loro document in one step (one commit), checks
   the document's rules still hold, and records one undo step.
4. **The screen updates.** The editor builds a fresh view model (blocks, text, cursor) and `leaf-ui` draws it.
   The goal is under 16 ms from key press to pixels.
5. **The change is saved.** Loro hands `leaf-store` a small update describing the change. Within 300 ms it is
   appended to the SQLite file, so a crash loses at most about a second of typing.

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
