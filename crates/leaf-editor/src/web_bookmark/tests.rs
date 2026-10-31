//! Tests for making web bookmarks in the editor (E15-S03): the offer after pasting a lone address, the
//! slash menu's "Web bookmark" and its address, and undo.

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, PageCommand, PageDoc, PageOutcome, WebPreview};

use super::BOOKMARK_OFFER_CHOICES;
use crate::{Key, PageEditor, Vertical};

const LEAF: &str = "https://leaf.app/blog";

fn page(blocks: &[(BlockKind, &str)]) -> (PageEditor, Vec<BlockId>) {
    let mut doc = PageDoc::new();
    let ids = blocks
        .iter()
        .map(|(kind, text)| {
            let index = doc.root_blocks().len();
            let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
                parent: None,
                index,
                kind: kind.clone(),
                text: (*text).into(),
            }) else {
                panic!("insert failed");
            };
            id
        })
        .collect();
    let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    doc.set_undo_merge_interval(0);
    (PageEditor::new(doc), ids)
}

fn para(text: &str) -> (BlockKind, &str) {
    (BlockKind::Paragraph, text)
}

fn bookmark(url: &str) -> BlockKind {
    BlockKind::WebBookmark {
        url: url.into(),
        preview: WebPreview::default(),
    }
}

fn outline(editor: &PageEditor) -> Vec<(BlockKind, String)> {
    editor
        .rows()
        .unwrap()
        .into_iter()
        .map(|row| (editor.doc().kind(row.id).unwrap(), row.text.unwrap_or_default()))
        .collect()
}

fn type_keys(editor: &mut PageEditor, keys: &str) {
    for ch in keys.chars() {
        editor.insert_text(&ch.to_string()).unwrap();
    }
}

/// An editor with `LEAF` just pasted into the first block at `at`.
fn pasted(text: &str, at: usize) -> (PageEditor, Vec<BlockId>) {
    let (mut editor, ids) = page(&[para(text), para("Next")]);
    editor.set_caret(Caret { block: ids[0], at }).unwrap();
    editor.paste_text(&format!("  {LEAF}\n")).unwrap();
    (editor, ids)
}

#[test]
fn a_pasted_web_address_is_a_link_with_an_offer_under_it() {
    let (mut editor, ids) = pasted("", 0);
    let spans = editor.doc().text(ids[0]).unwrap().unwrap();
    assert_eq!(spans[0].text, LEAF);
    assert_eq!(spans[0].marks.link.as_deref(), Some(LEAF), "a link by default");
    let offer = editor.bookmark_offer().unwrap().unwrap();
    assert_eq!(
        offer.at,
        Caret {
            block: ids[0],
            at: LEAF.len()
        }
    );
    assert_eq!(offer.url, LEAF);
    assert_eq!(
        BOOKMARK_OFFER_CHOICES[offer.selected], "Dismiss",
        "keeping the link comes first"
    );
}

#[test]
fn no_offer_for_other_pastes() {
    for text in ["mailto:me@leaf.app", "some words", "leaf.app and more"] {
        let (mut editor, ids) = page(&[para("")]);
        editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
        editor.paste_text(text).unwrap();
        assert_eq!(editor.bookmark_offer().unwrap(), None, "{text}");
    }
    // Over selected words the address links them: nothing to offer.
    let (mut editor, ids) = page(&[para("the blog")]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    editor.extend(crate::Motion::LineEnd).unwrap();
    editor.paste_text(LEAF).unwrap();
    assert_eq!(editor.bookmark_offer().unwrap(), None);
}

#[test]
fn typing_moving_or_any_other_key_ends_the_offer_and_keeps_the_link() {
    let (mut editor, ids) = pasted("", 0);
    editor.insert_text(" ").unwrap();
    assert_eq!(editor.bookmark_offer().unwrap(), None, "typing");

    let (mut editor, _) = pasted("", 0);
    editor.move_caret(crate::Motion::Left).unwrap();
    assert_eq!(editor.bookmark_offer().unwrap(), None, "moving");

    let (mut editor, _) = pasted("", 0);
    editor.press(Key::Backspace).unwrap();
    assert_eq!(editor.bookmark_offer().unwrap(), None, "another key");

    let (mut editor, _) = pasted("", 0);
    editor.dismiss_bookmark_offer();
    assert_eq!(editor.bookmark_offer().unwrap(), None, "Escape");
    assert_eq!(outline(&editor)[0], (BlockKind::Paragraph, LEAF.into()));
    let _ = ids;
}

#[test]
fn enter_on_dismiss_keeps_the_link_and_adds_no_line() {
    let (mut editor, ids) = pasted("", 0);
    editor.press(Key::Enter).unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            (BlockKind::Paragraph, LEAF.into()),
            (BlockKind::Paragraph, "Next".into())
        ]
    );
    assert_eq!(
        editor.caret().unwrap(),
        Some(Caret {
            block: ids[0],
            at: LEAF.len()
        })
    );
    assert_eq!(editor.bookmark_offer().unwrap(), None);
}

#[test]
fn choosing_the_bookmark_in_an_empty_block_turns_it_into_one_with_a_new_line_below() {
    let (mut editor, ids) = pasted("", 0);
    editor.bookmark_offer_move(Vertical::Down);
    assert_eq!(editor.bookmark_offer().unwrap().unwrap().selected, 1);
    editor.press(Key::Enter).unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            (bookmark(LEAF), String::new()),
            (BlockKind::Paragraph, String::new()),
            (BlockKind::Paragraph, "Next".into())
        ]
    );
    let caret = editor.caret().unwrap().unwrap();
    assert_ne!(caret.block, ids[0], "the caret goes to the new line");
    // One undo step: the link comes back, with the caret after it.
    assert!(editor.undo().unwrap());
    assert_eq!(outline(&editor)[0], (BlockKind::Paragraph, LEAF.into()));
    assert_eq!(
        editor.doc().text(ids[0]).unwrap().unwrap()[0]
            .marks
            .link
            .as_deref(),
        Some(LEAF)
    );
    assert_eq!(outline(&editor).len(), 2);
}

