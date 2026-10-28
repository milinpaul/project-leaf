//! Tests for the page editor's view model and block actions.

use leaf_core::BlockId;
use leaf_model::{BlockKind, Mark, PageCommand, PageDoc};

use super::{BlockRow, BlockStyle, PageEditor};

fn editor() -> (PageEditor, BlockId) {
    let mut editor = PageEditor::new(PageDoc::new());
    let first = editor.ensure_block().unwrap();
    (editor, first)
}

fn styles(editor: &PageEditor) -> Vec<(usize, BlockStyle)> {
    editor
        .rows()
        .unwrap()
        .into_iter()
        .map(|r| (r.depth, r.style))
        .collect()
}

fn text(editor: &PageEditor, id: BlockId) -> String {
    editor.doc().plain_text(id).unwrap().unwrap()
}

#[test]
fn an_empty_page_gets_one_paragraph_and_keeps_it() {
    let (mut editor, first) = editor();
    assert_eq!(
        editor.rows().unwrap(),
        vec![BlockRow {
            id: first,
            depth: 0,
            style: BlockStyle::Paragraph,
            text: Some(String::new())
        }]
    );
    assert_eq!(
        editor.ensure_block().unwrap(),
        first,
        "an existing first block is reused"
    );
    assert_eq!(editor.doc().len(), 1);
}

#[test]
fn set_text_rewrites_only_the_changed_middle_so_marks_survive() {
    let (mut editor, id) = editor();
    editor.set_text(id, "hello world").unwrap();
    editor
        .doc
        .apply(PageCommand::SetMark {
            id,
            range: 0..5,
            mark: Mark::Bold,
        })
        .unwrap();

    editor.set_text(id, "hello brave world").unwrap();
    assert_eq!(text(&editor, id), "hello brave world");
    let spans = editor.doc().text(id).unwrap().unwrap();
    assert_eq!(
        (spans[0].text.as_str(), spans[0].marks.bold),
        ("hello", true),
        "the untouched start keeps bold"
    );

    editor.set_text(id, "👋🏽 hello").unwrap();
    assert_eq!(text(&editor, id), "👋🏽 hello");
    editor.set_text(id, "👋🏽 hello").unwrap();
    editor.set_text(id, "").unwrap();
    assert_eq!(text(&editor, id), "");
}

#[test]
fn set_text_on_a_block_without_text_does_nothing() {
    let (mut editor, id) = editor();
    editor.turn_into(id, BlockKind::Divider).unwrap();
    editor.set_text(id, "ignored").unwrap();
    assert_eq!(editor.doc().plain_text(id).unwrap(), None);
}

#[test]
fn enter_adds_a_block_below_continuing_lists_and_todos() {
    let (mut editor, first) = editor();
    editor.turn_into(first, BlockKind::Heading { level: 1 }).unwrap();
    let para = editor.enter(first).unwrap();
    editor.turn_into(para, BlockKind::BulletedList).unwrap();
    let bullet = editor.enter(para).unwrap();
    editor
        .turn_into(bullet, BlockKind::Todo { checked: true })
        .unwrap();
    let todo = editor.enter(bullet).unwrap();

    assert_eq!(editor.doc().root_blocks(), vec![first, para, bullet, todo]);
    assert_eq!(editor.doc().kind(para).unwrap(), BlockKind::BulletedList);
    assert_eq!(
        editor.doc().kind(todo).unwrap(),
        BlockKind::Todo { checked: false },
        "a fresh to-do starts unticked"
    );
    let after_heading = editor.enter(first).unwrap();
    assert_eq!(
        editor.doc().kind(after_heading).unwrap(),
        BlockKind::Paragraph,
        "headings continue as paragraphs"
    );
    assert_eq!(
        editor.doc().root_blocks()[1],
        after_heading,
        "inserted right below, not at the end"
    );
}

#[test]
fn enter_in_a_nested_block_stays_at_its_level() {
    let (mut editor, first) = editor();
    let second = editor.enter(first).unwrap();
    editor.indent(second).unwrap();
    let third = editor.enter(second).unwrap();
    assert_eq!(editor.doc().children(first).unwrap(), vec![second, third]);
}

#[test]
fn numbered_items_are_numbered_per_run_of_siblings() {
    let (mut editor, first) = editor();
    editor.turn_into(first, BlockKind::NumberedList).unwrap();
    let two = editor.enter(first).unwrap();
    let nested = editor.enter(two).unwrap();
    editor.indent(nested).unwrap();
    let three = editor.enter(two).unwrap();
    let break_ = editor.enter(three).unwrap();
    editor.turn_into(break_, BlockKind::Paragraph).unwrap();
    let restart = editor.enter(break_).unwrap();
    editor.turn_into(restart, BlockKind::NumberedList).unwrap();

    assert_eq!(
        styles(&editor),
        vec![
            (0, BlockStyle::Numbered(1)),
            (0, BlockStyle::Numbered(2)),
            (1, BlockStyle::Numbered(1)),
            (0, BlockStyle::Numbered(3)),
            (0, BlockStyle::Paragraph),
            (0, BlockStyle::Numbered(1)),
        ]
    );
}

