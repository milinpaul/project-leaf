//! E10-S02: a copied page link pastes as a link to the same page, through the clipboard's JSON too; in
//! code it is left out, never pasted as a bare placeholder.

use leaf_core::PageId;
use leaf_model::{BlockKind, Marks, PAGE_LINK, PageCommand, PageDoc, PageOutcome, TextSpan};

use super::super::Fragment;
use super::{at, outline, plain};
use crate::PageEditor;

fn linked(page: PageId) -> TextSpan {
    TextSpan {
        text: PAGE_LINK.to_string(),
        marks: Marks {
            page: Some(page),
            ..Marks::default()
        },
    }
}

/// A paragraph "a", a link to `target`, "b"; then an empty paragraph and an empty code block.
fn page_with_link(target: PageId) -> (PageEditor, Vec<leaf_core::BlockId>) {
    let mut doc = PageDoc::new();
    let mut ids = Vec::new();
    for (kind, text) in [
        (BlockKind::Paragraph, "ab"),
        (BlockKind::Paragraph, ""),
        (
            BlockKind::Code {
                language: String::new(),
            },
            "",
        ),
    ] {
        let index = ids.len();
        let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
            parent: None,
            index,
            kind,
            text: text.into(),
        }) else {
            panic!("insert failed");
        };
        ids.push(id);
    }
    doc.apply(PageCommand::InsertPageLink {
        id: ids[0],
        pos: 1,
        page: target,
    })
    .unwrap();
    doc.set_undo_merge_interval(0);
    (PageEditor::new(doc), ids)
}

#[test]
fn a_copied_page_link_pastes_as_a_link_to_the_same_page() {
    let target = PageId::new();
    let (mut editor, ids) = page_with_link(target);
    editor.select(ids[0], 0..3).unwrap();
    let copied = editor.copy().unwrap().unwrap();
    let fragment = Fragment::from_json(&copied.to_json()).unwrap();

    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.paste(&fragment).unwrap();
    assert_eq!(
        outline(&editor)[1].2,
        vec![plain("a"), linked(target), plain("b")]
    );
    assert_eq!(editor.doc().page_links().len(), 2);

    // Code takes no links: the words paste, the link doesn't.
    editor.set_caret(at(ids[2], 0)).unwrap();
    editor.paste(&fragment).unwrap();
    assert_eq!(outline(&editor)[2].2, vec![plain("ab")]);
    assert_eq!(editor.doc().check_invariants(), Ok(()));
}