#[test]
fn choosing_the_bookmark_inside_text_takes_the_link_out_and_puts_the_bookmark_below() {
    let (mut editor, ids) = pasted("Read  today", 5);
    editor.bookmark_offer_highlight(1);
    let caret = editor.bookmark_offer_apply(None).unwrap().unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            (BlockKind::Paragraph, "Read  today".into()),
            (bookmark(LEAF), String::new()),
            (BlockKind::Paragraph, "Next".into())
        ]
    );
    assert_eq!(
        caret,
        Caret { block: ids[0], at: 5 },
        "the caret stays where the link was"
    );
    assert!(editor.undo().unwrap());
    assert_eq!(outline(&editor)[0].1, format!("Read {LEAF} today"));
    assert_eq!(outline(&editor).len(), 2);
}

#[test]
fn a_click_on_an_entry_applies_it() {
    let (mut editor, _) = pasted("", 0);
    editor.bookmark_offer_apply(Some(1)).unwrap();
    assert_eq!(outline(&editor)[0].0, bookmark(LEAF));
    // No offer standing: nothing happens.
    assert_eq!(editor.bookmark_offer_apply(Some(1)).unwrap(), None);
}

#[test]
fn the_offer_wraps_around() {
    let (mut editor, _) = pasted("", 0);
    editor.bookmark_offer_move(Vertical::Up);
    assert_eq!(editor.bookmark_offer().unwrap().unwrap().selected, 1);
    editor.bookmark_offer_move(Vertical::Down);
    assert_eq!(editor.bookmark_offer().unwrap().unwrap().selected, 0);
}

#[test]
fn the_slash_menu_finds_web_bookmark_by_bookmark_and_asks_for_an_address() {
    let (mut editor, ids) = page(&[para(""), para("Elsewhere")]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    type_keys(&mut editor, "/bookmark");
    let menu = editor.slash_menu().unwrap().unwrap();
    assert_eq!(menu.items[0].label, "Web bookmark");
    assert!(!editor.take_web_address_request());
    editor.press(Key::Enter).unwrap();
    assert_eq!(
        outline(&editor)[0],
        (BlockKind::Paragraph, String::new()),
        "`/bookmark` is gone"
    );
    assert!(
        editor.take_web_address_request(),
        "the UI is told to show the prompt"
    );
    assert!(!editor.take_web_address_request(), "only once");
    assert_eq!(
        editor.web_address_request_at().unwrap(),
        Some(Caret { block: ids[0], at: 0 })
    );

    // The caret wanders while the prompt is open; the bookmark still goes where it was asked.
    editor.set_caret(Caret { block: ids[1], at: 3 }).unwrap();
    editor.insert_web_bookmark("leaf.app/blog").unwrap().unwrap();
    assert_eq!(
        outline(&editor)[0],
        (bookmark("https://leaf.app/blog"), String::new()),
        "https:// added"
    );
    assert_eq!(
        outline(&editor)[1],
        (BlockKind::Paragraph, String::new()),
        "a new line below for the caret"
    );
    assert_eq!(outline(&editor)[2], (BlockKind::Paragraph, "Elsewhere".into()));
}

#[test]
fn web_bookmark_is_found_by_its_name_too() {
    let (mut editor, ids) = page(&[para("")]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    type_keys(&mut editor, "/web");
    assert_eq!(
        editor.slash_menu().unwrap().unwrap().items[0].label,
        "Web bookmark"
    );
}

#[test]
fn an_address_that_cant_be_a_bookmark_inserts_nothing() {
    for typed in ["", "not an address", "mailto:me@leaf.app", "javascript:alert(1)"] {
        assert_eq!(PageEditor::web_address(typed), None, "{typed}");
    }
    assert_eq!(
        PageEditor::web_address(" https://leaf.app "),
        Some("https://leaf.app".into())
    );
    let (mut editor, ids) = page(&[para("Notes")]);
    editor.set_caret(Caret { block: ids[0], at: 5 }).unwrap();
    assert_eq!(editor.insert_web_bookmark("not an address").unwrap(), None);
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, "Notes".into())]);
}

#[test]
fn next_to_text_the_bookmark_goes_below_and_a_cancelled_prompt_changes_nothing() {
    let (mut editor, ids) = page(&[para("Notes ")]);
    editor.set_caret(Caret { block: ids[0], at: 6 }).unwrap();
    type_keys(&mut editor, "/bookmark");
    editor.slash_apply(None).unwrap();
    assert!(editor.take_web_address_request());
    editor.cancel_web_address();
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, "Notes ".into())]);
    assert_eq!(editor.web_address_request_at().unwrap(), None);
    // Without a request it goes at the caret: below the text, the caret staying put.
    let caret = editor.insert_web_bookmark(LEAF).unwrap().unwrap();
    assert_eq!(caret, Caret { block: ids[0], at: 6 });
    assert_eq!(outline(&editor)[1], (bookmark(LEAF), String::new()));
}

#[test]
fn undo_after_the_slash_bookmark_gives_back_the_typed_query() {
    let (mut editor, ids) = page(&[para("")]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    type_keys(&mut editor, "/bookmark");
    editor.slash_apply(None).unwrap();
    editor.insert_web_bookmark(LEAF).unwrap();
    assert!(editor.undo().unwrap(), "the bookmark goes");
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, String::new())]);
    assert!(editor.undo().unwrap(), "then the query comes back");
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, "/bookmark".into())]);
}
