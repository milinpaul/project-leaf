//! Tests for what a link's preview card shows (E10-S12).

use leaf_core::{BlockId, PageId};
use leaf_model::{BlockKind, CalloutTone, PageCommand, PageDoc, PageOutcome, Workspace};

use super::super::WorkspaceEditor;
use super::{LINE_CHARS, PREVIEW_LINES, PreviewKind, PreviewLine};

const NOW: i64 = 1_700_000_000_000;

/// Adds a block of `kind` with `text` under `parent` (top level when `None`), at the end.
fn add(doc: &mut PageDoc, parent: Option<BlockId>, kind: BlockKind, text: &str) -> BlockId {
    let index = match parent {
        Some(parent) => doc.children(parent).unwrap().len(),
        None => doc.root_blocks().len(),
    };
    match doc
        .apply(PageCommand::InsertBlock {
            parent,
            index,
            kind,
            text: text.into(),
        })
        .unwrap()
    {
        PageOutcome::BlockCreated(id) => id,
        other => panic!("expected a new block, got {other:?}"),
    }
}

fn line(kind: PreviewKind, text: &str, depth: usize) -> PreviewLine {
    PreviewLine {
        kind,
        text: text.into(),
        depth,
        linked: false,
    }
}

/// A workspace with "Trips" › "Japan" (🗾), Japan's blocks loaded from `doc`.
fn japan(doc: PageDoc) -> (WorkspaceEditor, PageId) {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let trips = editor.new_page(NOW).unwrap();
    editor.rename(trips, "Trips").unwrap();
    let japan = editor.new_page_in(Some(trips), NOW).unwrap();
    editor.rename(japan, "Japan").unwrap();
    editor.set_icon(japan, Some("🗾")).unwrap();
    editor.insert_page(japan, doc).unwrap();
    (editor, japan)
}

#[test]
fn a_page_shows_its_path_title_and_first_blocks_in_reading_order() {
    let mut doc = PageDoc::new();
    add(&mut doc, None, BlockKind::Heading { level: 1 }, "Plan");
    add(&mut doc, None, BlockKind::Paragraph, "");
    let list = add(&mut doc, None, BlockKind::BulletedList, "Kyoto");
    add(
        &mut doc,
        Some(list),
        BlockKind::Todo { checked: true },
        "Book   the ryokan",
    );
    add(&mut doc, None, BlockKind::NumberedList, "Fly");
    add(&mut doc, None, BlockKind::NumberedList, "Train");
    add(&mut doc, None, BlockKind::Quote, "Go slow");
    add(
        &mut doc,
        None,
        BlockKind::Code {
            language: String::new(),
        },
        "let x = 1;",
    );
    let (editor, page) = japan(doc);

    let preview = editor.page_preview(page, None).unwrap();
    assert_eq!(preview.path, vec!["Trips".to_owned()]);
    assert_eq!(
        (preview.icon.as_deref(), preview.title.as_str()),
        (Some("🗾"), "Japan")
    );
    assert!(preview.loaded && !preview.in_trash);
    assert_eq!(
        preview.lines,
        vec![
            line(PreviewKind::Heading(1), "Plan", 0),
            line(PreviewKind::Bullet, "Kyoto", 0),
            line(PreviewKind::Todo(true), "Book the ryokan", 1),
            line(PreviewKind::Number(1), "Fly", 0),
            line(PreviewKind::Number(2), "Train", 0),
            line(PreviewKind::Quote, "Go slow", 0),
        ],
        "empty paragraphs are left out; at most {PREVIEW_LINES} lines"
    );
    assert!(preview.more, "the code block is past the cut");
}

#[test]
fn other_kinds_get_a_line_that_says_what_they_are() {
    let mut doc = PageDoc::new();
    add(&mut doc, None, BlockKind::Divider, "");
    add(
        &mut doc,
        None,
        BlockKind::Callout {
            tone: CalloutTone::Warning,
        },
        "Careful",
    );
    let table = match doc
        .apply(PageCommand::InsertTable {
            parent: None,
            index: 2,
            rows: 2,
            columns: 2,
            header_row: true,
        })
        .unwrap()
    {
        PageOutcome::BlockCreated(id) => id,
        other => panic!("expected a table, got {other:?}"),
    };
    let cells = doc.table_cells(table).unwrap();
    doc.apply(PageCommand::InsertText {
        id: cells[0][0],
        pos: 0,
        text: "Day".into(),
    })
    .unwrap();
    doc.apply(PageCommand::InsertText {
        id: cells[0][1],
        pos: 0,
        text: "City".into(),
    })
    .unwrap();
    doc.apply(PageCommand::InsertText {
        id: cells[1][0],
        pos: 0,
        text: "Monday".into(),
    })
    .unwrap();
    let (editor, page) = japan(doc);
    let lines = editor.page_preview(page, None).unwrap().lines;
    assert_eq!(
        lines,
        vec![
            line(PreviewKind::Divider, "", 0),
            line(PreviewKind::Callout(CalloutTone::Warning), "Careful", 0),
            line(PreviewKind::Table, "Day · City", 0),
        ],
        "a table is one line, its first row"
    );
}

