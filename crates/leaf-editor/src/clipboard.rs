//! Copy, cut and paste inside Leaf: the selection as a [`Fragment`] of formatted text or whole
//! blocks, and pasting one back at the caret.
//!
//! The editor never touches the system clipboard; the UI puts [`Fragment::to_json`] there and hands
//! back what it reads (ADR 0030). Pasting is one undo step, whatever it takes:
//!
//! - **Text** from inside one block goes in at the caret, keeping its formatting.
//! - **A whole block** is copied as a block, kind and all (E17-S05): all of its text selected, or just
//!   the caret in it with nothing selected. Pasted into text it goes in as text; on an empty line it
//!   gives back the same block.
//! - **Blocks** pasted mid-text split the line: the first block's text joins the text before the
//!   caret, the other blocks follow, and the text that was after the caret ends up after the last
//!   pasted text. In an empty paragraph the first block takes its place, kind and all, so copying
//!   blocks and pasting them on an empty line gives back exactly what was copied.
//! - **Tables** (E16-S04) are built with the table commands and filled cell by cell. One pasted first
//!   goes below the line (or in place of an empty paragraph), and the text after the caret goes to a
//!   new line below it. Into a cell, a table's cells go in as lines of text.

mod fragment;
pub(crate) mod markdown;

pub use fragment::{FRAGMENT_VERSION, Fragment, FragmentBlock};

use std::ops::Range;

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, Mark, MarkKind, PAGE_LINK, PageCommand, PageDoc, TextSpan};

use crate::page_editor::{insert_after, insert_block, insert_table};
use crate::selection::delete_span;
use crate::{EditorError, Key, PageEditor, Selection};

impl PageEditor {
    /// Copy: the selection as a fragment. All of one block's text, or the caret alone with nothing
    /// selected, copies that block whole, so its kind comes along (E17-S05). `None` with neither a
    /// selection nor a caret.
    pub fn copy(&mut self) -> Result<Option<Fragment>, EditorError> {
        if let Some(fragment) = self.copied_selection()? {
            return Ok(Some(fragment));
        }
        match (self.selection()?, self.caret()?) {
            (None, Some(caret)) => Ok(Some(self.whole_block(caret.block)?)),
            _ => Ok(None),
        }
    }

    /// The selection as a fragment, `None` with nothing selected.
    fn copied_selection(&mut self) -> Result<Option<Fragment>, EditorError> {
        Ok(match self.selection()? {
            None => None,
            Some(Selection::Blocks(blocks)) => Some(Fragment::Blocks(
                blocks
                    .iter()
                    .map(|&b| subtree(&self.doc, b))
                    .collect::<Result<_, _>>()?,
            )),
            Some(Selection::Text { start, end })
                if start.block == end.block
                    && start.at == 0
                    && end.at > 0
                    && end.at == self.text_len(end.block)? =>
            {
                Some(self.whole_block(start.block)?)
            }
            Some(Selection::Text { start, end }) if start.block == end.block => Some(Fragment::Text(
                spans_in(&self.doc, start.block, start.at..end.at)?,
            )),
            Some(Selection::Text { start, end }) => Some(Fragment::Blocks(self.touched_blocks(start, end)?)),
            Some(Selection::Cells { table, cells }) => {
                Some(Fragment::Blocks(vec![crate::table::cells_as_table(
                    &self.doc, table, &cells,
                )?]))
            }
        })
    }

    /// Cut: copies the selection, then deletes it as one undo step. Unlike copy, it needs a
    /// selection: with only the caret it does nothing, so a stray Cmd+X never deletes a block.
    pub fn cut(&mut self) -> Result<Option<Fragment>, EditorError> {
        let fragment = self.copied_selection()?;
        match self.selection()? {
            Some(Selection::Blocks(blocks)) => {
                self.delete_blocks(&blocks)?;
            }
            Some(Selection::Text { .. } | Selection::Cells { .. }) => {
                self.press(Key::Delete)?;
            }
            None => {}
        }
        Ok(fragment)
    }

