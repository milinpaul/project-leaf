//! Tests for the internal clipboard (E05-S05): the versioned JSON fragment, copy and cut, pasting
//! text and blocks (mid-text splits the line), and round trips.

use leaf_core::{AssetId, BlockId};
use leaf_model::{BlockKind, Caret, Mark, Marks, PageCommand, PageDoc, PageOutcome, TextSpan};

use super::{FRAGMENT_VERSION, Fragment, FragmentBlock};
use crate::PageEditor;

pub(super) fn page(blocks: &[(usize, BlockKind, &str)]) -> (PageEditor, Vec<BlockId>) {
    let mut doc = PageDoc::new();
    let mut ids: Vec<BlockId> = Vec::new();
    let mut parents: Vec<BlockId> = Vec::new();
    for (depth, kind, text) in blocks {
        parents.truncate(*depth);
        let parent = parents.last().copied();
        let index = match parent {
            Some(p) => doc.children(p).unwrap().len(),
            None => doc.root_blocks().len(),
        };
        let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
            parent,
            index,
            kind: kind.clone(),
            text: (*text).into(),
        }) else {
            panic!("insert failed");
        };
        parents.push(id);
        ids.push(id);
    }
    let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    doc.set_undo_merge_interval(0);
    (PageEditor::new(doc), ids)
}

pub(super) fn p(depth: usize, text: &str) -> (usize, BlockKind, &str) {
    (depth, BlockKind::Paragraph, text)
}

pub(super) fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

/// (depth, kind, formatted text) per block in reading order: the page without IDs.
pub(super) fn outline(editor: &PageEditor) -> Vec<(usize, BlockKind, Vec<TextSpan>)> {
    editor
        .rows()
        .unwrap()
        .into_iter()
        .map(|row| {
            (
                row.depth,
                editor.doc().kind(row.id).unwrap(),
                editor.doc().text(row.id).unwrap().unwrap_or_default(),
            )
        })
        .collect()
}

pub(super) fn texts(editor: &PageEditor) -> Vec<String> {
    editor
        .rows()
        .unwrap()
        .into_iter()
        .map(|r| r.text.unwrap_or_default())
        .collect()
}

pub(super) fn plain(text: &str) -> TextSpan {
    TextSpan {
        text: text.into(),
        marks: Marks::default(),
    }
}

pub(super) fn bold(text: &str) -> TextSpan {
    TextSpan {
        text: text.into(),
        marks: Marks {
            bold: true,
            ..Marks::default()
        },
    }
}

pub(super) fn mark(editor: &mut PageEditor, id: BlockId, range: std::ops::Range<usize>, mark: Mark) {
    editor
        .doc
        .apply(PageCommand::SetMark { id, range, mark })
        .unwrap();
}

// ---- The fragment format ----

#[test]
fn fragments_are_versioned_json_that_reads_back_the_same() {
    let blocks = Fragment::Blocks(vec![FragmentBlock {
        kind: BlockKind::Todo { checked: true },
        text: vec![plain("buy "), bold("milk")],
        children: vec![
            FragmentBlock {
                kind: BlockKind::Image {
                    asset: AssetId::of(b"photo"),
                    width: Some(320.0),
                    alt: "a cow".into(),
                },
                text: vec![plain("caption")],
                children: Vec::new(),
            },
            FragmentBlock {
                kind: BlockKind::Code {
                    language: "rust".into(),
                },
                text: vec![plain("fn main() {}")],
                children: Vec::new(),
            },
        ],
    }]);
    let json = blocks.to_json();
    assert!(json.starts_with(&format!("{{\"leaf_fragment\":{FRAGMENT_VERSION},")));
    assert!(json.contains("\"type\":\"todo\""), "{json}");
    assert_eq!(Fragment::from_json(&json), Some(blocks));

    let text = Fragment::Text(vec![TextSpan {
        text: "link".into(),
        marks: Marks {
            italic: true,
            strike: true,
            code: true,
            link: Some("https://leaf.app".into()),
            ..Marks::default()
        },
    }]);
    assert_eq!(Fragment::from_json(&text.to_json()), Some(text));
}

