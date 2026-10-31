# ADR-078. SVG images are checked before GPUI draws them

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E08-S12

## Context

Image blocks hand the stored file's path to GPUI's `img()`. GPUI (`gpui-pre` 0.3.10) draws anything that isn't a
known raster format as an SVG, with `usvg` 0.46 on its **default options**, which Leaf can't change:

- `usvg`'s default resolver **reads any image file an `href` names**, by absolute path or relative to the working
  directory, and draws nested SVGs with the same options. A crafted SVG could show your other pictures, which is
  in scope in `SECURITY.md`. `usvg` has no network code, and fonts come only from the system's installed fonts.
- The picture is drawn at **twice its own size**, with no cap: an SVG saying `width="100000"` asks for gigabytes.
- XML is parsed by `roxmltree` with DTDs allowed. It already refuses entity loops and "billion laughs" files.
- `usvg` walks the element tree recursively, so very deep nesting could overflow the stack.

## Decision

Before an image is drawn, `Workspace::drawable_asset` clears it:

- **Raster pictures** (PNG, JPEG, GIF, WebP, BMP, TIFF, AVIF, HEIC, by their first bytes) pass as before.
- **Anything else** goes through `check_svg`, which reads the file with `roxmltree` using `usvg`'s own parse options,
  then refuses it when:
  - an `href` (either namespace) or a CSS `url()` points anywhere but inside the SVG (`#id`), or isn't an
    embedded PNG, JPEG, GIF or WebP (`data:image/...`). Embedded SVGs are refused too, since `usvg` would draw them
    with the same options;
  - it has an `@import`;
  - the file is over **10 MB**, has over **250,000** nodes, or nests over **256** deep;
  - the picture is wider or taller than **8,192** CSS pixels, or bigger than **8 million** pixels. That's about
    128 MB once GPUI doubles it. The size is worked out the way `usvg` does it: `width`/`height` in absolute units,
    else the `viewBox`, else 100 × 100.
- A refused SVG shows the image frame with the reason in plain words. The file is kept exactly as it is (rule 5),
  and the verdict is cached per picture, since a stored picture never changes (it's named by its hash).
- `roxmltree` becomes a direct dependency of `leaf-store`. It's the same version `usvg` already uses inside GPUI,
  so no new crate is built.

## Alternatives considered

- **Configure `usvg` ourselves** (a resolver that loads nothing): GPUI builds its renderer internally with no hook.
  It would need a fork or an upstream change; worth proposing upstream, but not blocking on.
- **Draw SVGs ourselves** with `resvg`, and hand GPUI the pixels: full control, but it duplicates GPUI's font setup
  and adds a second renderer to maintain.
- **Strip the bad parts and draw the rest:** rewriting SVGs is easy to get subtly wrong, and the stored file must
  not change anyway.
- **Refuse such SVGs at import:** wouldn't cover workspaces made elsewhere. The draw-time check covers both.

## Consequences

- An SVG that uses linked images or a very large canvas no longer draws. It shows a frame saying why; the file
  stays in the workspace unchanged.
- The first time each SVG is drawn, Leaf reads and parses it once, on top of GPUI's own parse.
- If GPUI ever exposes its `usvg` options, the reference check can move there. The size limits stay.
- In plain English: a picture can't sneak other files from your computer onto the page, and a picture that claims
  to be enormous can't freeze Leaf; you see a short note instead.
