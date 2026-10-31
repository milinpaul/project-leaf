//! Random page commands for property tests (E02-S07, E02-S09): a `Step` strategy that picks real
//! blocks and in-range positions on the page as it is, so most commands are valid.

#![allow(dead_code, reason = "each test file uses part of this")]

use leaf_core::{AssetId, BlockId, PageId};
use leaf_model::{
    BlockKind, BlockProp, CalloutTone, DueDate, DueTime, Mark, MarkKind, PageCommand, PageDoc, Priority,
    TextSpan, WebPreview,
};
use proptest::prelude::*;

pub(crate) fn kinds() -> [BlockKind; 18] {
    [
        BlockKind::Paragraph,
        BlockKind::Heading { level: 1 },
        BlockKind::Heading { level: 2 },
        BlockKind::Heading { level: 3 },
        BlockKind::BulletedList,
        BlockKind::NumberedList,
        BlockKind::Todo { checked: false },
        BlockKind::Todo { checked: true },
        BlockKind::Quote,
        BlockKind::Code {
            language: "rust".into(),
        },
        BlockKind::Divider,
        BlockKind::Image {
            asset: AssetId::of(b"img"),
            width: Some(320.0),
            alt: "a".into(),
        },
        BlockKind::Task {
            done: false,
            due: None,
            time: None,
            priority: None,
        },
        BlockKind::Task {
            done: true,
            due: DueDate::new(2026, 10, 31).ok(),
            time: DueTime::new(14, 30).ok(),
            priority: Some(Priority::High),
        },
        BlockKind::Callout {
            tone: CalloutTone::Info,
        },
        BlockKind::Callout {
            tone: CalloutTone::Warning,
        },
        // The same address as the link mark, so a block that is only that link can become one.
        BlockKind::WebBookmark {
            url: "https://leaf.app".into(),
            preview: WebPreview::default(),
        },
        BlockKind::WebBookmark {
            url: "https://example.com/a".into(),
            preview: WebPreview {
                title: Some("A".into()),
                ..WebPreview::default()
            },
        },
    ]
}

/// What a fetch finds, for [`Step::Preview`].
pub(crate) fn fetched() -> WebPreview {
    WebPreview {
        title: Some("Leaf".into()),
        description: Some("A local-first editor".into()),
        image: Some(AssetId::of(b"cover")),
        favicon: None,
        fetched_at: Some(1_793_000_000_000),
    }
}

/// Pages that links point at: fixed, so a shrunk failing case reads the same every time.
pub(crate) fn link_targets() -> [PageId; 2] {
    [
        "01a18f8c-179f-7705-9e09-69f9f450c2f2",
        "01a18f8c-179f-7705-9e09-69f9f450c2f3",
    ]
    // Both parse; the fallback only keeps clippy's no-`expect` rule.
    .map(|id| id.parse().unwrap_or_else(|_| PageId::new()))
}

pub(crate) fn marks() -> [Mark; 6] {
    [
        Mark::Bold,
        Mark::Italic,
        Mark::Strike,
        Mark::Code,
        Mark::Link("https://leaf.app".into()),
        // Mostly refused (only links take it); on a link it re-points it.
        Mark::PageLink(link_targets()[1]),
    ]
}

pub(crate) fn mark_kinds() -> [MarkKind; 6] {
    [
        MarkKind::Bold,
        MarkKind::Italic,
        MarkKind::Strike,
        MarkKind::Code,
        MarkKind::Link,
        MarkKind::PageLink,
    ]
}

pub(crate) fn props() -> [BlockProp; 18] {
    [
        BlockProp::TableHeaderRow(true),
        BlockProp::TableHeaderRow(false),
        BlockProp::Checked(true),
        BlockProp::Checked(false),
        BlockProp::HeadingLevel(2),
        BlockProp::Language("python".into()),
        BlockProp::ImageWidth(None),
        BlockProp::ImageAlt("b".into()),
        BlockProp::TaskDone(true),
        BlockProp::TaskDue(DueDate::new(2027, 2, 28).ok()),
        BlockProp::TaskDue(None),
        BlockProp::TaskTime(DueTime::new(9, 0).ok()),
        BlockProp::TaskPriority(Some(Priority::Low)),
        BlockProp::TaskPriority(None),
        BlockProp::CalloutTone(CalloutTone::Error),
        BlockProp::CalloutTone(CalloutTone::Note),
        BlockProp::WebAddress("https://leaf.app".into()),
        BlockProp::WebAddress("https://example.com/b".into()),
    ]
}