    /// Paste: replaces the selection (or goes in at the caret) with `fragment`, as one undo step.
    /// With whole blocks selected, the fragment goes after the last of them. Returns where the caret
    /// went: after the pasted text.
    pub fn paste(&mut self, fragment: &Fragment) -> Result<Option<Caret>, EditorError> {
        let empty = match fragment {
            Fragment::Text(spans) => spans.iter().all(|s| s.text.is_empty()),
            Fragment::Blocks(blocks) => blocks.is_empty(),
        };
        if empty {
            return self.caret();
        }
        let selection = self.selection()?;
        let Some(caret) = self.caret()? else {
            return Ok(None);
        };
        self.begin()?;
        let order = self.order()?;
        let caret = self.doc.as_one_step(|doc| -> Result<Caret, EditorError> {
            if let Some(Selection::Blocks(selected)) = &selection {
                // After the selected blocks, as blocks.
                let as_blocks = match fragment {
                    Fragment::Text(spans) => vec![FragmentBlock {
                        kind: BlockKind::Paragraph,
                        text: spans.clone(),
                        children: Vec::new(),
                    }],
                    Fragment::Blocks(blocks) => blocks.clone(),
                };
                let last = selected.last().copied().unwrap_or(caret.block);
                return paste_after(doc, last, &as_blocks);
            }
            let at = match selection {
                Some(Selection::Text { start, end }) => delete_span(doc, &order, start, end)?,
                _ => caret,
            };
            match fragment {
                Fragment::Text(spans) => {
                    let end = insert_spans(doc, at.block, at.at, spans)?;
                    Ok(Caret { at: end, ..at })
                }
                // A cell holds text only: the blocks' text goes in, a line each.
                Fragment::Blocks(blocks) if doc.kind(at.block)? == BlockKind::TableCell => {
                    let end = insert_spans(doc, at.block, at.at, &crate::table::as_lines(blocks))?;
                    Ok(Caret { at: end, ..at })
                }
                Fragment::Blocks(blocks) => paste_blocks(doc, at, blocks),
            }
        })?;
        let caret = self.set_caret(caret)?;
        self.finish()?;
        Ok(Some(caret))
    }

    /// Pastes text from another app (or a Leaf clipboard without a fragment). Markdown is converted:
    /// one line without block syntax goes into the line with its inline formatting; anything else
    /// becomes blocks, a paragraph per plain line. Into a code block it goes exactly as it is. One
    /// undo step, like [`PageEditor::paste`].
    pub fn paste_text(&mut self, text: &str) -> Result<Option<Caret>, EditorError> {
        let into_code = match (self.caret()?, self.selection()?) {
            (Some(caret), None | Some(Selection::Text { .. })) => {
                matches!(self.doc.kind(caret.block)?, BlockKind::Code { .. })
            }
            _ => false,
        };
        if !into_code && let Some(href) = crate::link::pasted_link(text) {
            return self.paste_link(text.trim(), &href);
        }
        let fragment = if into_code {
            Fragment::Text(vec![TextSpan {
                text: text.replace("\r\n", "\n"),
                marks: leaf_model::Marks::default(),
            }])
        } else {
            markdown::fragment_from_text(text)
        };
        self.paste(&fragment)
    }

    /// A lone web address pasted (E17-S10): over selected text it links that text; anywhere else it
    /// goes in as a link whose text is the address. One undo step either way.
    fn paste_link(&mut self, text: &str, href: &str) -> Result<Option<Caret>, EditorError> {
        if let Some(Selection::Text { start, end }) = self.selection()?
            && start != end
        {
            self.set_link(href)?;
            return self.caret();
        }
        self.paste(&Fragment::Text(vec![TextSpan {
            text: text.to_owned(),
            marks: leaf_model::Marks {
                link: Some(href.to_owned()),
                ..leaf_model::Marks::default()
            },
        }]))
    }

    /// `block` alone as a fragment: one block with its kind and formatted text, without what is
    /// nested in it. A table cell gives just its text: a cell can't go on the clipboard or live
    /// outside its table.
    fn whole_block(&mut self, block: BlockId) -> Result<Fragment, EditorError> {
        let len = self.text_len(block)?;
        let text = spans_in(&self.doc, block, 0..len)?;
        let kind = self.doc.kind(block)?;
        if kind.is_table_part() {
            return Ok(Fragment::Text(text));
        }
        Ok(Fragment::Blocks(vec![FragmentBlock {
            kind,
            text,
            children: Vec::new(),
        }]))
    }

