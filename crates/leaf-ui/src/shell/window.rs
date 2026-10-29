//! The shell's window-level behaviour: the app's actions (menus and shortcuts), remembering the
//! window's frame, and opening Settings.

use gpui::{Context, Div, InteractiveElement as _, Subscription, Window, WindowBounds};
use leaf_editor::{Selection, Shortcut, ShortcutAction, shortcut_action};
use leaf_store::WindowFrame;

use super::Shell;
use crate::menus::{
    About, CloseWindow, Minimize, NewPage, OpenSettings, Search, SearchOrLink, ToggleSidebar, Zoom,
};
use crate::settings;
use crate::settings_dialog::{self, Section, WorkspaceInfo};

impl Shell {
    /// Records the window's frame in the settings whenever it moves or resizes, so the next launch
    /// opens it the same way.
    pub(super) fn track_window(window: &mut Window, cx: &mut Context<Self>) -> Subscription {
        cx.observe_window_bounds(window, |_, window, cx| {
            let frame = frame_of(window.window_bounds());
            settings::update(cx, |settings| settings.set_window_frame(frame));
        })
    }

    /// Makes `root` answer the app's actions while this window is focused.
    pub(super) fn with_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|shell, _: &NewPage, window, cx| shell.new_page(window, cx)))
            .on_action(cx.listener(|shell, _: &Search, window, cx| shell.open_search(window, cx)))
            .on_action(cx.listener(|shell, _: &SearchOrLink, window, cx| {
                let text_selected = shell.page_focus.is_focused(window)
                    && matches!(
                        shell.page().and_then(|page| page.selection().ok().flatten()),
                        Some(Selection::Text { .. })
                    );
                match shortcut_action(Shortcut::ModK, text_selected) {
                    ShortcutAction::OpenSearch => shell.open_search(window, cx),
                    ShortcutAction::Link => shell.open_link_popover(window, cx),
                }
            }))
            .on_action(cx.listener(|_, _: &ToggleSidebar, _, cx| {
                settings::update(cx, |settings| {
                    settings.set_sidebar_hidden(!settings.sidebar_hidden());
                });
                cx.notify();
            }))
            .on_action(cx.listener(|shell, _: &OpenSettings, window, cx| {
                shell.open_settings(Section::Appearance, window, cx);
            }))
            .on_action(cx.listener(|shell, _: &About, window, cx| {
                shell.open_settings(Section::About, window, cx);
            }))
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .on_action(cx.listener(|_, _: &Minimize, window, _| window.minimize_window()))
            .on_action(cx.listener(|_, _: &Zoom, window, _| window.zoom_window()))
    }

    /// Opens the search bar over this workspace (Cmd/Ctrl+P, or Cmd/Ctrl+K with no text selected).
    pub fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let root = self.storage.as_ref().map(|storage| storage.root().to_owned());
        crate::search::open(cx.entity().downgrade(), &self.editor, root, window, cx);
    }

    /// Opens the Settings dialog on `section`.
    pub(super) fn open_settings(&self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = WorkspaceInfo {
            folder: self.storage.as_ref().map(|storage| storage.root().to_owned()),
        };
        settings_dialog::open(section, workspace, window, cx);
    }
}

/// The frame to remember for `bounds`: a maximised or full-screen window remembers the size it
/// returns to, and that it was maximised.
pub(super) fn frame_of(bounds: WindowBounds) -> WindowFrame {
    let (rect, maximized) = match bounds {
        WindowBounds::Windowed(rect) => (rect, false),
        WindowBounds::Maximized(rect) | WindowBounds::Fullscreen(rect) => (rect, true),
    };
    WindowFrame {
        x: rect.origin.x.into(),
        y: rect.origin.y.into(),
        width: rect.size.width.into(),
        height: rect.size.height.into(),
        maximized,
    }
}

#[cfg(test)]
mod tests;
