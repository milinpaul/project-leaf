//! The page as the screen sees it: blocks flattened in reading order, each with its depth, style
//! (list numbers included) and formatted text.
//!
//! Reading text out of the document is the costly part, so [`PageView`] keeps each block's content
//! and, on [`PageView::refresh`], re-reads only the blocks the page reports as changed. When blocks
//! were created, moved or deleted, or a block changed kind, it walks the outline again for order,
//! depth and numbering, still reusing the text of untouched blocks. Typing in one block therefore
//! costs one block's read, however long the page is.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use leaf_core::{AssetId, BlockId};
use leaf_model::{BlockKind, ModelError, PageDoc, TextSpan};

/// How to draw a block. Mirrors `BlockKind`, but with what the screen needs: list numbers are worked
/// out here, never stored.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockStyle {
    /// Plain text.
    Paragraph,
    /// Heading 1 to 3.
    Heading(u8),
    /// A bullet point.
    Bulleted,
    /// A numbered item, with its number among consecutive numbered siblings.
    Numbered(usize),
    /// A checkbox item.
    Todo {
        /// Whether it is ticked.
        checked: bool,
    },
    /// A quotation.
    Quote,
    /// Code, shown in a monospace font.
    Code {
        /// Language label; may be empty.
        language: String,
    },
    /// A horizontal line.
    Divider,
    /// A picture from the workspace's asset store, with its caption as the row's text.
    Image {
        /// Which stored picture.
        asset: AssetId,
        /// The width the owner resized it to, in logical pixels; `None` for its natural size.
        width: Option<f64>,
        /// Description for screen readers.
        alt: String,
    },
    /// A kind from a newer version, drawn read-only.
    Unknown {
        /// The stored type name.
        type_name: String,
    },
}

impl BlockStyle {
    /// The style for `kind`; `number` is used only by numbered items.
    fn of(kind: &BlockKind, number: usize) -> Self {
        match kind.clone() {
            BlockKind::Paragraph => Self::Paragraph,
            BlockKind::Heading { level } => Self::Heading(level),
            BlockKind::BulletedList => Self::Bulleted,
            BlockKind::NumberedList => Self::Numbered(number),
            BlockKind::Todo { checked } => Self::Todo { checked },
            BlockKind::Quote => Self::Quote,
            BlockKind::Code { language } => Self::Code { language },
            BlockKind::Divider => Self::Divider,
            BlockKind::Image { asset, width, alt } => Self::Image { asset, width, alt },
            BlockKind::Unknown { type_name } => Self::Unknown { type_name },
            // Drawn as a read-only placeholder until the table UI (E16-S03); its rows and cells aren't
            // listed.
            table @ (BlockKind::Table { .. } | BlockKind::TableRow | BlockKind::TableCell) => Self::Unknown {
                type_name: table.type_name().to_owned(),
            },
        }
    }
}

/// One block, ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockRow {
    /// The block.
    pub id: BlockId,
    /// Nesting depth: 0 for top-level blocks.
    pub depth: usize,
    /// How to draw it.
    pub style: BlockStyle,
    /// Its plain text, or `None` for blocks without text.
    pub text: Option<String>,
    /// Its text as runs of equal formatting, or `None` for blocks without text.
    pub spans: Option<Vec<TextSpan>>,
}

/// What a block shows, wherever it sits: the part worth caching.
#[derive(Debug, Clone)]
struct Content {
    kind: BlockKind,
    spans: Option<Vec<TextSpan>>,
}

impl Content {
    fn read(doc: &PageDoc, id: BlockId) -> Result<Self, ModelError> {
        Ok(Self {
            kind: doc.kind(id)?,
            spans: doc.text(id)?,
        })
    }

    fn row(&self, id: BlockId, depth: usize, number: usize) -> BlockRow {
        BlockRow {
            id,
            depth,
            style: BlockStyle::of(&self.kind, number),
            text: self
                .spans
                .as_ref()
                .map(|spans| spans.iter().map(|s| s.text.as_str()).collect()),
            spans: self.spans.clone(),
        }
    }
}