#[test]
fn other_clipboard_content_and_other_versions_are_not_fragments() {
    for json in [
        "plain words",
        "{}",
        r#"{"leaf_fragment": 2, "text": []}"#,
        r#"{"leaf_fragment": 1}"#,
        r#"{"leaf_fragment": 1, "text": [], "blocks": []}"#,
        r#"{"leaf_fragment": 1, "blocks": [{"type": "spreadsheet"}]}"#,
    ] {
        assert_eq!(Fragment::from_json(json), None, "{json}");
    }
}

#[test]
fn blocks_from_newer_versions_are_left_out_of_a_copy() {
    let fragment = Fragment::Blocks(vec![
        FragmentBlock {
            kind: BlockKind::Unknown {
                type_name: "kanban".into(),
            },
            text: Vec::new(),
            children: Vec::new(),
        },
        FragmentBlock {
            kind: BlockKind::Divider,
            text: Vec::new(),
            children: Vec::new(),
        },
    ]);
    let Some(Fragment::Blocks(read)) = Fragment::from_json(&fragment.to_json()) else {
        panic!("not blocks");
    };
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].kind, BlockKind::Divider);
}

// ---- Copy and cut ----

#[test]
fn copying_text_inside_a_block_keeps_its_formatting() {
    let (mut editor, ids) = page(&[p(0, "make it bold now")]);
    mark(&mut editor, ids[0], 8..12, Mark::Bold);
    assert_eq!(editor.copy().unwrap(), None, "nothing selected");
    editor.select(ids[0], 5..12).unwrap();
    assert_eq!(
        editor.copy().unwrap(),
        Some(Fragment::Text(vec![plain("it "), bold("bold")]))
    );
}

#[test]
fn copying_across_blocks_trims_the_ends_and_keeps_nesting() {
    let (mut editor, ids) = page(&[
        p(1, "deep start"), // nested under nothing visible: depth comes from the page below
        p(0, "middle"),
        p(1, "child"),
        p(0, "end text"),
    ]);
    editor.select_between(at(ids[0], 5), at(ids[3], 3)).unwrap();
    let Some(Fragment::Blocks(blocks)) = editor.copy().unwrap() else {
        panic!("blocks expected");
    };
    let shape: Vec<(String, usize)> = blocks
        .iter()
        .map(|b| (b.text.iter().map(|s| s.text.as_str()).collect(), b.children.len()))
        .collect();
    assert_eq!(
        shape,
        vec![("start".into(), 0), ("middle".into(), 1), ("end".into(), 0)]
    );
    assert_eq!(blocks[1].children[0].text, vec![plain("child")]);
}

#[test]
fn copying_selected_blocks_takes_them_whole_with_children() {
    let (mut editor, ids) = page(&[
        (0, BlockKind::BulletedList, "a"),
        (1, BlockKind::BulletedList, "b"),
        p(0, "c"),
    ]);
    editor.select_blocks(&[ids[0]]).unwrap();
    let Some(Fragment::Blocks(blocks)) = editor.copy().unwrap() else {
        panic!("blocks expected");
    };
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].children.len(), 1);
}

#[test]
fn cut_copies_then_deletes_in_one_step() {
    let (mut editor, ids) = page(&[p(0, "keep cut keep")]);
    editor.select(ids[0], 5..9).unwrap();
    assert_eq!(editor.cut().unwrap(), Some(Fragment::Text(vec![plain("cut ")])));
    assert_eq!(texts(&editor), vec!["keep keep"]);
    editor.undo().unwrap();
    assert_eq!(texts(&editor), vec!["keep cut keep"]);

    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor.select_blocks(&[ids[1]]).unwrap();
    assert!(editor.cut().unwrap().is_some());
    assert_eq!(texts(&editor), vec!["a"]);
    assert_eq!(PageEditor::new(PageDoc::new()).cut().unwrap(), None);
}

// ---- Paste ----

