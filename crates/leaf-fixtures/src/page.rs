//! A big page: thousands of blocks of every kind people use, with nesting and formatting, the way
//! a long working document looks (headed sections of paragraphs, lists nested a few levels, to-dos,
//! quotes, code and dividers).

use std::collections::HashMap;

use leaf_core::BlockId;
use leaf_model::{BlockKind, Mark, PageCommand, PageDoc, PageOutcome};

use crate::words::{code, sentence, words};
use crate::{FixtureError, Rng};

/// How deep lists nest at most.
const MAX_DEPTH: usize = 3;

/// A new page with `blocks` blocks, its outline fixed by `seed`.
pub fn big_page(seed: u64, blocks: usize) -> Result<PageDoc, FixtureError> {
    let mut page = PageDoc::new();
    fill_page(&mut page, seed, blocks)?;
    Ok(page)
}

/// Appends `blocks` blocks to `page`, their outline fixed by `seed`. The changes are left unsaved,
/// as typing would leave them.
pub fn fill_page(page: &mut PageDoc, seed: u64, blocks: usize) -> Result<(), FixtureError> {
    let mut rng = Rng::new(seed);
    let mut writer = Writer {
        page,
        children: HashMap::new(),
        list: Vec::new(),
    };
    for i in 0..blocks {
        writer.add(&mut rng, i)?;
    }
    Ok(())
}

/// Builds a page block by block, remembering how many children each parent has (so each insert
/// appends without asking the document) and the open list (so items can nest under the one above).
struct Writer<'a> {
    page: &'a mut PageDoc,
    children: HashMap<Option<BlockId>, usize>,
    /// The list items open from the top level down: the last one is where a nested item goes.
    list: Vec<BlockId>,
}

impl Writer<'_> {
    fn add(&mut self, rng: &mut Rng, index: usize) -> Result<(), FixtureError> {
        let (kind, text) = pick_block(rng, index);
        let is_list = matches!(
            kind,
            BlockKind::BulletedList | BlockKind::NumberedList | BlockKind::Todo { .. }
        );
        let parent = if is_list {
            // Stay in the list at this depth, go one deeper, or climb back out.
            if !self.list.is_empty() && rng.chance(25) {
                self.list.truncate(rng.below(self.list.len()));
            }
            if self.list.len() < MAX_DEPTH && rng.chance(30) {
                self.list.last().copied()
            } else {
                self.list.pop();
                self.list.last().copied()
            }
        } else {
            self.list.clear();
            None
        };
        let id = self.insert(parent, kind.clone(), text.clone())?;
        if is_list {
            self.list.push(id);
        }
        if kind.allows_marks() && !text.is_empty() && rng.chance(30) {
            self.format(rng, id, &text)?;
        }
        Ok(())
    }

    fn insert(
        &mut self,
        parent: Option<BlockId>,
        kind: BlockKind,
        text: String,
    ) -> Result<BlockId, FixtureError> {
        let index = self.children.entry(parent).or_insert(0);
        let outcome = self.page.apply(PageCommand::InsertBlock {
            parent,
            index: *index,
            kind,
            text,
        })?;
        *index += 1;
        match outcome {
            PageOutcome::BlockCreated(id) => Ok(id),
            // The model always reports the new block; anything else is a model bug.
            other => Err(FixtureError::Unexpected(format!("{other:?} for a new block"))),
        }
    }

    /// Marks one run of words bold, italic, struck, as code, or as a link.
    fn format(&mut self, rng: &mut Rng, id: BlockId, text: &str) -> Result<(), FixtureError> {
        let len = text.chars().count();
        let start = rng.below(len / 2);
        let end = (start + rng.between(3, 12)).min(len);
        let mark = match rng.below(5) {
            0 => Mark::Bold,
            1 => Mark::Italic,
            2 => Mark::Strike,
            3 => Mark::Code,
            _ => Mark::Link(format!("https://example.com/notes/{}", rng.below(1_000))),
        };
        self.page.apply(PageCommand::SetMark {
            id,
            range: start..end,
            mark,
        })?;
        Ok(())
    }
}

/// The kind and text of block number `index`: a heading opens a section every 25 to 40 blocks,
/// and the rest is mostly paragraphs and lists.
fn pick_block(rng: &mut Rng, index: usize) -> (BlockKind, String) {
    if index % 32 == 0 || rng.chance(2) {
        let level = u8::try_from(rng.between(1, 3)).unwrap_or(2);
        let count = rng.between(2, 6);
        return (BlockKind::Heading { level }, words(rng, count));
    }
    match rng.below(100) {
        0..45 => (BlockKind::Paragraph, sentence(rng, 6, 28)),
        45..60 => (BlockKind::BulletedList, sentence(rng, 3, 12)),
        60..70 => (BlockKind::NumberedList, sentence(rng, 3, 12)),
        70..82 => (
            BlockKind::Todo {
                checked: rng.chance(40),
            },
            sentence(rng, 2, 9),
        ),
        82..88 => (BlockKind::Quote, sentence(rng, 8, 20)),
        88..95 => (
            BlockKind::Code {
                language: "rust".into(),
            },
            code(rng),
        ),
        _ => (BlockKind::Divider, String::new()),
    }
}
