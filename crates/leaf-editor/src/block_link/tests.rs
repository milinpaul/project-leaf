//! Tests for copying a link to a block (E10-S08): the menu entry, the copied fragment, and what the copy
//! is named for other apps.

use leaf_core::{BlockId, PageId};
use leaf_model::{Caret, Marks, PAGE_LINK, TextSpan, Workspace};

use crate::block_actions::tests::{p, page};
use crate::{Fragment, LinkState, LinkedBlock, MenuEntry, WorkspaceEditor};

const NOW: i64 = 1_700_000_000_000;

#[test]
fn copy_link_to_block_asks_for_the_menus_block_once_and_changes_nothing() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b")]);
    let before = editor.doc().to_bytes().unwrap();
    editor.open_block_menu(ids[1]).unwrap();
    assert_eq!(
        editor.choose(MenuEntry::CopyLink).unwrap(),
        None,
        "the menu closes"
    );
    assert_eq!(editor.take_link_to_copy(), Some(ids[1]));
    assert_eq!(editor.take_link_to_copy(), None);
    assert_eq!(editor.doc().to_bytes().unwrap(), before);
}

#[test]
fn a_copied_block_link_pastes_as_a_link_and_survives_the_clipboard() {
    let (target_page, target) = (PageId::new(), BlockId::new());
    let fragment = Fragment::block_link(target_page, target);
    let back = Fragment::from_json(&fragment.to_json()).unwrap();
    assert_eq!(back, fragment);

    let (mut editor, ids) = page(&[p(0, "ab")]);
    editor.set_caret(Caret { block: ids[0], at: 1 }).unwrap();
    editor.paste(&back).unwrap();
    assert_eq!(
        editor.doc().text(ids[0]).unwrap().unwrap()[1],
        TextSpan {
            text: PAGE_LINK.to_string(),
            marks: Marks {
                page: Some(target_page),
                block: Some(target),
                ..Marks::default()
            },
        }
    );
    assert_eq!(editor.doc().block_links(), vec![(ids[0], target_page, target)]);
}

/// A workspace with a page "Bookings" (🏨) holding one paragraph, open and loaded.
fn bookings(words: &str) -> (WorkspaceEditor, PageId, BlockId) {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    editor.rename(page, "Bookings").unwrap();
    editor.set_icon(page, Some("🏨")).unwrap();
    editor.select(page).unwrap();
    let open = editor.open_page().unwrap().unwrap();
    let block = open.doc().root_blocks()[0];
    open.set_caret(Caret { block, at: 0 }).unwrap();
    open.insert_text(words).unwrap();
    (editor, page, block)
}

#[test]
fn a_link_to_a_block_reads_its_words_from_the_loaded_page() {
    let (editor, page, block) = bookings("Book the ryokan");
    let target = editor.link_to(page, Some(block));
    assert_eq!(target.block, Some(LinkedBlock::Words("Book the ryokan".into())));
    assert_eq!(target.label(), "🏨 Bookings › Book the ryokan");
    assert_eq!(target.name(), "Bookings › Book the ryokan");
    assert_eq!(editor.link_to(page, None), editor.link_target(page));
    let named =
        Fragment::block_link(page, block).with_link_names(|page, block| editor.link_to(page, block).name());
    assert_eq!(named.to_plain_text(), "Bookings › Book the ryokan");
    assert_eq!(named.to_markdown(), "[[Bookings › Book the ryokan]]");
}

#[test]
fn a_link_to_a_deleted_block_is_greyed_but_opens_and_an_unloaded_page_shows_alone() {
    let (mut editor, page, _) = bookings("Book the ryokan");
    let gone = editor.link_to(page, Some(BlockId::new()));
    assert_eq!(
        (gone.state, gone.block.clone()),
        (LinkState::BlockGone, Some(LinkedBlock::Gone))
    );
    assert_eq!(gone.label(), "🏨 Bookings › deleted block");

    let other = editor.new_page(NOW).unwrap();
    editor.rename(other, "Trip").unwrap();
    let unknown = editor.link_to(other, Some(BlockId::new()));
    assert_eq!(
        (unknown.state, unknown.block.clone()),
        (LinkState::Live, Some(LinkedBlock::Unknown))
    );
    assert_eq!(unknown.label(), "Trip");

    editor.trash(page, NOW).unwrap();
    assert_eq!(
        editor.link_to(page, Some(BlockId::new())).label(),
        "Bookings (in trash)"
    );
    let missing = editor.link_to(PageId::new(), Some(BlockId::new()));
    assert_eq!(missing.label(), "Deleted page");
}

#[test]
fn the_pages_block_links_point_at_are_listed_once_until_loaded() {
    let (mut editor, bookings, block) = bookings("Book the ryokan");
    let trip = editor.new_page(NOW).unwrap();
    let source = editor.new_page(NOW).unwrap();
    editor.select(source).unwrap();
    let open = editor.open_page().unwrap().unwrap();
    let line = open.doc().root_blocks()[0];
    for (at, page) in [(0, trip), (1, trip), (2, bookings), (3, PageId::new())] {
        open.set_caret(Caret { block: line, at }).unwrap();
        open.paste(&Fragment::block_link(page, block)).unwrap();
    }
    // Bookings is loaded already, and a page not in the workspace has nothing to load.
    assert_eq!(editor.block_link_pages_to_load(), vec![trip]);
    editor.insert_page(trip, leaf_model::PageDoc::new()).unwrap();
    assert_eq!(editor.block_link_pages_to_load(), Vec::<PageId>::new());
}
