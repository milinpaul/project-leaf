# 0002. GPUI with gpui-component for the UI

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E00-S03, E00-S04 (spike findings below)

## Context

Leaf must feel instant (keystroke to paint under 16 ms on a 10,000-block page) and be native on macOS,
Windows and Linux, with no webview. We need text input, menus, a sidebar tree and dialogs without building
every widget ourselves.

## Decision

Use [GPUI](https://www.gpui.rs) (Zed's GPU-rendered UI framework) with
[gpui-component](https://github.com/longbridge/gpui-kit) for ready-made widgets. Only `leaf-ui` and
`leaf-app` may use them, and `leaf-app` never imports gpui-component directly: `leaf-ui` wraps it, so a
framework swap stays inside one crate.

**Pinned versions** (workspace `Cargo.toml`), upgraded together and deliberately:

| Crate | Version | Why this one |
|---|---|---|
| `gpui-component` | `=0.8.1` | Latest release. |
| `gpui` (package `gpui-pre`) | `=0.3.10` | The exact version gpui-component pins: a published snapshot of Zed at `08cbb88`. The crates.io `gpui` (0.2.2) is older and incompatible. |
| `gpui-platform` (package `gpui-pre-platform`) | `=0.3.10` | Provides `application()`; matches gpui. |

`gpui-platform` features: `runtime_shaders`, `font-kit`, `x11`, `wayland` (reasons in the findings).

## Alternatives considered

- **Tauri / Electron:** a webview, which breaks the native-speed principle.
- **egui / iced:** immediate-mode or Elm-style; weaker rich-text editing and less polished desktop widgets.
- **Slint:** good widgets, but custom rich-text editing is harder and the licence is more restrictive.
- **GPUI alone:** fast, but we would build every widget; gpui-component saves months.

## Spike findings (E00-S04)

- **It works.** `leaf-app` opens a window with a gpui-component sidebar and text input. Typing reaches the
  input in a GPUI test (`crates/leaf-ui/src/shell.rs`).
- **Text needs `font-kit`.** Without that feature, gpui's macOS backend uses a text system that draws
  nothing. The window opens, but every label is blank and nothing fails.
- **Shaders compile at startup (`runtime_shaders`).** Building without it needs the Metal compiler, which
  recent Xcode versions ship as a separate "Metal Toolchain" download. Runtime shaders avoid that on every
  machine and CI runner, at a small startup cost. Revisit against the 500 ms cold-start budget in E08.
- **Linux windowing is opt-in.** The `x11` and `wayland` features must be on, or Linux builds have no
  windowing backend.
- **Build cost.** About 520 crates; a cold debug build of the UI takes about 3 minutes on an M-series Mac.
- **Dependency hygiene.** GPUI brings in 8 crates marked unmaintained (for example `paste`, `instant`,
  `ttf-parser`) and the `0BSD` licence (`enum-iterator`). `deny.toml` now allows `0BSD` and fails
  "unmaintained" only for our direct dependencies; known vulnerabilities still fail for every crate.
- **No networking on desktop.** gpui-kit-assets uses an HTTP client (`reqwest`) only on WebAssembly; desktop
  builds embed assets with `rust-embed`.
- **Pain points.** Documentation is thin: the README targets the `gpui_kit` umbrella crate, so APIs had to be
  read from source. gpui's platform layer is a separate crate (`gpui-pre-platform`), which is easy to miss.
- **Icons need registering (found in E07-S10).** gpui-component draws icons such as the checkbox tick from
  `gpui-kit-assets` (`=0.8.1`), which the app must register with `with_assets`; without it they render as blank
  shapes. `leaf_ui::assets()` provides it, so `leaf-app` still never imports gpui-component.
- **Keys inside inputs.** Inputs bind Tab, Backspace and the arrows to their own actions; a parent can catch
  those actions in the capture phase (`capture_action`) and stop them, which is how the block editor takes Tab.
- **Missing for Leaf.** No block-based rich-text editor. gpui-component's `Input`/`Editor` are single text
  areas, so the block editor (E07) needs our own text element. `Sidebar`, `Tree`, `Dialog`, menus and
  popovers cover the shell (E06).

## Consequences

- Native, GPU-rendered UI on all three OSes, with a widget kit to start from.
- We track a pre-release snapshot of Zed's GPUI, so upgrades can break APIs; pinning plus single-crate
  wrapping keeps that contained.
- The block editor's text rendering is ours to build on GPUI primitives.
- Plain English: the app draws its own screen using the graphics card, like a game engine. That makes it fast
  and native, and a ready-made widget kit saves us building buttons and menus from scratch.
