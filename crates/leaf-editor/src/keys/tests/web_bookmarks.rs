//! E15-S03: a web bookmark is a whole block, like a divider or an image: the caret passes over it, it is
//! selected, copied, cut, moved and deleted whole, and the block menu swaps it with a link.

use leaf_model::{BlockKind, Mark, PageCommand, WebPreview};

use super::{at, outline, page, press_and_undo, row};
use crate::{BlockAction, Key, KeyOutcome, MenuEntry, Motion};

const LEAF: &str = "https://leaf.app/blog";

fn bookmark() -> BlockKind {
    BlockKind::WebBookmark {
        url: LEAF.into(),
        preview: WebPreview {
            title: Some("The Leaf blog".into()),
            ..WebPreview::default()
        },
    }
}

fn para(text: &str) -> (usize, BlockKind, &str) {
    (0, BlockKind::Paragraph, text)
}

#[test]
fn backspace_below_or_delete_above_selects_the_bookmark_and_deleting_it_moves_the_caret() {
    let (mut editor, ids) = page(&[para("a"), (0, bookmark(), ""), para("b")]);
    assert_eq!(
        press_and_undo(&mut editor, at(ids[2], 0), Key::Backspace),
        KeyOutcome::SelectBlock(ids[1])
    );
    assert_eq!(
        press_and_undo(&mut editor, at(ids[0], 1), Key::Delete),
        KeyOutcome::SelectBlock(ids[1])
    );
    assert_eq!(editor.doc().len(), 3, "selecting changes nothing");
    assert_eq!(
        editor.delete_block(ids[1]).unwrap(),
        Some(at(ids[0], 1)),
        "the caret goes above"
    );
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Paragraph, "a"),
            row(0, BlockKind::Paragraph, "b")
        ]
    );
}

#[test]
fn the_caret_passes_over_a_bookmark_and_enter_on_it_adds_a_line_below() {
    let (mut editor, ids) = page(&[para("a"), (0, bookmark(), ""), para("b")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    editor.move_caret(Motion::Right).unwrap();
    assert_eq!(
        editor.caret().unwrap(),
        Some(at(ids[2], 0)),
        "skipped, like a divider"
    );
    let below = editor.enter_at(ids[1], 0).unwrap();
    assert_eq!(outline(&editor)[2], row(0, BlockKind::Paragraph, ""));
    assert_eq!(below.at, 0);
}

#[test]
fn a_selected_bookmark_is_copied_cut_pasted_and_moved_whole() {
    let (mut editor, ids) = page(&[para("a"), (0, bookmark(), ""), para("b")]);
    editor.select_blocks(&[ids[1]]).unwrap();
    let copied = editor.copy().unwrap().unwrap();
    let cut = editor.cut().unwrap().unwrap();
    assert_eq!(copied, cut);
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Paragraph, "a"),
            row(0, BlockKind::Paragraph, "b")
        ]
    );
    editor.set_caret(at(ids[2], 1)).unwrap();
    editor.paste(&cut).unwrap();
    assert_eq!(
        outline(&editor)[2],
        row(0, bookmark(), ""),
        "the preview came along"
    );

    let (mut editor, ids) = page(&[para("a"), (0, bookmark(), "")]);
    editor.block_action(ids[1], BlockAction::MoveUp).unwrap();
    assert_eq!(outline(&editor)[0], row(0, bookmark(), ""));
    editor.block_action(ids[1], BlockAction::Duplicate).unwrap();
    assert_eq!(outline(&editor)[1], row(0, bookmark(), ""));
}

#[test]
fn the_block_menu_turns_a_bookmark_into_a_link_and_a_lone_link_into_a_bookmark() {
    let (mut editor, ids) = page(&[(0, bookmark(), ""), para("The Leaf blog"), para("words")]);
    editor
        .doc
        .apply(PageCommand::SetMark {
            id: ids[1],
            range: 0..13,
            mark: Mark::Link(LEAF.into()),
        })
        .unwrap();
    let entries = |editor: &mut crate::PageEditor, block| -> Vec<MenuEntry> {
        let menu = editor.open_block_menu(block).unwrap().unwrap();
        menu.items.into_iter().map(|item| item.entry).collect()
    };
    assert!(entries(&mut editor, ids[0]).contains(&MenuEntry::ToLink));
    assert!(entries(&mut editor, ids[1]).contains(&MenuEntry::ToWebBookmark(LEAF.into())));
    let plain = entries(&mut editor, ids[2]);
    assert!(
        !plain.contains(&MenuEntry::ToLink)
            && !plain.iter().any(|e| matches!(e, MenuEntry::ToWebBookmark(_)))
    );

    editor.open_block_menu(ids[0]).unwrap();
    editor.choose(MenuEntry::ToLink).unwrap();
    assert_eq!(outline(&editor)[0], row(0, BlockKind::Paragraph, LEAF));
    assert_eq!(
        editor.doc().text(ids[0]).unwrap().unwrap()[0]
            .marks
            .link
            .as_deref(),
        Some(LEAF)
    );

    editor.open_block_menu(ids[1]).unwrap();
    editor.choose(MenuEntry::ToWebBookmark(LEAF.into())).unwrap();
    assert_eq!(
        outline(&editor)[1],
        row(
            0,
            BlockKind::WebBookmark {
                url: LEAF.into(),
                preview: WebPreview::default()
            },
            ""
        )
    );
    assert!(editor.undo().unwrap());
    assert_eq!(
        outline(&editor)[1],
        row(0, BlockKind::Paragraph, "The Leaf blog"),
        "undo gives the link back"
    );
}

#[test]
fn turning_a_selection_into_a_bookmark_skips_blocks_that_would_lose_words() {
    let (mut editor, ids) = page(&[para(""), para("words")]);
    editor.select_blocks(&ids).unwrap();
    editor
        .block_action(
            ids[0],
            BlockAction::TurnInto(BlockKind::WebBookmark {
                url: LEAF.into(),
                preview: WebPreview::default(),
            }),
        )
        .unwrap();
    assert!(
        matches!(outline(&editor)[0].1, BlockKind::WebBookmark { .. }),
        "the empty one"
    );
    assert_eq!(outline(&editor)[1], row(0, BlockKind::Paragraph, "words"), "kept");
}