#[test]
fn pasting_text_goes_in_at_the_caret_with_exactly_its_formatting() {
    let (mut editor, ids) = page(&[p(0, "ab")]);
    // Typing after bold would be bold; pasted plain text stays plain.
    mark(&mut editor, ids[0], 0..1, Mark::Bold);
    editor.set_caret(at(ids[0], 1)).unwrap();
    let caret = editor
        .paste(&Fragment::Text(vec![plain("x"), bold("Y")]))
        .unwrap();
    assert_eq!(caret, Some(at(ids[0], 3)));
    assert_eq!(
        outline(&editor)[0].2,
        vec![bold("a"), plain("x"), bold("Y"), plain("b")]
    );
    editor.undo().unwrap();
    assert_eq!(texts(&editor), vec!["ab"], "one undo step");
}

#[test]
fn pasting_replaces_the_selection_even_across_blocks() {
    let (mut editor, ids) = page(&[p(0, "one"), p(0, "two")]);
    editor.select_between(at(ids[0], 1), at(ids[1], 2)).unwrap();
    editor.paste(&Fragment::Text(vec![plain("-")])).unwrap();
    assert_eq!(texts(&editor), vec!["o-o"]);
    editor.undo().unwrap();
    assert_eq!(
        texts(&editor),
        vec!["one", "two"],
        "delete and paste are one step"
    );
}

#[test]
fn pasting_blocks_mid_text_splits_the_line_around_them() {
    let (mut editor, ids) = page(&[p(0, "before|after")]);
    mark(&mut editor, ids[0], 7..12, Mark::Italic);
    editor.set_caret(at(ids[0], 6)).unwrap();
    let fragment = Fragment::Blocks(vec![
        FragmentBlock {
            kind: BlockKind::Heading { level: 1 },
            text: vec![plain("A")],
            children: Vec::new(),
        },
        FragmentBlock {
            kind: BlockKind::BulletedList,
            text: vec![plain("B")],
            children: vec![FragmentBlock {
                kind: BlockKind::BulletedList,
                text: vec![plain("B1")],
                children: Vec::new(),
            }],
        },
    ]);
    let caret = editor.paste(&fragment).unwrap().unwrap();
    let italic = TextSpan {
        text: "after".into(),
        marks: Marks {
            italic: true,
            ..Marks::default()
        },
    };
    assert_eq!(
        outline(&editor),
        vec![
            (0, BlockKind::Paragraph, vec![plain("beforeA")]),
            (0, BlockKind::BulletedList, vec![plain("B")]),
            (1, BlockKind::BulletedList, vec![plain("B1|"), italic]),
        ],
        "the first block's text joins the line; the rest of the line follows the last pasted text"
    );
    assert_eq!(caret.at, 2, "after the pasted text, before the rest of the line");
    editor.undo().unwrap();
    assert_eq!(texts(&editor), vec!["before|after"]);
}

#[test]
fn copy_then_paste_on_an_empty_line_gives_back_the_same_blocks() {
    let (mut editor, ids) = page(&[
        (0, BlockKind::Todo { checked: true }, "task"),
        (1, BlockKind::Quote, "quoted"),
        (2, BlockKind::Heading { level: 3 }, "deep"),
        (0, BlockKind::NumberedList, "two"),
        p(0, ""),
    ]);
    mark(&mut editor, ids[0], 0..2, Mark::Link("https://leaf.app".into()));
    mark(&mut editor, ids[2], 1..3, Mark::Strike);
    let original = outline(&editor)[..4].to_vec();
    editor.set_caret(at(ids[4], 0)).unwrap();
    editor.select_blocks(&[ids[0], ids[3]]).unwrap();
    let json = editor.copy().unwrap().unwrap().to_json();

    editor.set_caret(at(ids[4], 0)).unwrap();
    editor.paste(&Fragment::from_json(&json).unwrap()).unwrap();
    let after = outline(&editor);
    assert_eq!(&after[..4], &original[..], "untouched");
    assert_eq!(
        &after[4..],
        &original[..],
        "the pasted copy is identical, through JSON"
    );
}