/// A page's rows, kept up to date with as little reading as possible.
#[derive(Debug, Default)]
pub struct PageView {
    rows: Vec<BlockRow>,
    content: HashMap<BlockId, Content>,
    /// Where each block's row is in `rows`.
    position: HashMap<BlockId, usize>,
    built: bool,
    /// Blocks read from the document by the last refresh; tests check it stays small.
    reads: usize,
}

impl PageView {
    /// A full view of `doc`, built from scratch. Leaves the page's change record alone, so it can
    /// sit beside a view kept with [`PageView::refresh`].
    pub fn build(doc: &PageDoc) -> Result<Self, ModelError> {
        let mut view = Self::default();
        view.layout(doc)?;
        view.built = true;
        Ok(view)
    }

    /// The rows, in reading order.
    #[must_use]
    pub fn rows(&self) -> &[BlockRow] {
        &self.rows
    }

    /// Brings the rows up to date with what changed on `doc` since the last refresh. The first
    /// refresh builds everything.
    pub fn refresh(&mut self, doc: &PageDoc) -> Result<(), ModelError> {
        let changes = doc.take_changes();
        self.reads = 0;
        if !self.built {
            self.content.clear();
            self.layout(doc)?;
            self.built = true;
            return Ok(());
        }
        let mut kind_changed = false;
        for &id in &changes.blocks {
            let fresh = Content::read(doc, id)?;
            self.reads += 1;
            kind_changed |= self.content.get(&id).is_none_or(|old| old.kind != fresh.kind);
            self.content.insert(id, fresh);
        }
        if changes.structure || kind_changed {
            return self.layout(doc);
        }
        for id in changes.blocks {
            if let (Some(&at), Some(content)) = (self.position.get(&id), self.content.get(&id)) {
                let row = &mut self.rows[at];
                *row = content.row(id, row.depth, numbered(&row.style));
            }
        }
        Ok(())
    }

    /// Walks the outline for order, depth and list numbers, reading only blocks not cached yet.
    fn layout(&mut self, doc: &PageDoc) -> Result<(), ModelError> {
        let mut rows = Vec::with_capacity(doc.len());
        let mut seen = HashSet::with_capacity(doc.len());
        self.walk(doc, doc.root_blocks(), 0, &mut rows, &mut seen)?;
        self.content.retain(|id, _| seen.contains(id));
        self.position = rows.iter().enumerate().map(|(at, row)| (row.id, at)).collect();
        self.rows = rows;
        Ok(())
    }

    fn walk(
        &mut self,
        doc: &PageDoc,
        blocks: Vec<BlockId>,
        depth: usize,
        rows: &mut Vec<BlockRow>,
        seen: &mut HashSet<BlockId>,
    ) -> Result<(), ModelError> {
        // Consecutive numbered siblings count up; anything else between them starts again at 1.
        let mut number = 0;
        for id in blocks {
            let content = match self.content.entry(id) {
                Entry::Occupied(cached) => cached.into_mut(),
                Entry::Vacant(slot) => {
                    self.reads += 1;
                    slot.insert(Content::read(doc, id)?)
                }
            };
            number = if content.kind == BlockKind::NumberedList {
                number + 1
            } else {
                0
            };
            let table = matches!(content.kind, BlockKind::Table { .. });
            rows.push(content.row(id, depth, number));
            seen.insert(id);
            if !table {
                self.walk(doc, doc.children(id)?, depth + 1, rows, seen)?;
            }
        }
        Ok(())
    }
}

/// The list number in `style`, or 0.
fn numbered(style: &BlockStyle) -> usize {
    match style {
        BlockStyle::Numbered(n) => *n,
        _ => 0,
    }
}

#[cfg(test)]
mod tests;
