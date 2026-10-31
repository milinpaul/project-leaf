# 0074. A bundled code font, set before the UI library starts, for a fast cold start

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E08-S08

## Context

The spec's budget is "cold start to editable: < 500 ms". The Phase 0 acceptance pass (E08-S03) measured 550–620 ms
on repeat launches of the release build on an M1 Pro. About 220 ms of that was inside `gpui_component::init`: its
theme setup lists every installed font (237 on that Mac) to check that the platform's default faces (`.SystemUIFont`
and Menlo) are installed, and to swap them for ones that are. Leaf replaces both faces right after, so the listing
bought nothing. The library only lists fonts when the theme still names its own defaults, and its setup applies
whatever theme it finds already in place.

## Decision

- Leaf bundles a monospace face for code blocks and inline code, **JetBrains Mono NL** (Regular and Bold, SIL Open
  Font License, about 420 KB), the way it already bundles Inter (ADR 0027). The "NL" cut has no ligatures, so `->`
  and `=>` stay two characters for the caret to move through.
- `theme::install` loads the bundled fonts and puts Leaf's light and dark themes in place, naming Inter and
  JetBrains Mono NL, **before** `gpui_component::init`. The library's setup then finds Leaf's faces and never lists
  the installed fonts. `theme::init` still switches to the chosen appearance after the settings load.

## Alternatives considered

- **A patched copy of gpui-component in the repo (`[patch]`):** keeps Menlo exactly, but adds about 5 MB of copied
  source and a patch to redo on every upgrade.
- **A fork on GitHub:** the same upkeep, plus a fork for the owner to own.
- **Naming Menlo explicitly:** the library compares against its default by name, so `Menlo` still triggers the listing.
- The owner chose the bundled font (2026-10-31).

## Consequences

- Code looks the same on macOS, Windows and Linux, and no longer depends on Menlo (or a Linux distro's monospace font)
  being installed. Code blocks look slightly different from before.
- The app grows by about 420 KB. Italic code is drawn from the regular face.
- A test checks that the library's setup keeps Leaf's faces (`theme/tests.rs`); if a future gpui-component starts
  probing fonts some other way, the cold-start benchmark (`LEAF_STARTUP_TIMING`) shows it.
- Plain English: Leaf now carries its own font for code, like it does for text. That spares it a slow check of every
  font on the computer at launch, which was most of the gap to opening in under half a second.