#[test]
fn copy_then_paste_of_text_across_blocks_round_trips() {
    let (mut editor, ids) = page(&[
        p(0, "alpha"),
        (0, BlockKind::Quote, "beta"),
        p(0, "gamma"),
        p(0, ""),
    ]);
    editor.select_between(at(ids[0], 0), at(ids[2], 5)).unwrap();
    let fragment = editor.copy().unwrap().unwrap();
    editor.set_caret(at(ids[3], 0)).unwrap();
    editor.paste(&fragment).unwrap();
    assert_eq!(&outline(&editor)[3..], &outline(&editor)[..3]);
}

#[test]
fn pasted_blocks_ending_in_a_divider_leave_a_paragraph_for_the_rest() {
    let (mut editor, ids) = page(&[p(0, "xy")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    let fragment = Fragment::Blocks(vec![
        FragmentBlock {
            kind: BlockKind::Paragraph,
            text: vec![plain("1")],
            children: Vec::new(),
        },
        FragmentBlock {
            kind: BlockKind::Divider,
            text: Vec::new(),
            children: Vec::new(),
        },
    ]);
    let caret = editor.paste(&fragment).unwrap().unwrap();
    assert_eq!(texts(&editor), vec!["x1", "", "y"]);
    assert_eq!(outline(&editor)[1].1, BlockKind::Divider);
    assert_eq!(caret.at, 0, "the caret goes before the rest of the line");
}

#[test]
fn pasting_into_code_takes_plain_text_and_selected_blocks_paste_after() {
    let code = BlockKind::Code {
        language: String::new(),
    };
    let (mut editor, ids) = page(&[(0, code.clone(), "let = 1;")]);
    editor.set_caret(at(ids[0], 4)).unwrap();
    editor.paste(&Fragment::Text(vec![bold("x ")])).unwrap();
    assert_eq!(outline(&editor)[0], (0, code, vec![plain("let x = 1;")]));

    let (mut editor, ids) = page(&[p(0, "a"), p(1, "a child"), p(0, "b")]);
    editor.set_caret(at(ids[2], 0)).unwrap();
    editor.select_blocks(&[ids[0]]).unwrap();
    editor.paste(&Fragment::Text(vec![plain("new")])).unwrap();
    assert_eq!(
        texts(&editor),
        vec!["a", "a child", "new", "b"],
        "after the block and its children"
    );
    assert_eq!(outline(&editor)[2].0, 0);
}

#[test]
fn pasting_after_an_image_caption_and_children_that_cannot_nest() {
    let image = BlockKind::Image {
        asset: AssetId::of(b"img"),
        width: None,
        alt: String::new(),
    };
    let (mut editor, ids) = page(&[(0, image, "cap"), p(0, "end")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    // A heading can't hold children, so its children follow it.
    let fragment = Fragment::Blocks(vec![FragmentBlock {
        kind: BlockKind::Heading { level: 2 },
        text: vec![plain("H")],
        children: vec![FragmentBlock {
            kind: BlockKind::Paragraph,
            text: vec![plain("under")],
            children: Vec::new(),
        }],
    }]);
    editor.paste(&fragment).unwrap();
    assert_eq!(texts(&editor), vec!["cap", "H", "under", "end"]);
    let depths: Vec<usize> = outline(&editor).into_iter().map(|r| r.0).collect();
    assert_eq!(depths, vec![0, 0, 0, 0]);
}

#[test]
fn empty_fragments_and_no_caret_change_nothing() {
    let (mut editor, ids) = page(&[p(0, "a")]);
    assert_eq!(
        editor.paste(&Fragment::Text(vec![plain("x")])).unwrap(),
        None,
        "no caret"
    );
    editor.set_caret(at(ids[0], 1)).unwrap();
    assert_eq!(
        editor.paste(&Fragment::Text(Vec::new())).unwrap(),
        Some(at(ids[0], 1))
    );
    assert_eq!(
        editor.paste(&Fragment::Blocks(Vec::new())).unwrap(),
        Some(at(ids[0], 1))
    );
    assert_eq!(texts(&editor), vec!["a"]);
}

mod page_links;
mod pasted_links;
mod tasks;
mod whole_blocks;
