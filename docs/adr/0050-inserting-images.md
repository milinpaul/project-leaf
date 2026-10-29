# 0050. Inserting images: the editor places the block, the UI fetches the picture

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E05-S07

## Context

Images come in two ways: "Image" in the slash menu, which needs a file picked by the owner, and paste from another
app (a screenshot, "Copy image" in a browser, an image file copied in the file manager). The picture must be in the
workspace's asset store (ADR 0047) before a block can point at it. `leaf-editor` must stay plain Rust with no files
or dialogs (rule 3), and a picker is asynchronous: the owner may click elsewhere while it is open.

## Decision

- **Slash entries have an action**, `SlashAction::Block(kind)` or `SlashAction::Image`. Choosing "Image" removes the
  typed `/image` (its own undo step, like every slash entry) and records a request anchored where it was typed.
  `PageEditor::take_image_request()` tells the UI, once, to open the picker; `cancel_image()` drops it.
- **`PageEditor::insert_image(asset)`** places the block: at the request's anchor if there is one, else at the caret.
  An empty text block becomes the image; otherwise the image goes below. The caret goes into the empty caption. One
  undo step. An empty caption is never replaced (a second image goes below it).
- **The UI fetches the picture**: the file picker (`prompt_for_paths`, one file), or the clipboard; it stores it with
  `import_asset` / `import_asset_file`, then calls `insert_image`.
- **What a paste inserts** (`paste_kind`): Leaf's own blocks, then text, then a picture. Spreadsheets and word
  processors put a picture of the copied cells next to the text, and the text is what the owner wants. A copied file
  counts as a picture, not as text (GPUI's `ClipboardItem::text` would return its path); if it turns out not to be an
  image, it pastes as its path, as before.
- **Drawing**: until E07-S08 draws pictures, an image row is a frame with an image icon and its alt text, and the
  caption below it is ordinary editable text (the key rules for captions are ADR 0020's).
- **Errors** (not an image, no workspace folder) are logged and nothing is inserted, like other UI action failures
  until notifications exist.

## Alternatives considered

- **The editor opening the picker or reading files:** breaks rule 3 and can't be tested or built for WebAssembly.
- **A placeholder image block created before the file is picked:** a cancelled picker would leave an empty block
  and an undo step behind; the anchored request leaves nothing.
- **A picture before text on paste:** pasting cells from a spreadsheet would insert a picture of them.

## Consequences

- E07-S08 only has to replace the frame with the picture; insertion is done.
- Drag-and-drop of image files onto the page can reuse `insert_image` later.
- Plain English: "/image" opens a file picker and puts the picture where you typed it; pasting a screenshot or a
  copied image adds it below the line you're on. Each image has a caption you can type in. Until pictures are drawn
  (next story), the image shows as a grey frame.
