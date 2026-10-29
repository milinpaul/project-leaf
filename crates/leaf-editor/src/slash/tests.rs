//! Tests for the slash menu (E05-S02): opening and closing, the fuzzy filter's ranking, keyboard
//! navigation, and applying an entry.

use leaf_core::{AssetId, BlockId};
use leaf_model::{BlockKind, Caret, PageCommand, PageDoc, PageOutcome};

use super::items::{all_items, filter};
use crate::{Key, KeyOutcome, Motion, PageEditor, Vertical};

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

fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

/// Types `keys` one character at a time, like a keyboard.
fn type_keys(editor: &mut PageEditor, keys: &str) {
    for ch in keys.chars() {
        editor.insert_text(&ch.to_string()).unwrap();
    }
}

fn labels(items: &[super::SlashItem]) -> Vec<&'static str> {
    items.iter().map(|item| item.label).collect()
}

fn outline(editor: &PageEditor) -> Vec<(BlockKind, String)> {
    editor
        .rows()
        .unwrap()
        .into_iter()
        .map(|row| (editor.doc().kind(row.id).unwrap(), row.text.unwrap_or_default()))
        .collect()
}

/// A page with one empty paragraph and the caret in it.
fn empty() -> (PageEditor, BlockId) {
    let (mut editor, ids) = page(&[para("")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    (editor, ids[0])
}

// ---- Opening and closing ----

#[test]
fn a_slash_at_the_start_of_a_word_opens_the_menu() {
    let (mut editor, id) = empty();
    assert_eq!(editor.slash_menu().unwrap(), None);
    type_keys(&mut editor, "/");
    let menu = editor.slash_menu().unwrap().unwrap();
    assert_eq!((menu.at, menu.query.as_str(), menu.selected), (at(id, 0), "", 0));
    assert_eq!(menu.items, all_items(), "everything, in menu order");

    // After a space, mid-paragraph.
    let (mut editor, ids) = page(&[para("note ")]);
    editor.set_caret(at(ids[0], 5)).unwrap();
    type_keys(&mut editor, "/h");
    let menu = editor.slash_menu().unwrap().unwrap();
    assert_eq!((menu.at, menu.query.as_str()), (at(ids[0], 5), "h"));
}

#[test]
fn a_slash_inside_a_word_or_in_code_and_captions_is_just_text() {
    let (mut editor, id) = empty();
    type_keys(&mut editor, "and/or");
    assert_eq!(editor.slash_menu().unwrap(), None);
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, "and/or".into())]);
    let _ = id;

    let image = BlockKind::Image {
        asset: AssetId::of(b"img"),
        width: None,
        alt: String::new(),
    };
    let code = BlockKind::Code {
        language: String::new(),
    };
    let (mut editor, ids) = page(&[(code, ""), (image, "")]);
    for id in ids {
        editor.set_caret(at(id, 0)).unwrap();
        type_keys(&mut editor, "/");
        assert_eq!(editor.slash_menu().unwrap(), None);
    }
}

#[test]
fn escape_closes_and_leaves_the_typed_text() {
    let (mut editor, _) = empty();
    type_keys(&mut editor, "/quo");
    editor.slash_close();
    assert_eq!(editor.slash_menu().unwrap(), None);
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, "/quo".into())]);
    type_keys(&mut editor, "te");
    assert_eq!(editor.slash_menu().unwrap(), None, "it stays closed");
}

#[test]
fn a_space_closes_it_only_when_nothing_matches() {
    let (mut editor, _) = empty();
    type_keys(&mut editor, "/to ");
    assert!(
        editor.slash_menu().unwrap().is_some(),
        "\"to\" still matches To-do"
    );
    let (mut editor, _) = empty();
    type_keys(&mut editor, "/zzz ");
    assert_eq!(editor.slash_menu().unwrap(), None);
    type_keys(&mut editor, "h");
    assert_eq!(editor.slash_menu().unwrap(), None);
}

