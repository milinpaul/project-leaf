# 0038. Welcome screen: one window that switches between the welcome screen and a workspace

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E06-S09

## Context

Leaf opened `~/Documents/Leaf/My Workspace.leaf` (creating it) on every launch with no other choice. The owner asked
for a premium start page to create, open and switch workspaces, shown on the first launch, when the last workspace
can't open, and from File → Switch Workspace…; otherwise Leaf reopens the last workspace (E06-S08, ADR 0037).

## Decision

- **One window, two screens.** The window's top view (`leaf_ui::AppView`) shows either the welcome screen or a
  workspace. Switching drops the open workspace first, which saves it and releases its folder lock, so the same
  folder can be opened again at once. No second window per workspace for now.
- **Launch.** A path on the command line, else the last workspace. If neither exists (first launch) or opening fails,
  the welcome screen shows with the reason in plain words ("already open in another Leaf window", "can't find …").
  Leaf no longer creates a workspace without being asked.
- **The welcome screen asks, the app view does.** The welcome screen emits Open(folder) or Create(folder); the app
  view opens or creates and reports problems back. Folder kinds and safe names are decided in `leaf-store`
  (`inspect`, `new_workspace_path`), so they are tested without a window.
- **Native folder picker** (GPUI's `prompt_for_paths`) for Open, Locate… and the create form's Change…. Where the
  system has none (a Linux desktop without the portal service), the screen says so and suggests `leaf <folder>`.
- **Menus.** File → New Workspace… (Shift+Cmd/Ctrl+N), Open Workspace… (Cmd/Ctrl+O), Open Recent (a submenu rebuilt
  whenever the recent list changes; missing workspaces left out) and Switch Workspace… (Shift+Cmd/Ctrl+O).

## Alternatives considered

- **A separate window for the welcome screen:** two kinds of window to size, restore and close; one window that
  changes content is simpler and is what Obsidian does.
- **Keep auto-creating the default workspace on first launch:** quicker to a page, but the owner never chooses
  where their notes live, which matters for a local-first app.

## Consequences

- E06-S08's recent list now has its screen. Locate… replaces a missing entry by the folder the owner points at.
- Window tests drive the app view directly (launch, create, switch, errors); the folder picker itself is only checked
  by hand, as test windows have no system dialogs.
- Plain English: the first time you open Leaf, you choose to create a workspace or open one. After that Leaf
  reopens your last workspace; use File → Switch Workspace… to go back to the start page and pick another.
