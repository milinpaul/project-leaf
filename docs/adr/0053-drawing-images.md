# 0053. Drawing images: natural size shrunk to fit, resized by a handle, saved once

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E07-S08

## Context

Since E05-S07 an image block stores its picture in the workspace's `assets/` folder, but the page drew only a frame.
The block already has an optional `width` prop (BLOCK_MODEL.md). We need to draw the picture without slowing a long
page, show something while it loads, and let the owner resize it so the size is kept.

## Decision

- **Size.** A picture is drawn at its stored width, or its natural width when none is stored, never wider than the
  room its row has, and with height following its shape (`leaf_editor::image_display_size`). The width is in logical
  pixels, so the same page looks the same on any screen.
- **Loading.** GPUI's image loader reads and decodes the file in the background the first time the row is drawn. The
  virtual list only draws rows on screen, so pictures load as they scroll into view. Until then the row shows a
  "Loading image…" frame; a file that can't be decoded shows "This image can't be shown".
- **Room.** Each image row records its width while painting; when that changes (a resized window) the window draws
  once more so the picture fits. Before the first measure, the reading column's width is assumed.
- **Resize.** A bar on the picture's right edge, shown on hover, is dragged with GPUI's drag. The picture follows the
  pointer live (between 64 px and the room it has; `leaf_editor::resized_image_width`), and the width is saved once
  when the button is let go, as one undo step (`PageEditor::set_image_width`). A click without moving saves nothing.

## Alternatives considered

- **Save the width on every move:** fills the undo history and the save log with steps nobody wants.
- **Width as a share of the column:** survives column changes better, but the model's prop is already a pixel width
  and Notion behaves the same way; a share can come later with full-width pages if needed.
- **Load pictures ourselves on a thread pool:** GPUI's asset cache already does this and shares decoded images
  between frames and windows.

## Consequences

Pictures appear on the page and keep the size the owner gives them. A picture never overflows the column, even when
its stored width is larger, so a page made on a wide screen still reads on a narrow one. GPUI keeps decoded pictures
in memory while the window is open; a page with many very large pictures uses memory accordingly (watch this when
pages get long).

Plain English: images now show up in the page. Drag the bar on an image's right edge to make it smaller or larger;
Leaf remembers the size, and Undo puts it back.