#[test]
fn deleting_the_slash_or_moving_away_closes_it() {
    let (mut editor, id) = empty();
    type_keys(&mut editor, "/");
    editor.press(Key::Backspace).unwrap();
    assert_eq!(editor.slash_menu().unwrap(), None, "the / is gone");

    type_keys(&mut editor, "x /h");
    assert!(editor.slash_menu().unwrap().is_some());
    editor.move_caret(Motion::Left).unwrap();
    assert!(editor.slash_menu().unwrap().is_some(), "still inside the query");
    editor.move_caret(Motion::Left).unwrap();
    assert_eq!(editor.slash_menu().unwrap(), None, "before the /");
    editor.set_caret(at(id, 4)).unwrap();
    assert_eq!(editor.slash_menu().unwrap(), None, "and it doesn't come back");
}

// ---- The filter ----

#[test]
fn the_filter_ranks_name_starts_then_word_starts_then_letters_in_order() {
    let items = all_items();
    assert_eq!(labels(&filter(&items, "")).len(), 10);
    assert_eq!(
        labels(&filter(&items, "h")),
        vec![
            "Heading 1",
            "Heading 2",
            "Heading 3",
            "Divider",
            "To-do list",
            "Text"
        ],
        "names starting with h; then the divider's \"hr\" keyword; then h somewhere in a keyword, \
         closest first (\"checkbox\" before \"paragraph\")"
    );
    assert_eq!(
        labels(&filter(&items, "list")),
        vec!["Bulleted list", "Numbered list", "To-do list"]
    );
    assert_eq!(
        labels(&filter(&items, "QUOTE")),
        vec!["Quote"],
        "case doesn't matter"
    );
    assert_eq!(
        labels(&filter(&items, "2")),
        vec!["Heading 2"],
        "a word inside the name"
    );
    assert_eq!(labels(&filter(&items, "check")), vec!["To-do list"], "a keyword");
    assert_eq!(labels(&filter(&items, "xyz")), Vec::<&str>::new());
    // Letters in order, across words.
    assert_eq!(labels(&filter(&items, "nl"))[0], "Numbered list");
    // Fewer gaps rank higher: "hd" is tighter in "Heading" than "Divider"'s keywords.
    let hd = labels(&filter(&items, "hd"));
    assert_eq!(&hd[..3], ["Heading 1", "Heading 2", "Heading 3"]);
    assert_eq!(
        labels(&filter(&items, "  co ")),
        vec!["Code", "To-do list", "Quote"],
        "spaces around the query are ignored; \"checkbox\" and \"citation\" match loosely"
    );
}

#[test]
fn the_menu_filters_as_the_query_grows() {
    let (mut editor, _) = empty();
    type_keys(&mut editor, "/bu");
    let menu = editor.slash_menu().unwrap().unwrap();
    assert_eq!(menu.query, "bu");
    assert_eq!(labels(&menu.items)[0], "Bulleted list");
}

// ---- Navigation ----

#[test]
fn up_and_down_move_the_highlight_and_wrap() {
    let (mut editor, _) = empty();
    type_keys(&mut editor, "/list");
    let down = editor.slash_move(Vertical::Down).unwrap().unwrap();
    assert_eq!(down.selected, 1);
    editor.slash_move(Vertical::Down).unwrap();
    assert_eq!(
        editor.slash_move(Vertical::Down).unwrap().unwrap().selected,
        0,
        "wraps to the top"
    );
    assert_eq!(
        editor.slash_move(Vertical::Up).unwrap().unwrap().selected,
        2,
        "and to the bottom"
    );
    // A new query starts at the top again.
    type_keys(&mut editor, " ");
    assert_eq!(editor.slash_menu().unwrap().unwrap().selected, 0);

    let (mut editor, _) = empty();
    type_keys(&mut editor, "/zzz");
    assert_eq!(
        editor.slash_move(Vertical::Down).unwrap().unwrap().selected,
        0,
        "nothing to move to"
    );
    editor.slash_close();
    assert_eq!(editor.slash_move(Vertical::Down).unwrap(), None);
}

// ---- Applying ----

#[test]
fn applying_in_an_empty_block_converts_it_and_removes_the_query() {
    let (mut editor, id) = empty();
    type_keys(&mut editor, "/head");
    editor.slash_move(Vertical::Down).unwrap();
    assert_eq!(editor.slash_apply(None).unwrap(), Some(at(id, 0)));
    assert_eq!(
        outline(&editor),
        vec![(BlockKind::Heading { level: 2 }, String::new())]
    );
    assert_eq!(editor.slash_menu().unwrap(), None);
}

