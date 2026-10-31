//! Tests for the page-link menu as drawn (E10-S05): matching pages, "New page", moving the highlight and
//! linking.

use leaf_core::{BlockId, PageId};
use leaf_model::{Caret, Workspace};

use super::super::WorkspaceEditor;
use super::{LinkChoice, LinkMenu};
use crate::{Key, Vertical};

const NOW: i64 = 1_700_000_000_000;

/// A workspace with "Trip to Japan", "Japanese phrases" (in the trash: "Old Japan"), and an open page
/// "Notes" whose first line has the caret at its end.
fn workspace() -> (WorkspaceEditor, [PageId; 3], BlockId) {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let trip = editor.new_page(NOW).unwrap();
    editor.rename(trip, "Trip to Japan").unwrap();
    editor.set_icon(trip, Some("🗾")).unwrap();
    let phrases = editor.new_page(NOW).unwrap();
    editor.rename(phrases, "Japanese phrases").unwrap();
    let old = editor.new_page(NOW).unwrap();
    editor.rename(old, "Old Japan").unwrap();
    editor.trash(old, NOW).unwrap();
    let notes = editor.new_page(NOW).unwrap();
    editor.rename(notes, "Notes").unwrap();
    let page = editor.open_page().unwrap().unwrap();
    let block = page.ensure_block().unwrap();
    page.set_caret(Caret { block, at: 0 }).unwrap();
    (editor, [trip, phrases, notes], block)
}

fn typed(editor: &mut WorkspaceEditor, text: &str) {
    let page = editor.open_page().unwrap().unwrap();
    for c in text.chars() {
        page.insert_text(&c.to_string()).unwrap();
    }
}

fn labels(menu: &LinkMenu) -> Vec<String> {
    menu.choices
        .iter()
        .map(|choice| match choice {
            LinkChoice::Page(result) => result.label.clone(),
            LinkChoice::NewPage(title) => format!("+ {title}"),
        })
        .collect()
}

#[test]
fn the_menu_lists_matching_pages_then_a_new_page() {
    let (mut editor, [trip, ..], block) = workspace();
    assert_eq!(editor.link_menu().unwrap(), None);
    typed(&mut editor, "[[jap");
    let menu = editor.link_menu().unwrap().unwrap();
    assert_eq!(menu.at, Caret { block, at: 0 });
    assert_eq!(menu.query, "jap");
    assert_eq!(
        labels(&menu),
        vec!["Japanese phrases", "Trip to Japan", "+ jap"],
        "pages in the trash and the open page aren't offered"
    );
    let LinkChoice::Page(first) = &menu.choices[1] else {
        panic!("a page")
    };
    assert_eq!((first.page, first.icon.as_deref()), (trip, Some("🗾")));
}

#[test]
fn an_exact_title_means_no_new_page_and_an_empty_query_lists_pages() {
    let (mut editor, ..) = workspace();
    typed(&mut editor, "[[");
    let menu = editor.link_menu().unwrap().unwrap();
    assert!(menu.choices.iter().all(|c| matches!(c, LinkChoice::Page(_))));
    assert_eq!(menu.choices.len(), 2);
    typed(&mut editor, "trip to japan");
    assert_eq!(
        labels(&editor.link_menu().unwrap().unwrap()),
        vec!["Trip to Japan"]
    );
}

#[test]
fn up_and_down_wrap_and_a_new_query_goes_back_to_the_top() {
    let (mut editor, ..) = workspace();
    typed(&mut editor, "[[jap");
    assert_eq!(
        editor.link_menu_move(Vertical::Down).unwrap().unwrap().selected,
        1
    );
    assert_eq!(
        editor.link_menu_move(Vertical::Down).unwrap().unwrap().selected,
        2
    );
    assert_eq!(
        editor.link_menu_move(Vertical::Down).unwrap().unwrap().selected,
        0
    );
    assert_eq!(editor.link_menu_move(Vertical::Up).unwrap().unwrap().selected, 2);
    assert_eq!(editor.link_menu_highlight(1).unwrap().unwrap().selected, 1);
    typed(&mut editor, "a");
    assert_eq!(editor.link_menu().unwrap().unwrap().selected, 0);
}

#[test]
fn choosing_a_page_links_it_in_place_of_the_query() {
    let (mut editor, [trip, ..], block) = workspace();
    typed(&mut editor, "[[trip");
    let caret = editor.link_menu_apply(None, NOW).unwrap();
    assert_eq!(caret, Some(Caret { block, at: 1 }));
    let page = editor.open_page().unwrap().unwrap();
    assert_eq!(page.doc().page_links(), vec![(block, trip)]);
    assert_eq!(editor.link_menu().unwrap(), None);
}

#[test]
fn new_page_makes_it_inside_the_open_page_and_links_it_without_leaving() {
    let (mut editor, [.., notes], block) = workspace();
    typed(&mut editor, "[[Kyoto temples");
    let menu = editor.link_menu().unwrap().unwrap();
    let new = menu.choices.len() - 1;
    editor.link_menu_apply(Some(new), NOW).unwrap();
    assert_eq!(editor.selected(), Some(notes), "the open page stays open");
    let made = editor.workspace().children(notes).unwrap();
    assert_eq!(made.len(), 1);
    assert_eq!(editor.workspace().page(made[0]).unwrap().title, "Kyoto temples");
    let page = editor.open_page().unwrap().unwrap();
    assert_eq!(page.doc().page_links(), vec![(block, made[0])]);
}

#[test]
fn the_slash_menu_link_to_page_opens_the_menu() {
    let (mut editor, ..) = workspace();
    typed(&mut editor, "/link");
    let page = editor.open_page().unwrap().unwrap();
    let menu = page.slash_menu().unwrap().unwrap();
    assert_eq!(menu.items[0].label, "Link to page");
    page.press(Key::Enter).unwrap();
    let menu = editor.link_menu().unwrap().unwrap();
    assert_eq!(menu.query, "");
    typed(&mut editor, "phr");
    assert_eq!(
        labels(&editor.link_menu().unwrap().unwrap())[0],
        "Japanese phrases"
    );
}