/// A random step. Numbers pick blocks, kinds, marks and positions, wrapping around what exists.
#[derive(Debug, Clone)]
pub(crate) enum Step {
    Insert {
        parent: Option<usize>,
        index: usize,
        kind: usize,
        text: String,
    },
    Delete {
        blocks: Vec<usize>,
    },
    Move {
        blocks: Vec<usize>,
        parent: Option<usize>,
        index: usize,
    },
    Indent {
        blocks: Vec<usize>,
    },
    Outdent {
        blocks: Vec<usize>,
    },
    Type {
        block: usize,
        pos: usize,
        text: String,
    },
    Erase {
        block: usize,
        start: usize,
        len: usize,
    },
    Replace {
        block: usize,
        start: usize,
        len: usize,
        text: String,
    },
    /// A link to one of [`link_targets`].
    Link {
        block: usize,
        pos: usize,
        page: usize,
    },
    SetMark {
        block: usize,
        start: usize,
        len: usize,
        mark: usize,
    },
    RemoveMark {
        block: usize,
        start: usize,
        len: usize,
        kind: usize,
    },
    SetKind {
        block: usize,
        kind: usize,
    },
    SetProp {
        block: usize,
        prop: usize,
    },
    Split {
        block: usize,
        pos: usize,
    },
    Merge {
        block: usize,
    },
    Duplicate {
        blocks: Vec<usize>,
    },
    InsertTable {
        parent: Option<usize>,
        index: usize,
        rows: usize,
        columns: usize,
        header_row: bool,
    },
    /// A fetched preview arriving for a block: for the address it holds now, or (`stale`) one it held
    /// before, as when the owner changed it while fetching.
    Preview {
        block: usize,
        stale: bool,
    },
    /// A row or column command (`op` picks which) on a table on the page.
    Reshape {
        table: usize,
        op: usize,
        a: usize,
        b: usize,
    },
}

pub(crate) fn text() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        Just("leaf".to_owned()),
        Just("👋🏽 hi".to_owned()),
        Just("नमस्ते".to_owned())
    ]
}

pub(crate) fn step() -> impl Strategy<Value = Step> {
    let n = || 0..64_usize;
    let some = || proptest::collection::vec(0..64_usize, 0..3);
    prop_oneof![
        4 => (proptest::option::of(n()), n(), n(), text()).prop_map(|(parent, index, kind, text)| Step::Insert { parent, index, kind, text }),
        1 => some().prop_map(|blocks| Step::Delete { blocks }),
        2 => (some(), proptest::option::of(n()), n()).prop_map(|(blocks, parent, index)| Step::Move { blocks, parent, index }),
        1 => some().prop_map(|blocks| Step::Indent { blocks }),
        1 => some().prop_map(|blocks| Step::Outdent { blocks }),
        3 => (n(), n(), text()).prop_map(|(block, pos, text)| Step::Type { block, pos, text }),
        1 => (n(), n(), 0..4_usize).prop_map(|(block, start, len)| Step::Erase { block, start, len }),
        1 => (n(), n(), 0..4_usize, text()).prop_map(|(block, start, len, text)| Step::Replace { block, start, len, text }),
        2 => (n(), n(), n()).prop_map(|(block, pos, page)| Step::Link { block, pos, page }),
        2 => (n(), n(), 0..5_usize, n()).prop_map(|(block, start, len, mark)| Step::SetMark { block, start, len, mark }),
        1 => (n(), n(), 0..5_usize, n()).prop_map(|(block, start, len, kind)| Step::RemoveMark { block, start, len, kind }),
        2 => (n(), n()).prop_map(|(block, kind)| Step::SetKind { block, kind }),
        1 => (n(), n()).prop_map(|(block, prop)| Step::SetProp { block, prop }),
        2 => (n(), n()).prop_map(|(block, pos)| Step::Split { block, pos }),
        2 => n().prop_map(|block| Step::Merge { block }),
        1 => some().prop_map(|blocks| Step::Duplicate { blocks }),
        1 => (proptest::option::of(n()), n(), 1..4_usize, 1..4_usize, any::<bool>()).prop_map(
            |(parent, index, rows, columns, header_row)| Step::InsertTable { parent, index, rows, columns, header_row }
        ),
        3 => (n(), 0..6_usize, n(), n()).prop_map(|(table, op, a, b)| Step::Reshape { table, op, a, b }),
        1 => (n(), any::<bool>()).prop_map(|(block, stale)| Step::Preview { block, stale }),
    ]
}