    /// The blocks a cross-block text selection touches, trimmed at both ends, nested as they are
    /// relative to each other.
    fn touched_blocks(&mut self, start: Caret, end: Caret) -> Result<Vec<FragmentBlock>, EditorError> {
        let rows = self.rows()?;
        let from = rows.iter().position(|r| r.id == start.block).unwrap_or(0);
        let to = rows.iter().position(|r| r.id == end.block).unwrap_or(from);
        let rows = &rows[from..=to];
        let base = rows.iter().map(|r| r.depth).min().unwrap_or(0);
        let mut flat = Vec::new();
        for row in rows {
            let len = row.text.as_ref().map_or(0, |t| t.chars().count());
            let range = match (row.id == start.block, row.id == end.block) {
                (true, _) => start.at..len,
                (_, true) => 0..end.at,
                _ => 0..len,
            };
            let block = FragmentBlock {
                kind: self.doc.kind(row.id)?,
                text: spans_in(&self.doc, row.id, range)?,
                children: Vec::new(),
            };
            flat.push((row.depth - base, block));
        }
        Ok(nest(flat))
    }
}

/// Pastes blocks at `at`, splitting the line there (see the module docs).
fn paste_blocks(doc: &mut PageDoc, at: Caret, blocks: &[FragmentBlock]) -> Result<Caret, EditorError> {
    let kind = doc.kind(at.block)?;
    let len = doc.plain_text(at.block)?.map_or(0, |t| t.chars().count());
    // A caption doesn't take blocks in: they go after the image.
    if matches!(kind, BlockKind::Image { .. }) || !kind.has_text() {
        return paste_after(doc, at.block, blocks);
    }
    let tail = spans_in(doc, at.block, at.at..len)?;
    if at.at < len {
        doc.apply(PageCommand::DeleteText {
            id: at.block,
            range: at.at..len,
        })?;
    }
    let Some((first, rest)) = blocks.split_first() else {
        return Ok(at);
    };
    if matches!(first.kind, BlockKind::Table { .. }) {
        return paste_below_line(doc, at, blocks, &tail, len == 0 && kind == BlockKind::Paragraph);
    }
    // Only a line that is really empty takes the first block's kind: at the start of a line that has
    // text, the pasted text joins it and the line keeps its kind (E17-S05).
    let empty = len == 0 && kind == BlockKind::Paragraph;
    let mut end = Caret {
        block: at.block,
        at: at.at,
    };
    if empty {
        // The first block takes the empty line's place, kind and all.
        doc.apply(PageCommand::SetBlockKind {
            id: at.block,
            kind: first.kind.clone(),
        })?;
    }
    if doc.kind(at.block)?.has_text() {
        end.at = insert_spans(doc, at.block, at.at, &first.text)?;
    }
    let mut last = Placed {
        block: at.block,
        end: text_end(doc, at.block, end.at)?,
    };
    insert_children(doc, at.block, &first.children, &mut last)?;
    for block in rest {
        let below = top_level_after(doc, at.block, last.block)?;
        insert_tree(doc, below.0, below.1, block, &mut last)?;
    }
    finish_with_tail(doc, &last, &tail)
}

/// Pastes blocks starting with a table, which can't join a line's text: they go after the line, and
/// the text that was after the caret follows them. An empty paragraph with nothing nested in it is
/// replaced instead.
fn paste_below_line(
    doc: &mut PageDoc,
    at: Caret,
    blocks: &[FragmentBlock],
    tail: &[TextSpan],
    empty_line: bool,
) -> Result<Caret, EditorError> {
    let mut last = Placed {
        block: at.block,
        end: None,
    };
    for block in blocks {
        let (parent, index) = top_level_after(doc, at.block, last.block)?;
        insert_tree(doc, parent, index, block, &mut last)?;
    }
    if empty_line && doc.children(at.block)?.is_empty() {
        doc.apply(PageCommand::DeleteBlocks { ids: vec![at.block] })?;
    }
    finish_with_tail(doc, &last, tail)
}

/// Pastes blocks right after `after` (and what is nested in it).
fn paste_after(doc: &mut PageDoc, after: BlockId, blocks: &[FragmentBlock]) -> Result<Caret, EditorError> {
    let mut last = Placed {
        block: after,
        end: None,
    };
    for block in blocks {
        let (parent, index) = top_level_after(doc, after, last.block)?;
        insert_tree(doc, parent, index, block, &mut last)?;
    }
    finish_with_tail(doc, &last, &[])
}

/// The last pasted block in reading order, and where its text ends if it takes text.
struct Placed {
    block: BlockId,
    end: Option<usize>,
}

