# ADR-045. Page covers: a stored name, drawn from bundled SVG files

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E06-S11

## Context

A page can have a cover: a full-width band above the title, with the page icon overlapping its bottom edge, as in
Notion. Phase 0 ships a few built-in covers (E06-S12: Dawn, Forest, Ocean, Paper, Night, Travel); uploading your own image
may come later. Two questions: how a page stores its cover, and how the app draws one.

## Decision

- **Stored as a name** in the page's metadata in the workspace doc, next to `title` and `icon`: `cover:
  "builtin:dawn"`. Set and cleared with `SetPageCover { page, cover }`, one undo step like `SetPageIcon`. The
  sidebar, header and (later) database views know a page's cover without opening the page.
- **The model stores any non-empty name.** `leaf-editor` (`PageCover::parse`) decides which names this version can
  draw; anything else (a newer built-in, a future `image:<asset hash>`) is kept untouched and shown as a plain band
  that can still be removed (rule 5).
- **Built-in names never change once shipped**: they live in documents.
- **Drawn from bundled SVG files** (`crates/leaf-ui/assets/covers/<name>.svg`, embedded in the binary) with GPUI's
  `img()`, which renders SVG in full colour, scaled to fill the band (`ObjectFit::Cover`). Files are 1200×300
  (4:1); GPUI rasterises SVG at twice its size, so a cover stays sharp across the band without an outsized texture.
- **Layout:** the band spans the page area and its height follows its width (3.5:1, kept between 160 and 320 px;
  E06-S12), so a very wide window doesn't crop most of the picture away and a narrow one keeps its middle. An
  invisible spacer sets the height, because an aspect ratio on the band itself would narrow it once the height hits
  its limit. Designs keep their key elements in a central safe area, since the edges may be cropped. The icon is
  pulled up half its size to sit on the band's edge. "Add cover" sits beside "Add icon" and shows on hover; "Change
  cover" and "Remove" show on hover over the band. The picker is a popover of thumbnails.
- **Bitmap covers (E06-S15):** a cover may instead be a WebP file made with an image model, when it is clearly
  richer than a hand-built SVG. Travel is the first. Recipe: generate at 1536 × 1024 with the prompt in E06-S15;
  crop the content strip from the vertical centre (aiming for about 4:1); upscale 2× offline (ImageMagick
  `-filter Lanczos -resize 200% -unsharp 0x1+0.6+0.02`; an AI upscaler such as Real-ESRGAN is better where it can
  run) so very wide high-resolution windows stay sharp; save as WebP (quality 85). Each cover stays under 100 KB
  (tested). `img()` picks the decoder from the file extension; GPUI already links the WebP decoder, so no new
  dependency. Licence: OpenAI's Terms of Use (checked 2026-10-29) say the user owns the output and
  OpenAI assigns its rights in it, "if any"; other users may get similar output, and AI images may have no copyright
  protection, so Leaf can ship them but can't stop others using similar images. Check the current terms before adding
  more. Image models don't reliably respect safe areas,
  so check every bitmap cover in the real app at narrow, normal and very wide widths.

## Alternatives considered

- **Gradients drawn in code:** no files at all, but GPUI's `linear_gradient` takes only two colours, too plain for
  designed covers (layers, a sun, waves).
- **Photos (PNG/JPEG):** larger app, licensing questions, and soft at wide window sizes.
- **Store the cover in the page's own doc:** the sidebar and future table views would have to open every page.
- **A `props`-style map for covers:** nothing to put in it yet; a single string leaves room for `image:<hash>`.

## Consequences

- Adding a built-in cover is one SVG file, one `BuiltinCover` variant and one asset path.
- Covers from newer versions survive in older ones, shown as a plain band.
- Each cover in use costs one cached texture (about 2400×600 pixels); fine on desktop.
- Plain English: a page remembers its cover by name. Leaf draws the covers it knows from small picture files built
  into the app, and keeps any cover name it doesn't know instead of throwing it away.
