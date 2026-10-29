# 0039. The page takes the keyboard: actions for keys, the platform text input for typing and input methods

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-29
- **Story:** E07-S03

## Context

After E07-S02 every block was drawn by the rich text view, except the one being typed in, which still used a text
input. E07-S03 asks for:
- keys, input methods and the mouse to reach `leaf-editor` directly;
- input methods to work for at least Hindi, Japanese and the emoji picker;
- clicks and drags to select;
- keystroke-to-paint under 16 ms on a large page.

The editor already keeps an input method's draft outside the document until it is committed (ADR 0017).

## Decision

- **The page has its own focus and key context (`LeafPage`).**
  - Keys are GPUI actions bound in that context (`shell/page_keys.rs`); each calls one `PageEditor` method, then the
    UI scrolls the caret into view and redraws.
  - Shortcuts use Cmd on macOS and Ctrl elsewhere, with each platform's word and line keys (Option/Cmd+arrows on
    macOS, Ctrl+arrows elsewhere).
  - The bindings cover Enter, Cmd/Ctrl+Enter, Backspace, Delete, Tab, Shift+Tab, the arrows with and without Shift,
    Home/End, Cmd/Ctrl+A, B, I, E, Shift+S, Z, Shift+Z, C, X and V, and Escape.
  - Up/Down move by painted lines (`PaintedLayout`, ADR 0038); Up on the first line goes to the title.
- **Typing and input methods arrive through GPUI's `EntityInputHandler`, implemented by the shell (`shell/ime.rs`).**
  - It is registered for the page while the page is painted.
  - The input method's "document" is the caret's block, with the draft spliced in where it will go; offsets are
    UTF-16, as platforms count them.
  - Composing calls `set_preedit`, so nothing is written. The view draws the draft underlined in place (the
    selection it covers is hidden meanwhile) and reports its position for the candidate window.
  - Committing calls `commit_preedit`, one undo step.
  - A composition over a given range (reconversion) selects that range first; an accent-menu replacement selects
    its range and types.
  - A composition abandoned without a commit keeps what was shown.
- **Mouse:**
  - A click places the caret; Shift extends the selection.
  - A drag selects from where it started, across blocks.
  - A double click selects the word, using Unicode word boundaries (new `PageEditor::select_word_at`); a triple
    click selects the block's text (`select_block_text`).
- **Clipboard:** copy writes Markdown as text for other apps, with Leaf's fragment JSON as metadata. Paste prefers the
  fragment and otherwise reads the text as Markdown (ADR 0030, ADR 0032).
- **Performance:** the editor's block order now comes from its incrementally kept view instead of a full rebuild.
  That was the one O(page) cost per keystroke. On a 10,000-block page, keystroke to painted frame is 4.9 ms median
  and 7.9 ms at worst in a release build, and about 11 ms median in a debug build. Measured with `just screenshot …
  bench`.
- **Verified in the real app with real key events (macOS, unlocked):** typing with spaces, Enter, selecting a word
  with Shift+Option+Left, and Cmd+B. The off-screen screenshot tool can't type spaces: GPUI's visual test context
  drops keys after a space. The tool documents this; the real app is unaffected.

## Alternatives considered

- **Keep a text input per block and route around it:** the inputs can't draw formatting, and every structural key
  would need intercepting before the input saw it.
- **Write the draft into the document while composing:** simpler for the input method, but it would create undo
  steps, saves and sync traffic for keystrokes the user hasn't committed (ADR 0017).
- **One input handler per block:** the platform has one text input per window; the caret's block is the only one
  the input method needs to see.

## Consequences

- The link popover (Cmd/Ctrl+K, E07-S07) and the slash menu popover (E07-S06) attach to this keyboard handling.
- Plain English: you now type straight into the page, with formatting showing as you type. Shortcuts, Japanese and
  Hindi keyboards, the emoji picker, clicking, dragging and double/triple clicking all work. Typing stays instant even
  on a page with ten thousand blocks.