#[test]
fn links_read_as_their_pages_titles_and_long_words_are_cut() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let hotel = editor.new_page(NOW).unwrap();
    editor.rename(hotel, "Hotel").unwrap();
    let mut doc = PageDoc::new();
    let block = add(&mut doc, None, BlockKind::Paragraph, "Stay at  ");
    doc.apply(PageCommand::InsertPageLink {
        id: block,
        pos: 8,
        page: hotel,
    })
    .unwrap();
    add(&mut doc, None, BlockKind::Paragraph, &"word ".repeat(60));
    let page = editor.new_page(NOW).unwrap();
    editor.insert_page(page, doc).unwrap();

    let lines = editor.page_preview(page, None).unwrap().lines;
    assert_eq!(lines[0].text, "Stay at Hotel");
    assert!(lines[1].text.chars().count() <= LINE_CHARS + 1);
    assert!(lines[1].text.ends_with("word…"));
}

#[test]
fn a_link_to_a_block_shows_it_highlighted_among_its_neighbours() {
    let mut doc = PageDoc::new();
    let blocks: Vec<BlockId> = (1..=10)
        .map(|n| add(&mut doc, None, BlockKind::Paragraph, &format!("line {n}")))
        .collect();
    let (editor, page) = japan(doc);

    let preview = editor.page_preview(page, Some(blocks[6])).unwrap();
    assert_eq!(preview.block, Some(blocks[6]));
    let texts: Vec<(&str, bool)> = preview
        .lines
        .iter()
        .map(|line| (line.text.as_str(), line.linked))
        .collect();
    assert_eq!(
        texts,
        vec![
            ("line 6", false),
            ("line 7", true),
            ("line 8", false),
            ("line 9", false),
            ("line 10", false),
        ],
        "one line above, then on to the end"
    );
    assert!(!preview.more);

    let gone = editor.page_preview(page, Some(BlockId::new())).unwrap();
    assert_eq!(
        gone.block, None,
        "a block that's gone: the card opens the page at the top"
    );
    assert_eq!(gone.lines[0].text, "line 1");
}

#[test]
fn an_empty_linked_block_still_shows_where_it_is() {
    let mut doc = PageDoc::new();
    add(&mut doc, None, BlockKind::Paragraph, "before");
    let empty = add(&mut doc, None, BlockKind::Paragraph, "");
    let (editor, page) = japan(doc);
    let lines = editor.page_preview(page, Some(empty)).unwrap().lines;
    assert_eq!(lines.len(), 2);
    assert!(lines[1].linked && lines[1].text.is_empty());
}

#[test]
fn trash_deleted_and_unloaded_pages() {
    let (mut editor, page) = japan(PageDoc::new());
    let unloaded = editor.new_page(NOW).unwrap();
    let preview = editor.page_preview(unloaded, None).unwrap();
    assert!(
        !preview.loaded && preview.lines.is_empty(),
        "the UI loads it, then asks again"
    );
    assert_eq!(preview.title, "Untitled");
    assert_eq!(preview.path, Vec::<String>::new());

    let fresh = editor.page_preview(page, None).unwrap();
    assert!(
        fresh.loaded && fresh.lines.is_empty() && !fresh.more,
        "an empty page has no lines"
    );

    editor.trash(page, NOW).unwrap();
    let trashed = editor.page_preview(page, None).unwrap();
    assert!(trashed.in_trash && trashed.lines.is_empty());
    assert_eq!(trashed.title, "Japan");

    assert_eq!(
        editor.page_preview(PageId::new(), None),
        None,
        "a deleted page: no card"
    );
}

#[test]
fn deep_nesting_is_drawn_at_most_three_deep_and_numbers_restart() {
    let mut doc = PageDoc::new();
    let mut parent = None;
    for n in 0..5 {
        parent = Some(add(
            &mut doc,
            parent,
            BlockKind::BulletedList,
            &format!("level {n}"),
        ));
    }
    add(&mut doc, None, BlockKind::NumberedList, "one");
    let (editor, page) = japan(doc);
    let preview = editor.page_preview(page, None).unwrap();
    let depths: Vec<usize> = preview.lines.iter().map(|line| line.depth).collect();
    assert_eq!(depths, vec![0, 1, 2, 3, 3, 0]);
    assert_eq!(preview.lines[5].kind, PreviewKind::Number(1));
}

#[test]
fn numbers_restart_after_another_kind_of_block() {
    let mut doc = PageDoc::new();
    add(&mut doc, None, BlockKind::NumberedList, "one");
    add(&mut doc, None, BlockKind::NumberedList, "two");
    add(&mut doc, None, BlockKind::Paragraph, "break");
    add(&mut doc, None, BlockKind::NumberedList, "one again");
    let (editor, page) = japan(doc);
    let kinds: Vec<PreviewKind> = editor
        .page_preview(page, None)
        .unwrap()
        .lines
        .into_iter()
        .map(|line| line.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            PreviewKind::Number(1),
            PreviewKind::Number(2),
            PreviewKind::Text,
            PreviewKind::Number(1),
        ]
    );
}
