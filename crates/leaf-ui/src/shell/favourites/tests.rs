//! The sidebar's Favourites section in a (test) window (E18-S05).

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::Root;
use leaf_core::PageId;
use leaf_editor::{MenuPlace, PageAction};

use crate::shell::Shell;

fn open_window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
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

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// Pages "Garden", "Recipes" and "Travel", none open.
fn pages(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> [PageId; 3] {
    let pages = shell.update(cx, |shell, _| {
        let editor = shell.editor_mut();
        ["Garden", "Recipes", "Travel"].map(|title| {
            let page = editor.new_page(0).unwrap();
            editor.rename(page, title).unwrap();
            page
        })
    });
    draw(cx);
    pages
}

fn star(shell: &Entity<Shell>, page: PageId, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, _| {
        shell.editor_mut().set_favourite(page, true).unwrap();
    });
    draw(cx);
}

/// The titles the Favourites section lists, in order.
fn listed(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, _| {
        shell
            .editor()
            .favourites()
            .unwrap()
            .into_iter()
            .map(|item| item.label)
            .collect()
    })
}

fn selected(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<PageId> {
    shell.read_with(cx, |shell, _| shell.editor().selected())
}

#[gpui::test]
fn the_section_shows_above_the_tree_only_while_something_is_starred(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let [garden, recipes, travel] = pages(&shell, cx);
    assert!(
        cx.debug_bounds("favourites").is_none(),
        "hidden with no favourites"
    );

    star(&shell, travel, cx);
    star(&shell, garden, cx);
    let section = cx
        .debug_bounds("favourites")
        .expect("shown once a page is starred");
    let tree = cx.debug_bounds("page-tree-area").unwrap();
    assert!(section.bottom() <= tree.top(), "above the page tree");
    assert_eq!(
        listed(&shell, cx),
        ["Travel", "Garden"],
        "in the order they were starred"
    );
    assert!(cx.debug_bounds("favourite-row-1").is_some());
    assert!(
        cx.debug_bounds("favourite-row-2").is_none(),
        "Recipes isn't starred"
    );

    shell.update(cx, |shell, _| {
        shell.editor_mut().set_favourite(recipes, false).unwrap();
    });
    shell.update(cx, |shell, _| {
        shell.editor_mut().set_favourite(travel, false).unwrap();
    });
    shell.update(cx, |shell, _| {
        shell.editor_mut().set_favourite(garden, false).unwrap();
    });
    draw(cx);
    assert!(
        cx.debug_bounds("favourites").is_none(),
        "hidden again when emptied"
    );
}

#[gpui::test]
fn clicking_a_favourite_opens_it_and_marks_it_open(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let [garden, _, travel] = pages(&shell, cx);
    star(&shell, garden, cx);
    star(&shell, travel, cx);
    let row = cx.debug_bounds("favourite-row-1").unwrap().center();
    cx.simulate_click(row, Modifiers::none());
    draw(cx);
    assert_eq!(selected(&shell, cx), Some(travel));
    let marked: Vec<bool> = shell.read_with(cx, |shell, _| {
        shell
            .editor()
            .favourites()
            .unwrap()
            .iter()
            .map(|item| item.selected)
            .collect()
    });
    assert_eq!(marked, [false, true], "the open page is highlighted");
}

#[gpui::test]
fn favourites_follow_renames_icons_the_trash_and_restoring(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let [garden, recipes, _] = pages(&shell, cx);
    star(&shell, garden, cx);
    star(&shell, recipes, cx);

    shell.update(cx, |shell, _| {
        let editor = shell.editor_mut();
        editor.rename(garden, "Vegetable garden").unwrap();
        editor.set_icon(garden, Some("🌱")).unwrap();
    });
    draw(cx);
    assert_eq!(listed(&shell, cx), ["Vegetable garden", "Recipes"]);
    let icon = shell.read_with(cx, |shell, _| {
        shell.editor().favourites().unwrap()[0].icon.clone()
    });
    assert_eq!(icon.as_deref(), Some("🌱"));

    shell.update(cx, |shell, cx| shell.trash_page(garden, cx));
    draw(cx);
    assert_eq!(listed(&shell, cx), ["Recipes"], "a trashed page leaves the list");
    shell.update(cx, |shell, _| shell.editor_mut().restore(garden).unwrap());
    draw(cx);
    assert_eq!(
        listed(&shell, cx),
        ["Vegetable garden", "Recipes"],
        "and comes back to its place when restored"
    );
}

#[gpui::test]
fn the_menu_removes_a_favourite_and_renames_in_the_favourites_row(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let [garden, recipes, _] = pages(&shell, cx);
    star(&shell, garden, cx);
    star(&shell, recipes, cx);
    let run = |action, page, cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            shell.update(cx, |shell, cx| {
                shell.run_page_action(page, action, MenuPlace::Favourites, window, cx);
            });
        });
        draw(cx);
    };

    run(PageAction::Rename, garden, cx);
    let (here, in_tree) = shell.read_with(cx, |shell, _| {
        (
            shell.rename_field(garden, true).is_some(),
            shell.rename_field(garden, false).is_some(),
        )
    });
    assert!(here && !in_tree, "only the Favourites row becomes a text field");
    cx.simulate_input("Allotment");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(listed(&shell, cx), ["Allotment", "Recipes"]);

    run(PageAction::ToggleFavourite, recipes, cx);
    assert_eq!(listed(&shell, cx), ["Allotment"], "Remove from Favourites");
}
