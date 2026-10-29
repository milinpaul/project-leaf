//! The window's top view: what shows at launch, and opening, creating and switching workspaces.

use std::path::{Path, PathBuf};

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::AppView;
use crate::menus::{OpenRecent, SwitchWorkspace};
use crate::settings;
use crate::welcome::WelcomeEvent;

/// A fresh folder for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("leaf-ui-app-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_workspace(path: &Path) {
    drop(leaf_store::Workspace::create(path).unwrap());
}

fn launch<'a>(
    named: Option<PathBuf>,
    dir: &Path,
    cx: &'a mut TestAppContext,
) -> (Entity<AppView>, &'a mut VisualTestContext) {
    let location = dir.join("Documents");
    cx.add_window_view(move |window, cx| AppView::launch(named.clone(), location.clone(), window, cx))
}

fn showing_welcome(app: &Entity<AppView>, cx: &mut VisualTestContext) -> bool {
    app.read_with(cx, |app, _| app.welcome().is_some())
}

fn message(app: &Entity<AppView>, cx: &mut VisualTestContext) -> Option<String> {
    app.read_with(cx, |app, cx| {
        let welcome = app.welcome()?.read(cx);
        welcome
            .message
            .as_ref()
            .map(|m| m.text.clone())
            .or_else(|| welcome.create.as_ref().and_then(|form| form.problem.clone()))
    })
}

fn emit(app: &Entity<AppView>, event: WelcomeEvent, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        let welcome = app.read(cx).welcome().unwrap().clone();
        welcome.update(cx, |_, cx| cx.emit(event));
    });
}

