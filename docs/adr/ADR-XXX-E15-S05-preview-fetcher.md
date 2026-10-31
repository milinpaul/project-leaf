# ADR-XXX. The web bookmark preview fetcher, and fetching only once the owner says yes

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E15-S05

## Context

ADR-080 chose how Leaf may go online for web bookmark previews, and its trial showed the libraries and limits work.
This story builds the real fetcher and wires it into the app. The setting that lets the owner allow previews, and the
question asked the first time, come in E15-S06; until then nothing may go online (CLAUDE.md rule 6).

## Decision

- **`leaf-fetch`**, a new native crate, holds everything that goes online:
  - `Fetcher::preview(address)` fetches the page's `<head>` preview, then its picture and icon;
  - every ADR-080 limit is built in;
  - pages are decoded by their header or `<meta charset>` (`encoding_rs`);
  - pictures must be PNG, JPEG, GIF, WebP or SVG, the types Leaf can draw. An `.ico` favicon is skipped;
  - a picture that fails or is too big is left out; only an unreadable page is an error.

  It depends on `leaf-model` only, for the shared address check.
- **Opt-in, from this story on:** `settings.json` gains `web_previews`. It is missing until the owner answers, and
  previews are allowed only when it is `true`. Nothing in this story sets it, so the app stays offline until E15-S06
  adds the question and the setting.
- **Requests:** the page editor notes a bookmark's request when it is made on this device (offer, address box, Turn
  into) or when "Refresh preview" is chosen. It hands requests out only while previews are allowed, and drops them
  otherwise, so allowing previews later never fetches for old bookmarks on its own (E15-S06 decides about those).
  "Refresh preview" shows only when previews are allowed.
- **Running a fetch:** `shell/preview_fetch.rs` runs the fetcher on GPUI's background executor, so typing never waits,
  and marks the card as fetching. When the fetch finishes, it stores the pictures as assets and saves the preview
  through `SetWebBookmarkPreview`, off the undo history. That command does nothing if the bookmark changed meanwhile.
- **Dependencies** (new in the build; `cargo deny` passes):
  - `ureq` `=3.5.0`, with `rustls` and `gzip`;
  - `html5ever` `=0.27.0`, the version already in the tree;
  - `url` and `encoding_rs`, already in the tree.

  `deny.toml` allows CDLA-Permissive-2.0 for `webpki-roots` only.

## Alternatives considered

- **Fetch from the editor:** the editor is a core crate and never goes online (rule 6). It also has to compile for wasm.
- **An async HTTP client on GPUI's executor:** it would bring `hyper` and `tokio`. A blocking call on a background
  thread is simpler and fits ureq.
- **Build the fetcher without wiring it in until E15-S06:** it would leave the request and storage paths untested
  together. Wiring it behind a setting that is off keeps it testable and offline.

## Consequences

- With previews off (the default), Leaf makes no network request. A window test checks this against a local server.
- A preview that arrives after the page was closed is dropped. E15-S06 fetches missing previews when a page opens.
- Upgrading `ureq` needs a look at its resolver hook; the address tests catch a break.
- Plain English: Leaf can now fetch a web page's title, description and pictures for a bookmark, safely and in the
  background. It won't do so until you allow it, which the next story adds.