/// Puts the text that was after the caret at the end of the last pasted text, or in a new paragraph
/// when the last pasted block holds no text (a divider, an image). The caret goes before it.
fn finish_with_tail(doc: &mut PageDoc, last: &Placed, tail: &[TextSpan]) -> Result<Caret, EditorError> {
    if let Some(at) = last.end {
        insert_spans(doc, last.block, at, tail)?;
        return Ok(Caret {
            block: last.block,
            at,
        });
    }
    let block = insert_after(doc, last.block, BlockKind::Paragraph)?;
    insert_spans(doc, block, 0, tail)?;
    Ok(Caret { block, at: 0 })
}

/// Where the next top-level pasted block goes: right after the previous one, at the level of `start`.
fn top_level_after(
    doc: &PageDoc,
    start: BlockId,
    previous: BlockId,
) -> Result<(Option<BlockId>, usize), EditorError> {
    let parent = doc.parent(start)?;
    // `previous` may be nested deep inside the last pasted block: climb to the level of `start`.
    let mut top = previous;
    while doc.parent(top)? != parent {
        match doc.parent(top)? {
            Some(up) => top = up,
            None => break,
        }
    }
    let siblings = match parent {
        Some(p) => doc.children(p)?,
        None => doc.root_blocks(),
    };
    let index = siblings
        .iter()
        .position(|&b| b == top)
        .map_or(siblings.len(), |p| p + 1);
    Ok((parent, index))
}

/// Creates `block` and everything nested in it at `index` under `parent`.
fn insert_tree(
    doc: &mut PageDoc,
    parent: Option<BlockId>,
    index: usize,
    block: &FragmentBlock,
    last: &mut Placed,
) -> Result<usize, EditorError> {
    if let BlockKind::Table { header_row } = block.kind {
        let table = paste_table(doc, parent, index, block, header_row)?;
        *last = Placed {
            block: table,
            end: None,
        };
        return Ok(index + 1);
    }
    let id = insert_block(doc, parent, index, block.kind.clone())?;
    let end = if block.kind.has_text() {
        insert_spans(doc, id, 0, &block.text)?
    } else {
        0
    };
    *last = Placed {
        block: id,
        end: text_end(doc, id, end)?,
    };
    if block.kind.allows_children() {
        insert_children(doc, id, &block.children, last)?;
        Ok(index + 1)
    } else {
        // Children under a kind that can't hold them: they follow it instead.
        let mut next = index + 1;
        for child in &block.children {
            next = insert_tree(doc, parent, next, child, last)?;
        }
        Ok(next)
    }
}

/// Builds the table `block` describes under `parent` at `index`: as many rows as it has, as many
/// columns as its longest row (short rows get empty cells), each cell's formatted text filled in.
fn paste_table(
    doc: &mut PageDoc,
    parent: Option<BlockId>,
    index: usize,
    block: &FragmentBlock,
    header_row: bool,
) -> Result<BlockId, EditorError> {
    let rows: Vec<&[FragmentBlock]> = block
        .children
        .iter()
        .filter(|row| row.kind == BlockKind::TableRow)
        .map(|row| row.children.as_slice())
        .collect();
    let columns = rows.iter().map(|row| row.len()).max().unwrap_or(0).max(1);
    let table = insert_table(doc, parent, index, (rows.len().max(1), columns), header_row)?;
    let grid = doc.table_cells(table)?;
    for (cells, ids) in rows.iter().zip(&grid) {
        for (cell, &id) in cells.iter().zip(ids) {
            insert_spans(doc, id, 0, &cell.text)?;
        }
    }
    Ok(table)
}

fn insert_children(
    doc: &mut PageDoc,
    parent: BlockId,
    children: &[FragmentBlock],
    last: &mut Placed,
) -> Result<(), EditorError> {
    if children.is_empty() {
        return Ok(());
    }
    if !doc.kind(parent)?.allows_children() {
        let (grand, index) = top_level_after(doc, parent, parent)?;
        let mut next = index;
        for child in children {
            next = insert_tree(doc, grand, next, child, last)?;
        }
        return Ok(());
    }
    let mut index = doc.children(parent)?.len();
    for child in children {
        index = insert_tree(doc, Some(parent), index, child, last)?;
    }
    Ok(())
}