#[gpui::test]
fn the_first_launch_shows_the_welcome_screen(cx: &mut TestAppContext) {
    cx.update(|cx| crate::init(None, cx));
    let dir = temp_dir("first");
    let (app, cx) = launch(None, &dir, cx);
    assert!(showing_welcome(&app, cx));
    assert_eq!(
        message(&app, cx),
        None,
        "nothing went wrong, so nothing to explain"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn a_later_launch_reopens_the_last_workspace(cx: &mut TestAppContext) {
    cx.update(|cx| crate::init(None, cx));
    let dir = temp_dir("reopen");
    let work = dir.join("Work.leaf");
    make_workspace(&work);
    cx.update(|cx| settings::update(cx, |s| s.remember_workspace(&work, 1)));
    let (app, cx) = launch(None, &dir, cx);
    assert!(app.read_with(cx, |app, _| app.shell().is_some()));
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn a_missing_last_workspace_shows_the_welcome_screen_with_the_reason(cx: &mut TestAppContext) {
    cx.update(|cx| crate::init(None, cx));
    let dir = temp_dir("missing");
    let gone = dir.join("Gone.leaf");
    cx.update(|cx| settings::update(cx, |s| s.remember_workspace(&gone, 1)));
    let (app, cx) = launch(None, &dir, cx);
    assert!(showing_welcome(&app, cx));
    assert_eq!(
        message(&app, cx).as_deref(),
        Some("Leaf can't find “Gone”. It may have been moved, renamed or deleted.")
    );
    let missing = app.read_with(cx, |app, cx| app.welcome().unwrap().read(cx).recent[0].missing);
    assert!(missing, "listed, greyed out, not dropped");
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn creating_a_workspace_opens_and_remembers_it(cx: &mut TestAppContext) {
    cx.update(|cx| crate::init(None, cx));
    let dir = temp_dir("create");
    let (app, cx) = launch(None, &dir, cx);
    let path = dir.join("Documents").join("Notes.leaf");
    emit(&app, WelcomeEvent::Create(path.clone()), cx);
    assert!(
        app.read_with(cx, |app, _| app.shell().is_some()),
        "opened straight away"
    );
    assert!(path.join("workspace.sqlite").is_file());
    let last = cx.update(|_, cx| settings::get(cx).last_workspace());
    assert_eq!(
        last.map(|p| std::fs::canonicalize(p).unwrap()),
        Some(std::fs::canonicalize(&path).unwrap())
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn switching_closes_the_workspace_so_it_can_open_again(cx: &mut TestAppContext) {
    cx.update(|cx| crate::init(None, cx));
    let dir = temp_dir("switch");
    let work = dir.join("Work.leaf");
    make_workspace(&work);
    let (app, cx) = launch(Some(work.clone()), &dir, cx);
    assert!(app.read_with(cx, |app, _| app.shell().is_some()));
    cx.update(|window, cx| window.dispatch_action(Box::new(SwitchWorkspace), cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(showing_welcome(&app, cx));
    cx.update(|window, cx| window.dispatch_action(Box::new(OpenRecent { path: work.clone() }), cx));
    assert!(
        app.read_with(cx, |app, _| app.shell().is_some()),
        "the folder was released, so it opens again: {:?}",
        message(&app, cx)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn a_workspace_open_elsewhere_is_explained(cx: &mut TestAppContext) {
    cx.update(|cx| crate::init(None, cx));
    let dir = temp_dir("busy");
    let work = dir.join("Work.leaf");
    make_workspace(&work);
    let _held = leaf_store::Workspace::open(&work).unwrap(); // another Leaf has it
    let (app, cx) = launch(Some(work), &dir, cx);
    assert!(showing_welcome(&app, cx));
    assert_eq!(
        message(&app, cx).as_deref(),
        Some("“Work” is already open in another Leaf window.")
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn picking_a_folder_that_is_not_a_workspace_explains_or_offers_to_create(cx: &mut TestAppContext) {
    cx.update(|cx| crate::init(None, cx));
    let dir = temp_dir("pick");
    let other = dir.join("Photos");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("cat.jpg"), "meow").unwrap();
    let empty = dir.join("Fresh");
    std::fs::create_dir_all(&empty).unwrap();
    let (app, cx) = launch(None, &dir, cx);
    let welcome = app.read_with(cx, |app, _| app.welcome().unwrap().clone());

    cx.update(|_, cx| welcome.update(cx, |w, cx| w.open_folder(other.clone(), cx)));
    let text = message(&app, cx).unwrap();
    assert!(text.starts_with("“Photos” isn't a Leaf workspace."), "{text}");

    cx.update(|_, cx| welcome.update(cx, |w, cx| w.open_folder(empty.clone(), cx)));
    let offer = welcome.read_with(cx, |w, _| w.message.clone().unwrap());
    assert_eq!(offer.create_here.as_deref(), Some(empty.as_path()));
    emit(&app, WelcomeEvent::Create(empty.clone()), cx);
    assert!(
        app.read_with(cx, |app, _| app.shell().is_some()),
        "created in the empty folder and opened"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn the_sidebar_workspace_menu_switches_workspace(cx: &mut TestAppContext) {
    cx.update(|cx| crate::init(None, cx));
    let dir = temp_dir("sidebar-menu");
    let (work, home) = (dir.join("Work.leaf"), dir.join("Home.leaf"));
    make_workspace(&work);
    make_workspace(&home);
    cx.update(|cx| settings::update(cx, |s| s.remember_workspace(&home, 1)));
    let (app, cx) = launch(Some(work.clone()), &dir, cx);
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let switcher = cx
        .debug_bounds("workspace-switcher")
        .expect("the name is a button")
        .center();
    cx.simulate_click(switcher, gpui::Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    // The first Down lands on the "Workspaces" heading, the next on Work (this one, ticked), then Home.
    cx.simulate_keystrokes("down down down enter");
    cx.run_until_parked();
    let open = app.read_with(cx, |app, _| app.open_path.clone());
    // Compare resolved paths: on macOS the temp folder /var/... is a link to /private/var/...
    assert_eq!(
        open.map(|p| std::fs::canonicalize(p).unwrap()),
        Some(std::fs::canonicalize(&home).unwrap()),
        "{:?}",
        message(&app, cx)
    );
    let _ = std::fs::remove_dir_all(dir);
}
