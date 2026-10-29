//! The welcome screen on its own: the keyboard on the recent list, removing a missing workspace, and
//! the create form's checks.

use std::path::PathBuf;

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::{Welcome, WelcomeEvent};
use crate::settings;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("leaf-ui-welcome-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A welcome screen whose recent list is `present` (folders that exist) then `gone`.
fn welcome_with<'a>(
    dir: &std::path::Path,
    cx: &'a mut TestAppContext,
) -> (
    Entity<Welcome>,
    std::rc::Rc<std::cell::RefCell<Vec<WelcomeEvent>>>,
    &'a mut VisualTestContext,
) {
    cx.update(|cx| crate::init(None, cx));
    let here = dir.join("Here.leaf");
    std::fs::create_dir_all(&here).unwrap();
    cx.update(|cx| {
        settings::update(cx, |s| {
            s.remember_workspace(&dir.join("Gone.leaf"), 1);
            s.remember_workspace(&here, 2);
        });
    });
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = events.clone();
    let location = dir.to_owned();
    let (welcome, cx) = cx.add_window_view(move |window, cx| {
        let welcome = Welcome::new(None, location.clone(), cx);
        window.focus(&welcome.focus, cx);
        welcome
    });
    cx.update(|_, cx| {
        cx.subscribe(&welcome, move |_, event: &WelcomeEvent, _| {
            seen.borrow_mut().push(event.clone());
        })
        .detach();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    (welcome, events, cx)
}

#[gpui::test]
fn arrows_and_enter_open_a_recent_workspace_but_not_a_missing_one(cx: &mut TestAppContext) {
    let dir = temp_dir("keys");
    let (welcome, events, cx) = welcome_with(&dir, cx);
    cx.simulate_keystrokes("down"); // Here (most recent)
    cx.simulate_keystrokes("enter");
    assert!(matches!(events.borrow().as_slice(), [WelcomeEvent::Open(path)] if path.ends_with("Here.leaf")));

    cx.simulate_keystrokes("down enter"); // Gone: missing, so nothing opens
    assert_eq!(events.borrow().len(), 1);
    cx.simulate_keystrokes("down");
    assert_eq!(
        welcome.read_with(cx, |w, _| w.selected),
        Some(0),
        "wraps to the top"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn remove_forgets_a_missing_workspace(cx: &mut TestAppContext) {
    let dir = temp_dir("remove");
    let (welcome, _, cx) = welcome_with(&dir, cx);
    cx.update(|_, cx| welcome.update(cx, |w, cx| w.remove_recent(1, cx)));
    let names: Vec<String> = welcome.read_with(cx, |w, _| w.recent.iter().map(|r| r.name.clone()).collect());
    assert_eq!(names, ["Here"]);
    let saved = cx.update(|_, cx| settings::get(cx).recent_workspaces(|_| true).len());
    assert_eq!(saved, 1, "the settings forgot it too");
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn the_create_form_checks_the_name_before_asking(cx: &mut TestAppContext) {
    let dir = temp_dir("create");
    let (welcome, events, cx) = welcome_with(&dir, cx);
    cx.update(|window, cx| welcome.update(cx, |w, cx| w.start_create(window, cx)));
    cx.simulate_input("Here"); // replaces the selected "My Workspace"; Here.leaf exists already
    cx.simulate_keystrokes("enter");
    let problem = welcome.read_with(cx, |w, _| w.create.as_ref().and_then(|f| f.problem.clone()));
    assert_eq!(
        problem.as_deref(),
        Some("There's already something called “Here.leaf” there.")
    );
    assert!(events.borrow().is_empty());

    cx.simulate_keystrokes("backspace backspace backspace backspace");
    cx.simulate_input("Garden");
    cx.simulate_keystrokes("enter");
    assert!(
        matches!(events.borrow().as_slice(), [WelcomeEvent::Create(path)] if *path == dir.join("Garden.leaf"))
    );

    cx.simulate_keystrokes("escape");
    assert!(
        welcome.read_with(cx, |w, _| w.create.is_none()),
        "Escape leaves the form"
    );
    let _ = std::fs::remove_dir_all(dir);
}