/// Where typed text would continue in `block`: `Some(end)`, or `None` for blocks whose text isn't a
/// place to continue (dividers, image captions).
fn text_end(doc: &PageDoc, block: BlockId, end: usize) -> Result<Option<usize>, EditorError> {
    let kind = doc.kind(block)?;
    Ok((kind.has_text() && !matches!(kind, BlockKind::Image { .. })).then_some(end))
}

/// Inserts formatted text at `pos`, each run with exactly its own marks (none of the neighbours'),
/// and returns where it ends. Code blocks take the text without marks.
pub(crate) fn insert_spans(
    doc: &mut PageDoc,
    block: BlockId,
    mut pos: usize,
    spans: &[TextSpan],
) -> Result<usize, EditorError> {
    let marks_allowed = doc.kind(block)?.allows_marks();
    for span in spans.iter().filter(|s| !s.text.is_empty()) {
        // A page link pastes as a link where marks are allowed; a placeholder without a page (or in code)
        // is a link to nothing and is left out (ADR 0071).
        let link = span.marks.page.filter(|_| marks_allowed);
        let text: String = span.text.chars().filter(|&c| c != PAGE_LINK).collect();
        let len = match link {
            Some(page) => {
                let count = span.text.chars().filter(|&c| c == PAGE_LINK).count();
                for at in pos..pos + count {
                    doc.apply(PageCommand::InsertPageLink {
                        id: block,
                        pos: at,
                        page,
                    })?;
                }
                count
            }
            None if text.is_empty() => continue,
            None => {
                doc.apply(PageCommand::InsertText {
                    id: block,
                    pos,
                    text: text.clone(),
                })?;
                text.chars().count()
            }
        };
        if marks_allowed {
            let range = pos..pos + len;
            let flags = [
                (MarkKind::Bold, span.marks.bold.then_some(Mark::Bold)),
                (MarkKind::Italic, span.marks.italic.then_some(Mark::Italic)),
                (MarkKind::Strike, span.marks.strike.then_some(Mark::Strike)),
                (MarkKind::Code, span.marks.code.then_some(Mark::Code)),
                (MarkKind::Link, span.marks.link.clone().map(Mark::Link)),
            ];
            for (kind, mark) in flags {
                let range = range.clone();
                doc.apply(match mark {
                    Some(mark) => PageCommand::SetMark {
                        id: block,
                        range,
                        mark,
                    },
                    None => PageCommand::RemoveMark {
                        id: block,
                        range,
                        kind,
                    },
                })?;
            }
        }
        pos += len;
    }
    Ok(pos)
}

/// The formatted text of `range` in `block`, as runs.
fn spans_in(doc: &PageDoc, block: BlockId, range: Range<usize>) -> Result<Vec<TextSpan>, EditorError> {
    let mut out = Vec::new();
    let mut offset = 0;
    for span in doc.text(block)?.unwrap_or_default() {
        let len = span.text.chars().count();
        let (from, to) = (range.start.max(offset), range.end.min(offset + len));
        if from < to {
            out.push(TextSpan {
                text: span.text.chars().skip(from - offset).take(to - from).collect(),
                marks: span.marks,
            });
        }
        offset += len;
    }
    Ok(out)
}

/// `block` with all its text and everything nested in it.
fn subtree(doc: &PageDoc, block: BlockId) -> Result<FragmentBlock, EditorError> {
    Ok(FragmentBlock {
        kind: doc.kind(block)?,
        text: doc.text(block)?.unwrap_or_default(),
        children: doc
            .children(block)?
            .into_iter()
            .map(|child| subtree(doc, child))
            .collect::<Result<_, _>>()?,
    })
}

/// Builds trees from (depth, block) rows in reading order: each block nests under the nearest
/// shallower block before it.
fn nest(rows: Vec<(usize, FragmentBlock)>) -> Vec<FragmentBlock> {
    fn take(
        rows: &mut std::iter::Peekable<std::vec::IntoIter<(usize, FragmentBlock)>>,
        depth: usize,
    ) -> Vec<FragmentBlock> {
        let mut out = Vec::new();
        while let Some((d, _)) = rows.peek() {
            if *d < depth {
                break;
            }
            let Some((d, mut block)) = rows.next() else {
                break;
            };
            // Keep what the block already holds (a table's rows) ahead of what nests under it.
            let nested = take(rows, d + 1);
            block.children.extend(nested);
            out.push(block);
        }
        out
    }
    let mut rows = rows.into_iter().peekable();
    take(&mut rows, 0)
}

#[cfg(test)]
mod tests;
