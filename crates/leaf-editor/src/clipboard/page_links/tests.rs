//! Tests for page links in the editor (E10-S03): moving and deleting over them, typing and Enter beside
//! them, and the clipboard.

use std::collections::HashMap;

use leaf_core::{BlockId, PageId};
use leaf_model::{BlockKind, Caret, Marks, PAGE_LINK, PageCommand, PageDoc, PageOutcome, TextSpan};

use super::super::tests::{outline, plain};
use crate::{Key, Motion, PageEditor, Selection};

fn linked(page: PageId) -> TextSpan {
    TextSpan {
        text: PAGE_LINK.to_string(),
        marks: Marks {
            page: Some(page),
            ..Marks::default()
        },
    }
}

/// A page with one paragraph "ab" and a link to `target` between the letters.
fn page_with_link(target: PageId) -> (PageEditor, BlockId) {
    let mut doc = PageDoc::new();
    let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
        parent: None,
        index: 0,
        kind: BlockKind::Paragraph,
        text: "ab".into(),
    }) else {
        panic!("insert failed");
    };
    doc.apply(PageCommand::InsertPageLink {
        id,
        pos: 1,
        page: target,
    })
    .unwrap();
    doc.set_undo_merge_interval(0);
    (PageEditor::new(doc), id)
}

fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

#[test]
fn arrows_step_over_a_link_and_shift_selects_it_whole() {
    let (mut editor, id) = page_with_link(PageId::new());
    editor.set_caret(at(id, 1)).unwrap();
    assert_eq!(editor.move_caret(Motion::Right).unwrap(), Some(at(id, 2)));
    assert_eq!(editor.move_caret(Motion::Left).unwrap(), Some(at(id, 1)));
    let selection = editor.extend(Motion::Right).unwrap();
    assert_eq!(
        selection,
        Some(Selection::Text {
            start: at(id, 1),
            end: at(id, 2)
        })
    );
}

#[test]
fn backspace_and_delete_remove_a_link_whole() {
    let target = PageId::new();
    let (mut editor, id) = page_with_link(target);
    editor.set_caret(at(id, 2)).unwrap();
    editor.press(Key::Backspace).unwrap();
    assert_eq!(outline(&editor)[0].2, vec![plain("ab")]);
    let (mut editor, id) = page_with_link(target);
    editor.set_caret(at(id, 1)).unwrap();
    editor.press(Key::Delete).unwrap();
    assert_eq!(outline(&editor)[0].2, vec![plain("ab")]);
}

#[test]
fn typing_and_enter_beside_a_link_leave_it_whole() {
    let target = PageId::new();
    let (mut editor, id) = page_with_link(target);
    editor.set_caret(at(id, 2)).unwrap();
    editor.insert_text("!").unwrap();
    editor.set_caret(at(id, 1)).unwrap();
    editor.insert_text("?").unwrap();
    assert_eq!(
        outline(&editor)[0].2,
        vec![plain("a?"), linked(target), plain("!b")]
    );
    // Enter right after the link: the link stays on the first line.
    editor.set_caret(at(id, 3)).unwrap();
    editor.press(Key::Enter).unwrap();
    let rows = outline(&editor);
    assert_eq!(rows[0].2, vec![plain("a?"), linked(target)]);
    assert_eq!(rows[1].2, vec![plain("!b")]);
    assert_eq!(editor.doc().check_invariants(), Ok(()));
}

#[test]
fn copied_text_names_links_by_title() {
    let target = PageId::new();
    let (mut editor, id) = page_with_link(target);
    editor.select(id, 0..3).unwrap();
    let copied = editor.copy().unwrap().unwrap();
    let named = copied.with_link_names(|page| {
        assert_eq!(page, target);
        "Trip to Japan".into()
    });
    assert_eq!(named.to_plain_text(), "aTrip to Japanb");
    assert_eq!(named.to_markdown(), "a[[Trip to Japan]]b");
    // Leaf's own copy keeps the link itself.
    assert!(copied.to_json().contains(&target.to_string()));
}

#[test]
fn pasted_double_bracket_titles_become_links_when_one_page_has_the_title() {
    let japan = PageId::new();
    let titles: HashMap<String, PageId> = [("Trip to Japan".to_owned(), japan)].into();
    let find = |title: &str| titles.get(title).copied();
    let (mut editor, id) = page_with_link(PageId::new());
    editor.set_caret(at(id, 3)).unwrap();
    editor
        .paste_text_linking(" see [[Trip to Japan]] and [[Nowhere]]", &find)
        .unwrap();
    let text = &outline(&editor)[0].2;
    assert_eq!(text[2], plain("b see "));
    assert_eq!(text[3], linked(japan));
    assert_eq!(text[4], plain(" and [[Nowhere]]"));
    assert_eq!(editor.doc().check_invariants(), Ok(()));
}

#[test]
fn bracket_titles_in_inline_code_stay_text() {
    let japan = PageId::new();
    let find = |title: &str| (title == "Japan").then_some(japan);
    let (mut editor, id) = page_with_link(PageId::new());
    editor.set_caret(at(id, 0)).unwrap();
    editor.paste_text_linking("`[[Japan]]`", &find).unwrap();
    assert_eq!(editor.doc().page_links().len(), 1, "only the link that was there");
}
