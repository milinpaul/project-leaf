# 0028. App settings in one JSON file; a Settings dialog over the page

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E06-S02

## Context

Leaf should reopen the way it was left (window size and position, sidebar width) and let the owner choose light or
dark. These belong to the app, not to a workspace: they must not travel with a workspace folder, and they apply
before any workspace is open. The owner also asked for a Settings screen, opened from a cog at the bottom of the
sidebar. Settings are a convenience: nothing about them may stop Leaf from starting or put pages at risk.

## Decision

- **One JSON file per user**, `settings.json` in the OS's settings folder: `~/Library/Application Support/Leaf`
  (macOS), `%APPDATA%\Leaf` (Windows), `$XDG_CONFIG_HOME/leaf` or `~/.config/leaf` (Linux). The format and its rules
  live in `leaf-store` (`settings.rs`), where they are tested; `leaf-ui` holds the loaded copy while Leaf runs.
- **It never blocks startup.** Missing → defaults. Damaged → logged, moved aside to `settings.json.bad`, defaults.
  Fields from a newer Leaf are kept on save; an unknown value for a known field reads as its default.
- **Written safely and rarely:** to a temporary file renamed into place, on window blur and on quit, not on every
  pixel of a drag.
- **Sane on restore:** the sidebar stays 180–480 px wide; a window smaller than 480 × 320 or no longer on any
  screen (an unplugged monitor) opens centred instead.
- **Settings window:** a large panel (about 80% of the app window, 760–1080 px wide) floating over the dimmed page.
  Its left column is styled like the app's sidebar, with section groups split by hairline dividers and the version at
  the foot; the right side has a large heading and the section's content in rounded cards. Sections: Appearance
  (three preview cards showing a miniature Leaf in System, Light and Dark), Workspace (name, folder, Show in
  Finder/Explorer/Files) and About (app icon, version, font licence). Opened from the sidebar's cog, the
  Settings… menu item and Cmd/Ctrl+,; About Leaf opens it on About. Esc or a click outside closes it.
- **Menus:** Leaf (About, Settings…, Quit), File (New Page, Close Window), Edit (Undo, Redo, Cut, Copy, Paste, Select
  All), View (Toggle Sidebar), and Window (Minimize, Zoom) on macOS. Shortcuts use Cmd on macOS and Ctrl elsewhere.

## Alternatives considered

- **Settings inside each workspace:** the window size would change with the workspace and couldn't apply before one
  opens; it also mixes app preferences into the owner's data.
- **SQLite for settings:** heavier than needed for a handful of values, and harder to read or fix by hand.
- **A full-page Settings view:** roomier, but the owner preferred keeping their place in the page (a large dialog).
- **The first-launch open/create dialog in this story:** moved to the welcome screen (E06-S09), which covers it fully.

## Consequences

- E06-S08 (recent workspaces) now only adds its list to this file.
- On Windows and Linux GPUI shows no menu bar; the shortcuts and the sidebar's cog still reach everything.
- Plain English: Leaf remembers how you left it (window, sidebar) in a small settings file on your computer, and has
  a Settings window, opened from the cog at the bottom of the sidebar, where you can pick light or dark, see where
  your pages are stored and check the version.