#[test]
fn every_kind_has_a_style() {
    let (mut editor, first) = editor();
    let asset = leaf_core::AssetId::of(b"img");
    let kinds = [
        BlockKind::Heading { level: 2 },
        BlockKind::Todo { checked: true },
        BlockKind::Quote,
        BlockKind::Code {
            language: "rust".into(),
        },
        BlockKind::Image {
            asset,
            width: None,
            alt: "A leaf".into(),
        },
    ];
    let mut last = first;
    for kind in kinds {
        last = editor.enter(last).unwrap();
        editor.turn_into(last, kind).unwrap();
    }
    let divider = editor.enter(last).unwrap();
    editor.turn_into(divider, BlockKind::Divider).unwrap();
    let got: Vec<BlockStyle> = styles(&editor).into_iter().map(|(_, s)| s).collect();
    assert_eq!(
        got,
        vec![
            BlockStyle::Paragraph,
            BlockStyle::Heading(2),
            BlockStyle::Todo { checked: true },
            BlockStyle::Quote,
            BlockStyle::Code {
                language: "rust".into()
            },
            BlockStyle::Image { alt: "A leaf".into() },
            BlockStyle::Divider,
        ]
    );
}

#[test]
fn backspace_deletes_only_empty_childless_blocks_and_says_where_to_go() {
    let (mut editor, first) = editor();
    assert_eq!(
        editor.backspace_empty(first).unwrap(),
        None,
        "the last block stays"
    );

    editor.set_text(first, "keep").unwrap();
    let second = editor.enter(first).unwrap();
    assert_eq!(
        editor.backspace_empty(second).unwrap(),
        Some(first),
        "focus moves up"
    );
    assert_eq!(editor.doc().root_blocks(), vec![first]);

    assert_eq!(editor.backspace_empty(first).unwrap(), None, "not empty");
    let child = editor.enter(first).unwrap();
    editor.indent(child).unwrap();
    editor.set_text(first, "").unwrap();
    assert_eq!(editor.backspace_empty(first).unwrap(), None, "has children");

    let (mut editor, top) = self::editor();
    let below = editor.enter(top).unwrap();
    editor.set_text(below, "below").unwrap();
    assert_eq!(
        editor.backspace_empty(top).unwrap(),
        Some(below),
        "the first block hands focus down"
    );
}

#[test]
fn indent_outdent_turn_into_and_toggle_go_through_commands() {
    let (mut editor, first) = editor();
    let second = editor.enter(first).unwrap();
    editor.indent(second).unwrap();
    assert_eq!(
        styles(&editor),
        vec![(0, BlockStyle::Paragraph), (1, BlockStyle::Paragraph)]
    );
    editor.outdent(second).unwrap();
    assert_eq!(
        styles(&editor),
        vec![(0, BlockStyle::Paragraph), (0, BlockStyle::Paragraph)]
    );

    editor.toggle_checked(second).unwrap();
    assert_eq!(
        editor.doc().kind(second).unwrap(),
        BlockKind::Paragraph,
        "only to-dos tick"
    );
    editor
        .turn_into(second, BlockKind::Todo { checked: false })
        .unwrap();
    editor.toggle_checked(second).unwrap();
    assert_eq!(
        editor.doc().kind(second).unwrap(),
        BlockKind::Todo { checked: true }
    );
    editor.toggle_checked(second).unwrap();
    assert_eq!(
        editor.doc().kind(second).unwrap(),
        BlockKind::Todo { checked: false }
    );

    editor.set_text(first, "words").unwrap();
    assert!(
        editor.turn_into(first, BlockKind::Divider).is_err(),
        "model rules still apply"
    );
}

#[test]
fn previous_and_next_follow_reading_order() {
    let (mut editor, first) = editor();
    let second = editor.enter(first).unwrap();
    editor.indent(second).unwrap();
    let third = editor.enter(second).unwrap();
    editor.outdent(third).unwrap();

    assert_eq!(editor.previous(first).unwrap(), None);
    assert_eq!(editor.next(first).unwrap(), Some(second), "into the nested block");
    assert_eq!(editor.next(second).unwrap(), Some(third));
    assert_eq!(editor.previous(third).unwrap(), Some(second));
    assert_eq!(editor.next(third).unwrap(), None);
    assert_eq!(
        editor.next(BlockId::new()).unwrap(),
        None,
        "unknown blocks have no neighbours"
    );
}
