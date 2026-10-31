# ADR-013. How the app opens its workspace and runs autosave

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-28
- **Story:** E03-S08

## Context

Storage and the autosave rule (ADR-010, ADR-011) existed but the app still edited an in-memory workspace. There is
no settings store yet to remember the last workspace, no workspace picker, and no error notifications. The
autosave rule must hear about every change, and missing one would lose data.

## Decision

- **Which workspace:** `leaf <path>` opens or creates the workspace at `<path>`. With no argument the app uses
  `~/Documents/Leaf/My Workspace.leaf`. That is a visible folder the owner can find and back up, so for now it
  doubles as "the last workspace".
- **Polling, not reporting:** a 100 ms timer calls `Autosave::tick` with every open document. `tick` asks each one
  whether it has unsaved changes (a cheap version comparison), so no editing code has to remember to report
  changes.
- **Save everything at once** on window blur, on quit (a new Quit action: Cmd+Q on macOS, Ctrl+Q elsewhere, and
  the app menu) and when the window closes.
- **Lazy pages:** each page's blocks load from disk the first time the page is opened.
- **Saving needs only read access:** `mark_saved` takes `&self`, so saving never needs write access to the
  documents the editor is using.
- **Can't open:** a window shows the reason ("already open in another Leaf window", damaged file, ...) instead of
  the editor.

## Alternatives considered

- **The editor reports every change:** precise, but every new editing path must remember to report, and a
  forgotten one silently loses data.
- **Store the workspace in a hidden app-data folder:** that's typical for apps, but harder for the owner to find
  and back up. It can be revisited with a workspace picker.
- **A workspace picker at first launch:** better long-term, but it needs a file dialog and settings. It is left
  for later.

## Consequences

- Edits reach disk within about 400 ms (300 ms delay plus up to one 100 ms tick), inside the 1 s promise.
- **Before this change, Leaf had no Quit command;** ⌘Q did nothing. Now it quits and saves.
- **To change later:** remembering the last workspace and choosing another one needs a small settings file and
  a picker.
- Plain English: Leaf now keeps your pages in a folder in Documents and saves as you type. Quitting, switching
  away or even a crash keeps your work. If the folder is already open in another Leaf window, you get a clear
  message.