/// Every block in reading order with its depth, kind and formatted text: what "the same page" means.
pub(crate) fn outline(page: &PageDoc) -> Vec<(usize, BlockId, BlockKind, Option<Vec<TextSpan>>)> {
    fn walk(
        page: &PageDoc,
        blocks: Vec<BlockId>,
        depth: usize,
        out: &mut Vec<(usize, BlockId, BlockKind, Option<Vec<TextSpan>>)>,
    ) {
        for block in blocks {
            out.push((
                depth,
                block,
                page.kind(block).ok().unwrap_or(BlockKind::Paragraph),
                page.text(block).ok().flatten(),
            ));
            walk(page, page.children(block).unwrap_or_default(), depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(page, page.root_blocks(), 0, &mut out);
    out
}

/// Picks blocks and positions on the page as it is now, wrapping numbers into range.
pub(crate) struct Picker<'a> {
    page: &'a PageDoc,
    order: Vec<BlockId>,
}

impl<'a> Picker<'a> {
    pub(crate) fn new(page: &'a PageDoc) -> Self {
        Self {
            page,
            order: outline(page).into_iter().map(|(_, id, _, _)| id).collect(),
        }
    }

    pub(crate) fn block(&self, n: usize) -> Option<BlockId> {
        self.order.get(n % self.order.len().max(1)).copied()
    }

    pub(crate) fn blocks(&self, ns: Vec<usize>) -> Vec<BlockId> {
        ns.into_iter().filter_map(|n| self.block(n)).collect()
    }

    pub(crate) fn len(&self, id: BlockId) -> usize {
        self.page
            .plain_text(id)
            .ok()
            .flatten()
            .map_or(0, |t| t.chars().count())
    }

    pub(crate) fn pos(&self, id: BlockId, n: usize) -> usize {
        n % (self.len(id) + 1)
    }

    pub(crate) fn range(&self, id: BlockId, start: usize, n: usize) -> std::ops::Range<usize> {
        let start = self.pos(id, start);
        start..(start + n).min(self.len(id))
    }
}

/// Turns a step into a command against the page as it is now. `None` when there is nothing to act on.
pub(crate) fn command(step: Step, page: &PageDoc) -> Option<PageCommand> {
    let p = Picker::new(page);
    Some(match step {
        Step::Insert {
            parent,
            index,
            kind,
            text,
        } => {
            let parent = parent.and_then(|n| p.block(n));
            let siblings = parent.map_or_else(
                || page.root_blocks().len(),
                |b| page.children(b).map_or(0, |c| c.len()),
            );
            let kind = kinds()[kind % kinds().len()].clone();
            let text = if kind.has_text() { text } else { String::new() };
            PageCommand::InsertBlock {
                parent,
                index: index % (siblings + 1),
                kind,
                text,
            }
        }
        Step::Delete { blocks } => PageCommand::DeleteBlocks {
            ids: p.blocks(blocks),
        },
        Step::Move {
            blocks,
            parent,
            index,
        } => PageCommand::MoveBlocks {
            ids: p.blocks(blocks),
            new_parent: parent.and_then(|n| p.block(n)),
            index: index % 4,
        },
        Step::Indent { blocks } => PageCommand::Indent {
            ids: p.blocks(blocks),
        },
        Step::Outdent { blocks } => PageCommand::Outdent {
            ids: p.blocks(blocks),
        },
        Step::SetKind { block, kind } => PageCommand::SetBlockKind {
            id: p.block(block)?,
            kind: kinds()[kind % kinds().len()].clone(),
        },
        Step::SetProp { block, prop } => PageCommand::SetProp {
            id: p.block(block)?,
            prop: props()[prop % props().len()].clone(),
        },
        Step::Merge { block } => PageCommand::MergeIntoPrevious { id: p.block(block)? },
        Step::Duplicate { blocks } => PageCommand::DuplicateBlocks {
            ids: p.blocks(blocks),
        },
        Step::InsertTable {
            parent,
            index,
            rows,
            columns,
            header_row,
        } => {
            let parent = parent.and_then(|n| p.block(n));
            let siblings = parent.map_or_else(
                || page.root_blocks().len(),
                |b| page.children(b).map_or(0, |c| c.len()),
            );
            PageCommand::InsertTable {
                parent,
                index: index % (siblings + 1),
                rows,
                columns,
                header_row,
            }
        }
        Step::Reshape { table, op, a, b } => reshape(page, table, op, a, b)?,
        Step::Preview { block, stale } => {
            let id = p.block(block)?;
            let url = match page.kind(id).ok()? {
                BlockKind::WebBookmark { url, .. } if !stale => url,
                _ => "https://leaf.app/old".into(),
            };
            PageCommand::SetWebBookmarkPreview {
                id,
                url,
                preview: fetched(),
            }
        }
        text_step => return text_command(text_step, &p),
    })
}

/// A row or column command on one of the page's tables, with positions wrapped into (or just past)
/// range so most are valid and some are refused.
fn reshape(page: &PageDoc, table: usize, op: usize, a: usize, b: usize) -> Option<PageCommand> {
    let tables = tables(page);
    let table = *tables.get(table % tables.len().max(1))?;
    let cells = page.table_cells(table).ok()?;
    let rows = cells.len() + 1;
    let columns = cells.iter().map(Vec::len).max().unwrap_or(0) + 1;
    Some(match op {
        0 => PageCommand::InsertTableRow {
            table,
            index: a % rows,
        },
        1 => PageCommand::DeleteTableRow {
            table,
            index: a % rows,
        },
        2 => PageCommand::MoveTableRow {
            table,
            from: a % rows,
            to: b % rows,
        },
        3 => PageCommand::InsertTableColumn {
            table,
            index: a % columns,
        },
        4 => PageCommand::DeleteTableColumn {
            table,
            index: a % columns,
        },
        _ => PageCommand::MoveTableColumn {
            table,
            from: a % columns,
            to: b % columns,
        },
    })
}

/// Every table on the page, in reading order.
pub(crate) fn tables(page: &PageDoc) -> Vec<BlockId> {
    outline(page)
        .into_iter()
        .filter(|(_, _, kind, _)| matches!(kind, BlockKind::Table { .. }))
        .map(|(_, id, _, _)| id)
        .collect()
}

/// Whether every table is a whole grid: at least one row, every row the same length, at least one
/// cell. Commands keep tables this way; only merges may leave rows uneven.
pub(crate) fn tables_are_grids(page: &PageDoc) -> bool {
    tables(page).into_iter().all(|table| {
        let cells = page.table_cells(table).unwrap_or_default();
        let width = cells.first().map_or(0, Vec::len);
        width > 0 && cells.iter().all(|row| row.len() == width)
    })
}

/// The text-editing steps: typing, erasing, marks and splitting.
pub(crate) fn text_command(step: Step, p: &Picker<'_>) -> Option<PageCommand> {
    Some(match step {
        Step::Type { block, pos, text } => {
            let id = p.block(block)?;
            PageCommand::InsertText {
                id,
                pos: p.pos(id, pos),
                text,
            }
        }
        Step::Erase { block, start, len } => {
            let id = p.block(block)?;
            PageCommand::DeleteText {
                id,
                range: p.range(id, start, len),
            }
        }
        Step::Replace {
            block,
            start,
            len,
            text,
        } => {
            let id = p.block(block)?;
            PageCommand::ReplaceText {
                id,
                range: p.range(id, start, len),
                text,
            }
        }
        Step::Link { block, pos, page } => {
            let id = p.block(block)?;
            PageCommand::InsertPageLink {
                id,
                pos: p.pos(id, pos),
                page: link_targets()[page % 2],
            }
        }
        Step::SetMark {
            block,
            start,
            len,
            mark,
        } => {
            let id = p.block(block)?;
            PageCommand::SetMark {
                id,
                range: p.range(id, start, len),
                mark: marks()[mark % 6].clone(),
            }
        }
        Step::RemoveMark {
            block,
            start,
            len,
            kind,
        } => {
            let id = p.block(block)?;
            PageCommand::RemoveMark {
                id,
                range: p.range(id, start, len),
                kind: mark_kinds()[kind % 6],
            }
        }
        Step::Split { block, pos } => {
            let id = p.block(block)?;
            PageCommand::SplitBlock {
                id,
                pos: p.pos(id, pos),
            }
        }
        _ => return None,
    })
}
