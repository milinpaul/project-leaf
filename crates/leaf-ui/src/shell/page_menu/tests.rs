//! The page menu in a (test) window (E18-S04): from the top bar's `⋯` and the sidebar.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::{Root, WindowExt as _};
use leaf_core::PageId;
use leaf_editor::{MenuPlace, PageAction};

use crate::shell::Shell;

pub(super) fn open_window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.update(|cx| crate::init(None, cx));
    let made: Rc<RefCell<Option<Entity<Shell>>>> = Rc::default();
    let keep = made.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let shell = cx.new(|cx| Shell::new(window, cx));
        *keep.borrow_mut() = Some(shell.clone());
        Root::new(shell, window, cx)
    });
    draw(cx);
    let shell = made.borrow().clone().unwrap();
    (shell, cx)
}

pub(super) fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// A page "Projects" with "Notes" inside, Notes open.
pub(super) fn pages(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (PageId, PageId) {
    let (projects, notes) = shell.update(cx, |shell, _| {
        let editor = shell.editor_mut();
        let projects = editor.new_page(0).unwrap();
        let notes = editor.new_page_in(Some(projects), 0).unwrap();
        editor.rename(projects, "Projects").unwrap();
        editor.rename(notes, "Notes").unwrap();
        (projects, notes)
    });
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(notes, window, cx)));
    draw(cx);
    (projects, notes)
}

fn run(
    shell: &Entity<Shell>,
    page: PageId,
    action: PageAction,
    place: MenuPlace,
    cx: &mut VisualTestContext,
) {
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.run_page_action(page, action, place, window, cx);
        });
    });
    draw(cx);
}

fn selected(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<PageId> {
    shell.read_with(cx, |shell, _| shell.editor().selected())
}

fn title(shell: &Entity<Shell>, page: PageId, cx: &mut VisualTestContext) -> String {
    shell.read_with(cx, |shell, _| {
        shell.editor().workspace().page(page).unwrap().title
    })
}

#[gpui::test]
fn the_menu_opens_from_the_top_bar_and_the_keyboard_runs_its_first_entry(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (_, notes) = pages(&shell, cx);
    let button = cx.debug_bounds("page-menu").unwrap().center();
    cx.simulate_click(button, Modifiers::none());
    draw(cx);
    // Down to the first entry (Add to Favourites), Enter runs it and closes the menu.
    cx.simulate_keystrokes("down enter");
    draw(cx);
    assert!(shell.read_with(cx, |shell, _| shell.editor().is_favourite(notes)));
    // Opened again, Escape closes it without doing anything.
    cx.simulate_click(button, Modifiers::none());
    draw(cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(shell.read_with(cx, |shell, _| shell.editor().is_favourite(notes)));
}

#[gpui::test]
fn rename_from_the_top_bar_selects_the_title_to_type_over(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (_, notes) = pages(&shell, cx);
    run(&shell, notes, PageAction::Rename, MenuPlace::TopBar, cx);
    cx.simulate_input("Meeting notes");
    draw(cx);
    assert_eq!(title(&shell, notes, cx), "Meeting notes");
}

#[gpui::test]
fn rename_from_the_sidebar_edits_the_row(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (projects, _) = pages(&shell, cx);
    run(&shell, projects, PageAction::Rename, MenuPlace::Sidebar, cx);
    cx.simulate_input("Work");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(title(&shell, projects, cx), "Work");
}

#[gpui::test]
fn move_to_trash_opens_the_parent_and_undo_brings_the_page_back(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (projects, notes) = pages(&shell, cx);
    let notices = |cx: &mut VisualTestContext| cx.update(|window, cx| window.notifications(cx).len());
    run(&shell, notes, PageAction::MoveToTrash, MenuPlace::TopBar, cx);
    let in_trash = |cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            shell.editor().workspace().is_in_trash(notes).unwrap()
        })
    };
    assert!(in_trash(cx));
    assert_eq!(selected(&shell, cx), Some(projects), "its parent opens");
    assert_eq!(notices(cx), 1, "a notice offers to undo");

    assert!(
        cx.debug_bounds("undo-trash").is_some(),
        "the notice has an Undo button"
    );
    // The notice slides in on the real clock, so a simulated click can miss it mid-slide; run what
    // its Undo button runs.
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| shell.undo_trash(notes, Some(notes), window, cx));
    });
    draw(cx);
    assert!(!in_trash(cx), "Undo restores it");
    assert_eq!(selected(&shell, cx), Some(notes), "and opens it again");
}

#[gpui::test]
fn the_sidebar_menu_adds_a_page_inside(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (projects, _) = pages(&shell, cx);
    run(&shell, projects, PageAction::AddInside, MenuPlace::Sidebar, cx);
    let children = shell.read_with(cx, |shell, _| {
        shell.editor().workspace().children(projects).unwrap()
    });
    assert_eq!(children.len(), 2);
}
