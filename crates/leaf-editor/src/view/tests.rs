//! Tests for the incremental page view (E04-S01).

use std::fmt::Write as _;

use leaf_core::{AssetId, BlockId};
use leaf_model::{BlockKind, Mark, PageCommand, PageDoc, PageOutcome};
use proptest::prelude::*;

use super::{BlockRow, BlockStyle, PageView};

fn add(page: &mut PageDoc, parent: Option<BlockId>, kind: BlockKind, text: &str) -> BlockId {
    let index = match parent {
        Some(p) => page.children(p).unwrap().len(),
        None => page.root_blocks().len(),
    };
    match page
        .apply(PageCommand::InsertBlock {
            parent,
            index,
            kind,
            text: text.into(),
        })
        .unwrap()
    {
        PageOutcome::BlockCreated(id) => id,
        other => panic!("expected a block, got {other:?}"),
    }
}

/// Rows as readable lines (no ids, which change every run): depth, style, then formatted text.
fn render(rows: &[BlockRow]) -> String {
    let mut out = String::new();
    for row in rows {
        let style = match &row.style {
            BlockStyle::Numbered(n) => format!("{n}."),
            BlockStyle::Todo { checked } => if *checked { "[x]" } else { "[ ]" }.to_owned(),
            BlockStyle::Bulleted => "•".to_owned(),
            other => format!("{other:?}"),
        };
        let text = row.spans.as_ref().map_or_else(String::new, |spans| {
            spans
                .iter()
                .map(|s| {
                    let mut marks = Vec::new();
                    if s.marks.bold {
                        marks.push("b".to_owned());
                    }
                    if s.marks.italic {
                        marks.push("i".to_owned());
                    }
                    if s.marks.code {
                        marks.push("code".to_owned());
                    }
                    if let Some(href) = &s.marks.link {
                        marks.push(format!("link={href}"));
                    }
                    if marks.is_empty() {
                        format!("{:?}", s.text)
                    } else {
                        format!("{:?}<{}>", s.text, marks.join(","))
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        });
        let _ = writeln!(out, "{}{style} {text}", "  ".repeat(row.depth));
    }
    out
}

fn numbers(page: &PageDoc) -> Vec<(usize, Option<usize>)> {
    PageView::build(page)
        .unwrap()
        .rows()
        .iter()
        .map(|row| match row.style {
            BlockStyle::Numbered(n) => (row.depth, Some(n)),
            _ => (row.depth, None),
        })
        .collect()
}

#[test]
fn consecutive_numbered_siblings_count_up_and_anything_else_restarts() {
    let mut page = PageDoc::new();
    for _ in 0..3 {
        add(&mut page, None, BlockKind::NumberedList, "n");
    }
    add(&mut page, None, BlockKind::Paragraph, "break");
    add(&mut page, None, BlockKind::NumberedList, "again");
    add(&mut page, None, BlockKind::BulletedList, "bullet");
    add(&mut page, None, BlockKind::NumberedList, "and again");
    assert_eq!(
        numbers(&page),
        vec![
            (0, Some(1)),
            (0, Some(2)),
            (0, Some(3)),
            (0, None),
            (0, Some(1)),
            (0, None),
            (0, Some(1)),
        ]
    );
}

#[test]
fn nested_lists_count_on_their_own_and_children_dont_break_the_parent_run() {
    let mut page = PageDoc::new();
    let first = add(&mut page, None, BlockKind::NumberedList, "one");
    add(&mut page, Some(first), BlockKind::NumberedList, "one.a");
    add(&mut page, Some(first), BlockKind::NumberedList, "one.b");
    add(&mut page, Some(first), BlockKind::Paragraph, "note");
    let second = add(&mut page, None, BlockKind::NumberedList, "two");
    add(&mut page, Some(second), BlockKind::NumberedList, "two.a");
    assert_eq!(
        numbers(&page),
        vec![
            (0, Some(1)),
            (1, Some(1)),
            (1, Some(2)),
            (1, None),
            (0, Some(2)),
            (1, Some(1)),
        ]
    );
}

#[test]
fn typing_rereads_one_block_and_turning_a_block_into_a_list_renumbers() {
    let mut page = PageDoc::new();
    let ids: Vec<BlockId> = (0..50)
        .map(|i| add(&mut page, None, BlockKind::NumberedList, &format!("item {i}")))
        .collect();
    let mut view = PageView::default();
    view.refresh(&page).unwrap();
    assert_eq!(view.reads, 50, "the first refresh reads everything");

    page.apply(PageCommand::InsertText {
        id: ids[10],
        pos: 0,
        text: "x".into(),
    })
    .unwrap();
    view.refresh(&page).unwrap();
    assert_eq!(view.reads, 1, "typing in one block re-reads only that block");
    assert_eq!(view.rows()[10].text.as_deref(), Some("xitem 10"));

    page.apply(PageCommand::SetBlockKind {
        id: ids[5],
        kind: BlockKind::Paragraph,
    })
    .unwrap();
    view.refresh(&page).unwrap();
    assert_eq!(view.reads, 1, "renumbering reuses the cached text");
    assert_eq!(
        view.rows()[6].style,
        BlockStyle::Numbered(1),
        "restarts after the paragraph"
    );
    assert_eq!(view.rows(), PageView::build(&page).unwrap().rows());

    view.refresh(&page).unwrap();
    assert_eq!(view.reads, 0, "nothing changed, nothing read");
}

#[test]
fn snapshot_of_a_representative_page() {
    let mut page = PageDoc::new();
    add(&mut page, None, BlockKind::Heading { level: 1 }, "Trip plans");
    let intro = add(
        &mut page,
        None,
        BlockKind::Paragraph,
        "Pack light and book early.",
    );
    page.apply(PageCommand::SetMark {
        id: intro,
        range: 0..10,
        mark: Mark::Bold,
    })
    .unwrap();
    page.apply(PageCommand::SetMark {
        id: intro,
        range: 15..19,
        mark: Mark::Link("https://trains.example".into()),
    })
    .unwrap();
    let steps = add(&mut page, None, BlockKind::NumberedList, "Book the train");
    add(
        &mut page,
        Some(steps),
        BlockKind::NumberedList,
        "Seat by the window",
    );
    add(&mut page, Some(steps), BlockKind::NumberedList, "Bring snacks");
    add(&mut page, None, BlockKind::NumberedList, "Pack");
    add(&mut page, None, BlockKind::Todo { checked: true }, "Charge phone");
    add(
        &mut page,
        None,
        BlockKind::Todo { checked: false },
        "Print tickets",
    );
    add(&mut page, None, BlockKind::Quote, "Travel far, travel light.");
    add(&mut page, None, BlockKind::Divider, "");
    add(
        &mut page,
        None,
        BlockKind::Code {
            language: "rust".into(),
        },
        "fn main() {}",
    );
    add(
        &mut page,
        None,
        BlockKind::Image {
            asset: AssetId::of(b"map"),
            width: None,
            alt: "Route map".into(),
        },
        "",
    );
    insta::assert_snapshot!(render(PageView::build(&page).unwrap().rows()));
}

/// A random edit. Numbers pick blocks and positions, wrapping around what exists.
#[derive(Debug, Clone)]
enum Step {
    Insert(Option<usize>, usize, usize),
    Delete(usize),
    Move(usize, Option<usize>),
    Indent(usize),
    Outdent(usize),
    Type(usize, usize),
    Erase(usize, usize),
    Bold(usize, usize),
    Kind(usize, usize),
    Split(usize, usize),
    Merge(usize),
    Undo,
    Redo,
}

fn step() -> impl Strategy<Value = Step> {
    let n = || 0..32_usize;
    prop_oneof![
        3 => (proptest::option::of(n()), n(), n()).prop_map(|(p, i, k)| Step::Insert(p, i, k)),
        1 => n().prop_map(Step::Delete),
        1 => (n(), proptest::option::of(n())).prop_map(|(b, p)| Step::Move(b, p)),
        1 => n().prop_map(Step::Indent),
        1 => n().prop_map(Step::Outdent),
        3 => (n(), n()).prop_map(|(b, p)| Step::Type(b, p)),
        1 => (n(), n()).prop_map(|(b, p)| Step::Erase(b, p)),
        1 => (n(), n()).prop_map(|(b, p)| Step::Bold(b, p)),
        2 => (n(), n()).prop_map(|(b, k)| Step::Kind(b, k)),
        1 => (n(), n()).prop_map(|(b, p)| Step::Split(b, p)),
        1 => n().prop_map(Step::Merge),
        1 => Just(Step::Undo),
        1 => Just(Step::Redo),
    ]
}

fn kinds() -> [BlockKind; 6] {
    [
        BlockKind::Paragraph,
        BlockKind::NumberedList,
        BlockKind::NumberedList,
        BlockKind::BulletedList,
        BlockKind::Todo { checked: false },
        BlockKind::Divider,
    ]
}

fn command(step: &Step, page: &PageDoc, order: &[BlockId]) -> Option<PageCommand> {
    let pick = |n: usize| order.get(n % order.len().max(1)).copied();
    let len = |id: BlockId| {
        page.plain_text(id)
            .ok()
            .flatten()
            .map_or(0, |t| t.chars().count())
    };
    Some(match *step {
        Step::Insert(parent, index, kind) => {
            let parent = parent.and_then(pick);
            let siblings = parent.map_or(page.root_blocks().len(), |p| {
                page.children(p).map_or(0, |c| c.len())
            });
            let kind = kinds()[kind % 6].clone();
            let text = if kind.has_text() { "ab" } else { "" };
            PageCommand::InsertBlock {
                parent,
                index: index % (siblings + 1),
                kind,
                text: text.into(),
            }
        }
        Step::Delete(b) => PageCommand::DeleteBlocks { ids: vec![pick(b)?] },
        Step::Move(b, p) => PageCommand::MoveBlocks {
            ids: vec![pick(b)?],
            new_parent: p.and_then(pick),
            index: 0,
        },
        Step::Indent(b) => PageCommand::Indent { ids: vec![pick(b)?] },
        Step::Outdent(b) => PageCommand::Outdent { ids: vec![pick(b)?] },
        Step::Type(b, p) => {
            let id = pick(b)?;
            PageCommand::InsertText {
                id,
                pos: p % (len(id) + 1),
                text: "z".into(),
            }
        }
        Step::Erase(b, p) => {
            let id = pick(b)?;
            let start = p % (len(id) + 1);
            PageCommand::DeleteText {
                id,
                range: start..(start + 1).min(len(id)),
            }
        }
        Step::Bold(b, p) => {
            let id = pick(b)?;
            let start = p % (len(id) + 1);
            PageCommand::SetMark {
                id,
                range: start..(start + 2).min(len(id)),
                mark: Mark::Bold,
            }
        }
        Step::Kind(b, k) => PageCommand::SetBlockKind {
            id: pick(b)?,
            kind: kinds()[k % 6].clone(),
        },
        Step::Split(b, p) => {
            let id = pick(b)?;
            PageCommand::SplitBlock {
                id,
                pos: p % (len(id) + 1),
            }
        }
        Step::Merge(b) => PageCommand::MergeIntoPrevious { id: pick(b)? },
        Step::Undo | Step::Redo => return None,
    })
}

proptest! {
    /// After every random edit, undo or redo, refreshing the kept view gives exactly what a view
    /// built from scratch shows.
    #[test]
    fn the_incremental_view_always_matches_a_fresh_build(steps in proptest::collection::vec(step(), 1..40)) {
        let mut page = PageDoc::new();
        page.set_undo_merge_interval(0);
        let mut view = PageView::default();
        view.refresh(&page).map_err(|e| TestCaseError::fail(e.to_string()))?;
        for step in steps {
            let order: Vec<BlockId> = view.rows().iter().map(|r| r.id).collect();
            match step {
                Step::Undo => { let _ = page.undo(); }
                Step::Redo => { let _ = page.redo(); }
                ref other => if let Some(command) = command(other, &page, &order) {
                    let _ = page.apply(command); // refused commands must leave the view right too
                },
            }
            view.refresh(&page).map_err(|e| TestCaseError::fail(e.to_string()))?;
            let fresh = PageView::build(&page).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(view.rows(), fresh.rows(), "after {:?}", step);
        }
    }
}

fn table(
    page: &mut PageDoc,
    parent: Option<BlockId>,
    rows: usize,
    columns: usize,
    header_row: bool,
) -> BlockId {
    let index = match parent {
        Some(p) => page.children(p).unwrap().len(),
        None => page.root_blocks().len(),
    };
    match page
        .apply(PageCommand::InsertTable {
            parent,
            index,
            rows,
            columns,
            header_row,
        })
        .unwrap()
    {
        PageOutcome::BlockCreated(id) => id,
        other => panic!("expected a table, got {other:?}"),
    }
}

fn type_in(page: &mut PageDoc, cell: BlockId, text: &str) {
    page.apply(PageCommand::InsertText {
        id: cell,
        pos: 0,
        text: text.into(),
    })
    .unwrap();
}

#[test]
fn a_table_lists_its_cells_row_by_row_after_it() {
    let mut page = PageDoc::new();
    add(&mut page, None, BlockKind::Paragraph, "Shopping");
    let t = table(&mut page, None, 3, 2, true);
    for (r, row) in page.table_cells(t).unwrap().into_iter().enumerate() {
        for (c, cell) in row.into_iter().enumerate() {
            let text = [["Item", "Price"], ["Tomatoes", "2.50"], ["Basil", ""]][r][c];
            type_in(&mut page, cell, text);
        }
    }
    let price = page.table_cells(t).unwrap()[1][1];
    page.apply(PageCommand::SetMark {
        id: price,
        range: 0..4,
        mark: Mark::Bold,
    })
    .unwrap();
    add(&mut page, None, BlockKind::Paragraph, "after");
    insta::assert_snapshot!(render(PageView::build(&page).unwrap().rows()));
}

#[test]
fn a_nested_table_without_a_header_row() {
    let mut page = PageDoc::new();
    let quote = add(&mut page, None, BlockKind::Quote, "Notes");
    let t = table(&mut page, Some(quote), 1, 3, false);
    let cell = page.table_cells(t).unwrap()[0][2];
    type_in(&mut page, cell, "two\nlines");
    insta::assert_snapshot!(render(PageView::build(&page).unwrap().rows()));
}

#[test]
fn typing_in_a_cell_rereads_only_that_cell_and_keeps_its_place() {
    let mut page = PageDoc::new();
    let t = table(&mut page, None, 2, 2, true);
    let mut view = PageView::default();
    view.refresh(&page).unwrap();
    let cell = page.table_cells(t).unwrap()[1][0];
    type_in(&mut page, cell, "x");
    view.refresh(&page).unwrap();
    assert_eq!(view.reads, 1);
    assert_eq!(view.rows(), PageView::build(&page).unwrap().rows());
    let row = view.rows().iter().find(|r| r.id == cell).unwrap();
    assert_eq!(
        row.style,
        BlockStyle::TableCell {
            row: 1,
            column: 0,
            header: false
        }
    );

    page.apply(PageCommand::SetProp {
        id: t,
        prop: leaf_model::BlockProp::TableHeaderRow(false),
    })
    .unwrap();
    page.apply(PageCommand::InsertTableColumn { table: t, index: 2 })
        .unwrap();
    view.refresh(&page).unwrap();
    assert_eq!(view.rows(), PageView::build(&page).unwrap().rows());
    assert_eq!(
        view.rows()[0].style,
        BlockStyle::Table {
            header_row: false,
            rows: 2,
            columns: 3
        }
    );
}
