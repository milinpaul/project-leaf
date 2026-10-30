# 0064. Cover gallery: sections of thumbnails, and colour and gradient covers as SVG files

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-30
- **Story:** E06-S16

## Context

The cover picker was a small grid of six labelled thumbnails. The owner asked for a picker that looks like Notion's
gallery (a "Gallery" tab with "Remove" on the right, then sections of wide thumbnails four to a row) and for calm solid
colour and gradient covers alongside the six drawn scenes (ADR 0045).

## Decision

- **Sixteen new built-in covers**, stored like the others (`builtin:<name>`, never renamed once shipped):
  eight solid colours (Sage, Mist, Sand, Blush, Lavender, Seafoam, and the deeper Slate and Pine, which sit calmly in
  dark mode) and eight gradients (Sunrise, Lagoon, Meadow, Glacier, Twilight, and the soft "mesh" glows Aurora, Petal
  and Haze). Colours are muted on purpose.
- **Each is a tiny SVG file** (under 2 KB) in `crates/leaf-ui/assets/covers/`, drawn with `img()` like every other
  cover. Mesh gradients are a few blurred-looking radial gradients on a base colour.
- **Sections live in `leaf-editor`**: `BuiltinCover::section()` puts each cover in `CoverSection::ColourAndGradient`
  or `CoverSection::Illustration`; the picker shows the sections in order, plain covers first.
- **The picker** (`leaf_ui::cover_picker`) shows a "Gallery" tab, "Remove" while the page has a cover, then each
  section as a grid of 112 × 60 px rounded thumbnails, four per row, with the name as a tooltip. The page's current
  cover is outlined in the accent colour. The list scrolls past 360 px.
- `leaf_ui::covers` keeps one table of (cover, path, bytes), so a new cover is one variant, one file and one row.
- Out of scope: Upload, Link and Unsplash tabs (no network in Phase 0; uploads need their own story) and Reposition.

## Alternatives considered

- **Colours and gradients drawn in code** (`bg()` / `linear_gradient`): no files, but a second way to draw covers,
  and GPUI's gradients take only two stops, too plain for the mesh glows. The SVG files are a few hundred bytes each.
- **Tabs for Colours and Pictures:** more clicks; one scrolling gallery shows everything at a glance.

## Consequences

- The app grows by about 6 KB. Twenty-two covers in one popover; more covers mean a longer scroll, not a wider picker.
- Plain English: the cover picker now looks like a gallery, with sixteen calm colour and gradient covers first and the
  six pictures after them. The cover you have is outlined, and you can remove it from the picker too.
