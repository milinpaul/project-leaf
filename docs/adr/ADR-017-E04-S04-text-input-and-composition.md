# ADR-017. Typing replaces the selection in one command; compositions stay out of the document

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E04-S04

## Context

Typing over a selection must be one undo step. Many people type through an input method: Japanese, Chinese and
Korean keyboards, Indic keyboards, and dead keys for accents. An input method shows a draft ("preedit") that changes
with each key until the user commits it. Writing each draft into the Loro document would:
- create undo steps for text the user never meant to keep;
- trigger saves;
- later, send half-typed syllables to other devices.

Editors also have a history of losing the selection when an input method sends an empty draft.

## Decision

- **A new `PageCommand::ReplaceText { id, range, text }`** replaces a range in one commit, so it is one undo step.
  - The new text takes the formatting of the first character it replaces: typing over a bold word stays bold, and
    over link text keeps the link, as in word processors.
  - With an empty range it is `InsertText`.
- **`PageEditor::insert_text` types:** it replaces the selection, or inserts at the caret, and puts the caret after
  the new text. The caret before typing is recorded for undo.
- **The draft lives only in the editor** (`set_preedit`, `commit_preedit`, `cancel_preedit`):
  - `preedit()` tells the UI what to draw and where: over the selection it will replace, or at the caret.
  - Nothing reaches the document until the commit, which goes through `insert_text`.
  - An empty draft ends the composition and never touches the selection.
- **A minimal selection inside one block** (`select`, `selection`) is kept as a second anchor next to the caret, so
  it follows edits. Any caret move drops it. Selections across blocks are E04-S06.

## Alternatives considered

- **Delete then insert:** two commands, two undo steps. It also deletes the selection as soon as a composition
  starts.
- **Write the draft into the document and undo it on commit:** creates and discards history, saves, and later
  broadcasts drafts. An abandoned composition could leave text behind.
- **Typed text inherits marks only by the expand rules:** typing over a bold word right after plain text would come
  out plain, which surprises people.

## Consequences

- The UI (E07-S02, E07-S03) draws the draft underlined in place of the covered text, and forwards the platform's
  input-method callbacks to these four methods.
- One more page command, covered by the invariants and undo property tests.
- Plain English: when you type over selected text it's replaced in one go, and one undo brings it back. While a
  Japanese or Hindi keyboard is still composing a character, nothing is saved or added to undo until you pick the
  final character.
