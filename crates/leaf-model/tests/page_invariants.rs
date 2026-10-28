//! Property test: every page command, in any order, keeps every invariant (E02-S07).
//!
//! Random sequences of all page commands, picking real block IDs and in-range positions so most are
//! valid (invalid ones must be refused cleanly). After every step `check_invariants()` must pass, and
//! a save and load at the end must give back the same page. Run with `PROPTEST_CASES=10000` for the
//! long run; CI does that on `main`.

use leaf_core::{AssetId, BlockId};
use leaf_model::{
    BlockKind, BlockProp, History, Mark, MarkKind, PageCommand, PageDoc, PageOutcome, TextSpan,
};
use proptest::prelude::*;

fn kinds() -> [BlockKind; 12] {
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
    ]
}

fn marks() -> [Mark; 5] {
    [
        Mark::Bold,
        Mark::Italic,
        Mark::Strike,
        Mark::Code,
        Mark::Link("https://leaf.app".into()),
    ]
}

fn mark_kinds() -> [MarkKind; 5] {
    [
        MarkKind::Bold,
        MarkKind::Italic,
        MarkKind::Strike,
        MarkKind::Code,
        MarkKind::Link,
    ]
}

fn props() -> [BlockProp; 6] {
    [
        BlockProp::Checked(true),
        BlockProp::Checked(false),
        BlockProp::HeadingLevel(2),
        BlockProp::Language("python".into()),
        BlockProp::ImageWidth(None),
        BlockProp::ImageAlt("b".into()),
    ]
}

/// A random step. Numbers pick blocks, kinds, marks and positions, wrapping around what exists.
#[derive(Debug, Clone)]
enum Step {
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
}

fn text() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        Just("leaf".to_owned()),
        Just("👋🏽 hi".to_owned()),
        Just("नमस्ते".to_owned())
    ]
}

fn step() -> impl Strategy<Value = Step> {
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
        2 => (n(), n(), 0..5_usize, n()).prop_map(|(block, start, len, mark)| Step::SetMark { block, start, len, mark }),
        1 => (n(), n(), 0..5_usize, n()).prop_map(|(block, start, len, kind)| Step::RemoveMark { block, start, len, kind }),
        2 => (n(), n()).prop_map(|(block, kind)| Step::SetKind { block, kind }),
        1 => (n(), n()).prop_map(|(block, prop)| Step::SetProp { block, prop }),
        2 => (n(), n()).prop_map(|(block, pos)| Step::Split { block, pos }),
        2 => n().prop_map(|block| Step::Merge { block }),
    ]
}

/// Every block in reading order with its depth, kind and formatted text: what "the same page" means.
fn outline(page: &PageDoc) -> Vec<(usize, BlockId, BlockKind, Option<Vec<TextSpan>>)> {
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
struct Picker<'a> {
    page: &'a PageDoc,
    order: Vec<BlockId>,
}

impl<'a> Picker<'a> {
    fn new(page: &'a PageDoc) -> Self {
        Self {
            page,
            order: outline(page).into_iter().map(|(_, id, _, _)| id).collect(),
        }
    }

    fn block(&self, n: usize) -> Option<BlockId> {
        self.order.get(n % self.order.len().max(1)).copied()
    }

    fn blocks(&self, ns: Vec<usize>) -> Vec<BlockId> {
        ns.into_iter().filter_map(|n| self.block(n)).collect()
    }

    fn len(&self, id: BlockId) -> usize {
        self.page
            .plain_text(id)
            .ok()
            .flatten()
            .map_or(0, |t| t.chars().count())
    }

    fn pos(&self, id: BlockId, n: usize) -> usize {
        n % (self.len(id) + 1)
    }

    fn range(&self, id: BlockId, start: usize, n: usize) -> std::ops::Range<usize> {
        let start = self.pos(id, start);
        start..(start + n).min(self.len(id))
    }
}

/// Turns a step into a command against the page as it is now. `None` when there is nothing to act on.
fn command(step: Step, page: &PageDoc) -> Option<PageCommand> {
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
            let kind = kinds()[kind % 12].clone();
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
            kind: kinds()[kind % 12].clone(),
        },
        Step::SetProp { block, prop } => PageCommand::SetProp {
            id: p.block(block)?,
            prop: props()[prop % 6].clone(),
        },
        Step::Merge { block } => PageCommand::MergeIntoPrevious { id: p.block(block)? },
        text_step => return text_command(text_step, &p),
    })
}

/// The text-editing steps: typing, erasing, marks and splitting.
fn text_command(step: Step, p: &Picker<'_>) -> Option<PageCommand> {
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
                mark: marks()[mark % 5].clone(),
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
                kind: mark_kinds()[kind % 5],
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

proptest! {
    #[test]
    fn every_command_sequence_keeps_every_invariant(steps in proptest::collection::vec(step(), 1..40)) {
        let mut page = PageDoc::new();
        for step in steps {
            let Some(command) = command(step, &page) else { continue };
            // Valid commands change the page; invalid ones must be refused. Either way the rules hold.
            let result = page.apply(command.clone());
            if let Err(broken) = page.check_invariants() {
                let list: Vec<String> = broken.iter().map(ToString::to_string).collect();
                prop_assert!(false, "after {:?} (result {:?}):\n{}", command, result, list.join("\n"));
            }
        }
        let reloaded = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
        prop_assert_eq!(reloaded.check_invariants(), Ok(()));
        prop_assert_eq!(outline(&reloaded), outline(&page));
    }
}

proptest! {
    /// BLOCK_MODEL invariant 7 (E02-S08): undo of command N restores the state before N, compared by
    /// `BlockId`, and redo replays it; the other invariants hold after every undo and redo.
    #[test]
    fn undo_retraces_every_command_and_redo_replays_it(steps in proptest::collection::vec(step(), 1..30)) {
        let mut page = PageDoc::new();
        page.set_undo_merge_interval(0);
        let mut history = vec![outline(&page)];
        for step in steps {
            let Some(command) = command(step, &page) else { continue };
            match page.apply(command) {
                Ok(PageOutcome::Unchanged) | Err(_) => {} // no change, no undo step
                Ok(_) => history.push(outline(&page)),
            }
        }

        let last = history.len() - 1;
        for expected in history[..last].iter().rev() {
            let undone = matches!(page.undo().unwrap(), History::Done { .. });
            prop_assert!(undone);
            prop_assert_eq!(&outline(&page), expected);
            prop_assert_eq!(page.check_invariants(), Ok(()));
        }
        prop_assert_eq!(page.undo().unwrap(), History::Nothing);
        for expected in &history[1..] {
            let redone = matches!(page.redo().unwrap(), History::Done { .. });
            prop_assert!(redone);
            prop_assert_eq!(&outline(&page), expected);
            prop_assert_eq!(page.check_invariants(), Ok(()));
        }
        prop_assert_eq!(page.redo().unwrap(), History::Nothing);
    }
}
