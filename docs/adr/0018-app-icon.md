# 0018. App icon: one SVG, generated per-OS files, a bundle for `just run`

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E08-S05

## Context

Leaf showed a generic icon in the Dock and taskbar and was called "leaf-app" in the macOS menu bar. Every system
takes an app's icon from a different place:

- **macOS** reads it from the app bundle (`Leaf.app/Contents/Resources/*.icns`, named in `Info.plist`). A bare binary
  has no bundle, so it gets the generic icon.
- **Windows** reads it from a resource built into the `.exe`; GPUI uses icon resource 1 for its windows.
- **Linux** desktops find it through a `.desktop` entry whose name matches the window's `app_id`. X11 also accepts
  an image on the window itself.

## Decision

- **One master artwork:** `crates/leaf-app/resources/icon.svg`: a white leaf on a green rounded square. Its veins
  are level lines, like a paragraph of text, so it reads as a leaf and a page. It follows the macOS layout grid.
- **Generated files are committed:** `just icons` (`scripts/icons.py`) renders PNG files (16 to 1024 px), `Leaf.icns`
  (via macOS `iconutil`) and `leaf.ico` (written directly) with `resvg`. Builds and CI use the committed files and
  need no image tools.
- **Identity:** `io.github.milinpaul.Leaf`, a reverse-DNS name under the owner's GitHub that no one else can claim.
  It is the macOS bundle identifier, the Windows app ID and the Linux `app_id` and `.desktop` file name.
- **macOS:** `just run` builds the binary, wraps it in `target/debug/Leaf.app` (`scripts/macos_app.py`) and runs it
  from there, so the Dock, app switcher and menu bar show Leaf. The copy is an APFS clone, so it is instant.
  `just install` (E08-S07) makes the same bundle from a release build and copies it into `/Applications`, replacing
  an older Leaf but refusing to delete a `Leaf.app` with another bundle ID.
- **Windows:** `build.rs` embeds `leaf.ico` with `embed-resource`. It fails the build if a Windows build cannot embed
  it, rather than ship without an icon.
- **Linux:** the window gets `app_id` and, on X11, the 256 px PNG via `image`. The `.desktop` entry and PNG files are ready
  for the installers in E08-S04.

## Alternatives considered

- **Set the Dock icon at runtime on macOS** (`NSApplication.setApplicationIconImage`): needs Objective-C calls
  (`unsafe`, forbidden) and still leaves the name "leaf-app" and a generic icon until launch.
- **Generate icons during the build:** would put an SVG renderer in every build and in CI, for files that change
  only when someone redraws the icon.
- **Use `cargo-bundle` or `cargo-packager` now:** a new tool for one Info.plist; E08-S04 decides on the installer
  tool and can reuse the bundle script or replace it.

## Consequences

- Two new direct dependencies, both already in the tree through GPUI: `image` (Linux only, PNG only) and
  `embed-resource` (build-time).
- Changing the icon means editing the SVG, running `just icons` and committing the outputs; `just setup` installs
  `resvg`, and the `.icns` needs a Mac.
- Plain English: Leaf now has its own icon, a white leaf whose veins look like lines of text, on a green tile. It shows
  in the Dock and app switcher on Mac, on the taskbar on Windows, and on Linux windows.
