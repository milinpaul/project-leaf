//! Leaf's actions, their keyboard shortcuts and the menu bar.
//!
//! Shortcuts use Cmd on macOS and Ctrl on Windows and Linux (the `command!` macro), so one list serves
//! every OS. On Windows and Linux GPUI draws no menu bar; the shortcuts work there all the same,
//! and Settings and About are also reachable from the sidebar's cog.

use gpui::{KeyBinding, Menu, MenuItem, actions};

/// `key` with the platform's command modifier, Cmd on macOS and Ctrl elsewhere, as a `&'static str`
/// key bindings need. (GPUI's `secondary` spelling doesn't parse in key bindings.)
#[cfg(target_os = "macos")]
macro_rules! command {
    ($key:literal) => {
        concat!("cmd-", $key)
    };
}
#[cfg(not(target_os = "macos"))]
macro_rules! command {
    ($key:literal) => {
        concat!("ctrl-", $key)
    };
}
use gpui_component::input::{Copy, Cut, Paste, Redo, SelectAll, Undo};

actions!(
    leaf,
    [
        /// Quits Leaf. Everything unsaved is saved first.
        Quit,
        /// Shows Leaf's version and licences (the Settings dialog's About section).
        About,
        /// Opens the Settings dialog.
        OpenSettings,
        /// Creates a page and opens it.
        NewPage,
        /// Opens the quick switcher to jump to a page by its title.
        QuickSwitch,
        /// Closes the window; with one window this quits, after saving.
        CloseWindow,
        /// Hides or shows the sidebar.
        ToggleSidebar,
        /// Minimises the window (macOS Window menu).
        Minimize,
        /// Zooms the window to fill the screen and back (macOS Window menu).
        Zoom,
    ]
);

/// Every app-wide shortcut. The Edit menu's shortcuts (Cmd/Ctrl+Z, C, V, …) come with the text
/// fields themselves, so they work wherever text is being edited.
#[must_use]
pub fn key_bindings() -> Vec<KeyBinding> {
    let mut bindings = vec![
        KeyBinding::new(command!("q"), Quit, None),
        KeyBinding::new(command!(","), OpenSettings, None),
        KeyBinding::new(command!("n"), NewPage, None),
        KeyBinding::new(command!("p"), QuickSwitch, None),
        KeyBinding::new(command!("w"), CloseWindow, None),
        KeyBinding::new(command!("\\"), ToggleSidebar, None),
    ];
    if cfg!(target_os = "macos") {
        bindings.push(KeyBinding::new("cmd-m", Minimize, None));
    }
    bindings
}

/// The menu bar: the standard menus a Mac app has, holding only what Leaf can do today.
#[must_use]
pub fn menus() -> Vec<Menu> {
    let mut menus = vec![
        menu(
            "Leaf",
            vec![
                MenuItem::action("About Leaf", About),
                MenuItem::separator(),
                MenuItem::action("Settings…", OpenSettings),
                MenuItem::separator(),
                MenuItem::action("Quit Leaf", Quit),
            ],
        ),
        menu(
            "File",
            vec![
                MenuItem::action("New Page", NewPage),
                MenuItem::action("Go to Page…", QuickSwitch),
                MenuItem::separator(),
                MenuItem::action("Close Window", CloseWindow),
            ],
        ),
        menu(
            "Edit",
            vec![
                MenuItem::action("Undo", Undo),
                MenuItem::action("Redo", Redo),
                MenuItem::separator(),
                MenuItem::action("Cut", Cut),
                MenuItem::action("Copy", Copy),
                MenuItem::action("Paste", Paste),
                MenuItem::action("Select All", SelectAll),
            ],
        ),
        menu("View", vec![MenuItem::action("Toggle Sidebar", ToggleSidebar)]),
    ];
    if cfg!(target_os = "macos") {
        menus.push(menu(
            "Window",
            vec![
                MenuItem::action("Minimize", Minimize),
                MenuItem::action("Zoom", Zoom),
            ],
        ));
    }
    menus
}

fn menu(name: &'static str, items: Vec<MenuItem>) -> Menu {
    Menu {
        name: name.into(),
        items,
        disabled: false,
    }
}

#[cfg(test)]
mod tests;
