use gpui::{Action, MenuItem};

use super::{
    CloseWindow, NewPage, OpenRecent, OpenSettings, OpenWorkspace, QuickSwitch, Quit, ToggleSidebar,
    key_bindings, menus,
};

/// The menus' titles and their items' labels, separators left out.
fn outline() -> Vec<(String, Vec<String>)> {
    menus(&[])
        .into_iter()
        .map(|menu| {
            let items = menu
                .items
                .into_iter()
                .filter_map(|item| match item {
                    MenuItem::Action { name, .. } => Some(name.to_string()),
                    _ => None,
                })
                .collect();
            (menu.name.to_string(), items)
        })
        .collect()
}

#[test]
fn the_standard_menus_are_there() {
    let outline = outline();
    let titles: Vec<&str> = outline.iter().map(|(title, _)| title.as_str()).collect();
    let expected: &[&str] = if cfg!(target_os = "macos") {
        &["Leaf", "File", "Edit", "View", "Window"]
    } else {
        &["Leaf", "File", "Edit", "View"]
    };
    assert_eq!(titles, expected);
    assert_eq!(outline[0].1, ["About Leaf", "Settings…", "Quit Leaf"]);
    assert_eq!(
        outline[1].1,
        [
            "New Page",
            "Go to Page…",
            "New Workspace…",
            "Open Workspace…",
            "Switch Workspace…",
            "Close Window"
        ]
    );
    assert_eq!(
        outline[2].1,
        ["Undo", "Redo", "Cut", "Copy", "Paste", "Select All"]
    );
    assert_eq!(outline[3].1, ["Toggle Sidebar"]);
}

#[test]
fn app_actions_have_their_standard_shortcuts() {
    let bindings = key_bindings();
    // The single keystroke bound to `action`, as (Cmd on macOS / Ctrl elsewhere held, key).
    let shortcut = |action: &dyn Action| {
        let binding = bindings
            .iter()
            .find(|binding| binding.action().partial_eq(action))?;
        let [keystroke] = binding.keystrokes() else {
            return None;
        };
        let keystroke = keystroke.inner();
        let modifier = if cfg!(target_os = "macos") {
            keystroke.modifiers.platform
        } else {
            keystroke.modifiers.control
        };
        Some((modifier, keystroke.key.clone()))
    };
    for (action, key) in [
        (&Quit as &dyn Action, "q"),
        (&OpenSettings, ","),
        (&NewPage, "n"),
        (&QuickSwitch, "p"),
        (&OpenWorkspace, "o"),
        (&CloseWindow, "w"),
        (&ToggleSidebar, "\\"),
    ] {
        assert_eq!(
            shortcut(action),
            Some((true, key.to_owned())),
            "{}",
            action.name()
        );
    }
}

#[test]
fn open_recent_lists_workspaces_that_are_still_there() {
    use std::path::PathBuf;

    use leaf_store::RecentWorkspace;

    let entry = |name: &str, missing: bool| RecentWorkspace {
        path: PathBuf::from(format!("/w/{name}.leaf")),
        name: name.to_owned(),
        opened_at: 0,
        missing,
    };
    let file = menus(&[entry("Work", false), entry("Gone", true)]).remove(1);
    let recent = file
        .items
        .into_iter()
        .find_map(|item| match item {
            MenuItem::Submenu(menu) if menu.name.as_ref() == "Open Recent" => Some(menu),
            _ => None,
        })
        .unwrap();
    assert_eq!(recent.items.len(), 1, "the missing one is left out");
    let MenuItem::Action { name, action, .. } = &recent.items[0] else {
        panic!("an action")
    };
    assert_eq!(name.as_ref(), "Work");
    assert!(action.partial_eq(&OpenRecent {
        path: PathBuf::from("/w/Work.leaf")
    }));
}
