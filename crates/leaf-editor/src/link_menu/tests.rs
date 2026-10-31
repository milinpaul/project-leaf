//! Tests for the page-link menu's typing side (E10-S05): `[[` opens it, the query follows the caret, and
//! it closes when the text no longer reads as a query.

use leaf_core::{BlockId, PageId};
use leaf_model::{BlockKind, Caret, Marks, PAGE_LINK, PageCommand, PageDoc, PageOutcome, TextSpan};

use crate::{Key, Motion, PageEditor};

fn page(kind: BlockKind, text: &str) -> (PageEditor, BlockId) {
    let mut doc = PageDoc::new();
    let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
        parent: None,
        index: 0,
        kind,
        text: text.into(),
    }) else {
        panic!("insert failed");
    };
    doc.set_undo_merge_interval(0);
    let mut editor = PageEditor::new(doc);
    let len = text.chars().count();
    editor.set_caret(Caret { block: id, at: len }).unwrap();
    (editor, id)
}

fn typed(editor: &mut PageEditor, text: &str) {
    for c in text.chars() {
        editor.insert_text(&c.to_string()).unwrap();
    }
}

#[test]
fn typing_two_brackets_opens_the_menu_and_the_query_follows() {
    let (mut editor, id) = page(BlockKind::Paragraph, "see ");
    typed(&mut editor, "[");
    assert_eq!(
        editor.link_query().unwrap(),
        None,
        "one bracket is just a bracket"
    );
    typed(&mut editor, "[Jap");
    assert_eq!(
        editor.link_query().unwrap(),
        Some((Caret { block: id, at: 4 }, "Jap".into()))
    );
    editor.press(Key::Backspace).unwrap();
    assert_eq!(editor.link_query().unwrap().map(|q| q.1), Some("Ja".into()));
}

#[test]
fn the_menu_closes_when_the_text_stops_being_a_query() {
    // A closing bracket.
    let (mut editor, _) = page(BlockKind::Paragraph, "");
    typed(&mut editor, "[[Japan]");
    assert_eq!(editor.link_query().unwrap(), None);
    // The caret leaving the query.
    let (mut editor, _) = page(BlockKind::Paragraph, "x ");
    typed(&mut editor, "[[Ja");
    editor.move_caret(Motion::LineStart).unwrap();
    assert_eq!(editor.link_query().unwrap(), None);
    // A bracket deleted.
    let (mut editor, _) = page(BlockKind::Paragraph, "");
    typed(&mut editor, "[[");
    editor.press(Key::Backspace).unwrap();
    assert_eq!(editor.link_query().unwrap(), None);
    // Escape keeps the text.
    let (mut editor, id) = page(BlockKind::Paragraph, "");
    typed(&mut editor, "[[Ja");
    editor.link_menu_close();
    assert_eq!(editor.link_query().unwrap(), None);
    assert_eq!(editor.doc().plain_text(id).unwrap().as_deref(), Some("[[Ja"));
}

#[test]
fn code_takes_brackets_literally() {
    let (mut editor, _) = page(
        BlockKind::Code {
            language: String::new(),
        },
        "",
    );
    typed(&mut editor, "[[");
    assert_eq!(editor.link_query().unwrap(), None);
}

#[test]
fn completing_replaces_the_query_with_a_link_in_one_undo_step() {
    let (mut editor, id) = page(BlockKind::Paragraph, "see ");
    typed(&mut editor, "[[Jap");
    let target = PageId::new();
    let caret = editor.complete_link(target).unwrap();
    assert_eq!(caret, Some(Caret { block: id, at: 5 }));
    assert_eq!(
        editor.doc().text(id).unwrap().unwrap(),
        vec![
            TextSpan {
                text: "see ".into(),
                marks: Marks::default()
            },
            TextSpan {
                text: PAGE_LINK.to_string(),
                marks: Marks {
                    page: Some(target),
                    ..Marks::default()
                }
            },
        ]
    );
    assert_eq!(editor.link_query().unwrap(), None, "the menu is closed");
    editor.undo().unwrap();
    assert_eq!(editor.doc().plain_text(id).unwrap().as_deref(), Some("see [[Jap"));
    assert_eq!(
        editor.complete_link(PageId::new()).unwrap(),
        None,
        "no menu, nothing to complete"
    );
}
