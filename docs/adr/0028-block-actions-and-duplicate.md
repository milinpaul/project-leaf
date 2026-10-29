# 0028. Block actions follow the selection; Duplicate is a model command that copies everything

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E05-S03

## Context

The block handle (⋮⋮) menu offers Turn into, Duplicate and Delete (`PRODUCT_SPEC.md`). They must work on one block or
a multi-block selection, Duplicate must deep-copy a subtree with new IDs, and each action must be one undo step.
Turn into and Delete already have commands (`SetBlockKind`, `DeleteBlocks`). Nothing copies blocks.

## Decision

- **Which blocks:** an action opened on a block that is part of the selection applies to the whole selection:
  every block a text selection touches, or the selected blocks, including blocks nested in them. Otherwise it
  applies to that block alone (`PageEditor::action_targets`), as in other block editors.
- **`PageEditor::block_action(block, BlockAction)`** runs it as one undo step:
  - **Turn into:** `SetBlockKind` on each block, in one undo group. Blocks that can't become the kind are skipped:
    text into a divider would lose words, and unknown kinds are never rewritten. The selection stays.
  - **Duplicate:** one `DuplicateBlocks` command; the copies end up selected.
  - **Delete:** the existing `delete_blocks`. The caret goes above, and a page is never left empty.
- **New model command `DuplicateBlocks { ids }`:** copies each outermost listed block, with everything nested in it,
  right after the original, and returns the copies (`PageOutcome::BlocksCreated`).
  - The copy is made from the stored record, entry by entry, not from what this version understands. Kind,
    properties, formatting (marks this version doesn't know included), and maps, lists and texts from newer versions
    are all copied. Only the IDs are new (architecture rule 5).
  - The block text keeps its usual container (ADR 0001). It is one commit, so one undo step.

## Alternatives considered

- **Duplicate in the editor with `InsertBlock` + `SetMark`:** works for what this version knows, but drops properties
  and marks from newer versions, breaking rule 5. It also needs many commands per block.
- **Act only on the menu's block:** simpler, but selecting several blocks and choosing an action would surprise.

## Consequences

- The menu itself (E07-S04) calls `block_action` with the hovered block; the drag handle is E05-S04 / E07-S05.
- Loro container kinds a block record has never held (trees, counters) would be copied as plain values. None exist
  today.
- Plain English: from a block's ⋮⋮ menu you can change its type, copy it, or delete it, along with everything indented
  under it. If several blocks are selected, the action applies to all of them. One Undo reverses it.