#[test]
fn applying_after_text_inserts_the_new_block_below() {
    let (mut editor, ids) = page(&[para("Shopping "), para("next")]);
    editor.set_caret(at(ids[0], 9)).unwrap();
    type_keys(&mut editor, "/todo");
    let caret = editor.slash_apply(None).unwrap().unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            (BlockKind::Paragraph, "Shopping ".into()),
            (BlockKind::Todo { checked: false }, String::new()),
            (BlockKind::Paragraph, "next".into()),
        ]
    );
    assert_eq!(caret.at, 0);
    assert_ne!(caret.block, ids[0], "the caret goes into the new block");
}

#[test]
fn text_after_the_query_stays_where_it_was() {
    let (mut editor, ids) = page(&[para(" tail")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    type_keys(&mut editor, "/quote");
    editor.slash_apply(None).unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            (BlockKind::Paragraph, " tail".into()),
            (BlockKind::Quote, String::new())
        ]
    );
}

#[test]
fn a_divider_never_takes_the_caret() {
    let (mut editor, id) = empty();
    type_keys(&mut editor, "/div");
    let caret = editor.slash_apply(None).unwrap().unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            (BlockKind::Divider, String::new()),
            (BlockKind::Paragraph, String::new())
        ]
    );
    assert_ne!(caret.block, id);

    let (mut editor, ids) = page(&[para("above ")]);
    editor.set_caret(at(ids[0], 6)).unwrap();
    type_keys(&mut editor, "/---");
    assert_eq!(
        editor.slash_apply(None).unwrap(),
        Some(at(ids[0], 6)),
        "the caret stays"
    );
    assert_eq!(
        outline(&editor),
        vec![
            (BlockKind::Paragraph, "above ".into()),
            (BlockKind::Divider, String::new())
        ]
    );
}

#[test]
fn a_click_applies_any_entry_and_nothing_happens_without_a_menu() {
    let (mut editor, id) = empty();
    assert_eq!(editor.slash_apply(None).unwrap(), None);
    type_keys(&mut editor, "/");
    assert_eq!(editor.slash_apply(Some(99)).unwrap(), None, "no such entry");
    assert_eq!(editor.slash_apply(Some(7)).unwrap(), Some(at(id, 0)));
    assert_eq!(outline(&editor), vec![(BlockKind::Quote, String::new())]);
}

#[test]
fn enter_applies_while_the_menu_has_matches_and_splits_otherwise() {
    let (mut editor, id) = empty();
    type_keys(&mut editor, "/code");
    assert_eq!(editor.press(Key::Enter).unwrap(), KeyOutcome::Caret(at(id, 0)));
    assert_eq!(
        outline(&editor),
        vec![(
            BlockKind::Code {
                language: String::new()
            },
            String::new()
        )]
    );
    let (mut editor, _) = empty();
    type_keys(&mut editor, "/zzz");
    editor.press(Key::Enter).unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            (BlockKind::Paragraph, "/zzz".into()),
            (BlockKind::Paragraph, String::new())
        ]
    );
}

#[test]
fn undo_right_after_applying_gives_back_the_typed_query() {
    let (mut editor, id) = empty();
    editor.doc.set_undo_merge_interval(60_000);
    type_keys(&mut editor, "/h1");
    editor.slash_apply(None).unwrap();
    assert!(editor.undo().unwrap());
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, "/h1".into())]);
    assert_eq!(editor.caret().unwrap(), Some(at(id, 3)));
    assert!(editor.redo().unwrap());
    assert_eq!(
        outline(&editor),
        vec![(BlockKind::Heading { level: 1 }, String::new())]
    );
}

#[test]
fn the_mouse_highlights_an_entry_and_a_new_query_resets_it() {
    let (mut editor, _) = empty();
    assert_eq!(editor.slash_highlight(2).unwrap(), None, "no menu, nothing to do");
    type_keys(&mut editor, "/");
    assert_eq!(editor.slash_highlight(3).unwrap().unwrap().selected, 3);
    assert_eq!(
        editor.slash_highlight(99).unwrap().unwrap().selected,
        3,
        "past the end is ignored"
    );
    type_keys(&mut editor, "h");
    assert_eq!(editor.slash_menu().unwrap().unwrap().selected, 0);
}
