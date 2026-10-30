//! E17-S05: copying a block keeps its type, whichever way it is copied, and pasting it on an empty
//! line gives back the same block.

use leaf_model::{BlockKind, Mark};

use super::{at, bold, mark, outline, p, page, plain, texts};
use crate::clipboard::{Fragment, FragmentBlock};

fn heading(text: &str) -> (usize, BlockKind, &str) {
    (0, BlockKind::Heading { level: 2 }, text)
}

#[test]
fn selecting_all_of_a_blocks_text_copies_the_block_with_its_kind() {
    let (mut editor, ids) = page(&[heading("Plans for May"), p(0, "")]);
    mark(&mut editor, ids[0], 10..13, Mark::Bold);
    editor.select(ids[0], 0..13).unwrap();
    let copied = editor.copy().unwrap().unwrap();
    assert_eq!(
        copied,
        Fragment::Blocks(vec![FragmentBlock {
            kind: BlockKind::Heading { level: 2 },
            text: vec![plain("Plans for "), bold("May")],
            children: Vec::new(),
        }])
    );
    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.paste(&copied).unwrap();
    let pasted = &outline(&editor)[1];
    assert_eq!(
        pasted.1,
        BlockKind::Heading { level: 2 },
        "the empty line became the heading"
    );
    assert_eq!(pasted.2, vec![plain("Plans for "), bold("May")], "with its bold");
}

#[test]
fn part_of_a_blocks_text_is_still_copied_as_text() {
    let (mut editor, ids) = page(&[heading("Plans for May")]);
    editor.select(ids[0], 0..5).unwrap();
    assert_eq!(editor.copy().unwrap(), Some(Fragment::Text(vec![plain("Plans")])));
}

#[test]
fn copy_with_only_the_caret_copies_its_block_but_cut_changes_nothing() {
    let (mut editor, ids) = page(&[(0, BlockKind::Todo { checked: true }, "buy milk"), p(0, "")]);
    editor.set_caret(at(ids[0], 3)).unwrap();
    let copied = editor.copy().unwrap();
    assert_eq!(
        copied,
        Some(Fragment::Blocks(vec![FragmentBlock {
            kind: BlockKind::Todo { checked: true },
            text: vec![plain("buy milk")],
            children: Vec::new(),
        }]))
    );
    assert_eq!(editor.cut().unwrap(), None, "cut needs a selection");
    assert_eq!(texts(&editor), vec!["buy milk", ""]);

    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.paste(&copied.unwrap()).unwrap();
    assert_eq!(outline(&editor)[1].1, BlockKind::Todo { checked: true });
}

#[test]
fn a_copied_block_pasted_into_text_goes_in_as_text() {
    let (mut editor, ids) = page(&[heading("May"), p(0, "Plans for !")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    let copied = editor.copy().unwrap().unwrap();
    // Mid-text: just the words.
    editor.set_caret(at(ids[1], 10)).unwrap();
    editor.paste(&copied).unwrap();
    assert_eq!(texts(&editor), vec!["May", "Plans for May!"]);
    assert_eq!(
        outline(&editor)[1].1,
        BlockKind::Paragraph,
        "the line keeps its own kind"
    );
    // At the start of a line that has text: still just the words, the line stays a paragraph.
    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.paste(&copied).unwrap();
    assert_eq!(texts(&editor), vec!["May", "MayPlans for May!"]);
    assert_eq!(outline(&editor)[1].1, BlockKind::Paragraph);
}

#[test]
fn copying_through_the_json_clipboard_keeps_the_block() {
    let (mut editor, ids) = page(&[(0, BlockKind::Quote, "to be"), p(0, "")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    let json = editor.copy().unwrap().unwrap().to_json();
    let back = Fragment::from_json(&json).expect("Leaf reads its own copy");
    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.paste(&back).unwrap();
    assert_eq!(outline(&editor)[1].1, BlockKind::Quote);
    assert_eq!(texts(&editor), vec!["to be", "to be"]);
}

#[test]
fn without_the_leaf_fragment_the_markdown_still_brings_the_kind_and_marks() {
    let (mut editor, ids) = page(&[heading("Plans for May"), p(0, "")]);
    mark(&mut editor, ids[0], 10..13, Mark::Bold);
    editor.set_caret(at(ids[0], 0)).unwrap();
    let markdown = editor.copy().unwrap().unwrap().to_markdown();
    assert_eq!(markdown, "## Plans for **May**");
    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.paste_text(&markdown).unwrap();
    let pasted = &outline(&editor)[1];
    assert_eq!(pasted.1, BlockKind::Heading { level: 2 });
    assert_eq!(pasted.2, vec![plain("Plans for "), bold("May")]);
}
