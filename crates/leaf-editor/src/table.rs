//! Editing inside tables (E16-S02): moving between cells, the keys that act differently in a cell,
//! and selections that cover whole cells.
//!
//! A cell is a text block (ADR 0061), so typing, marks, Left and Right, undo and the caret already
//! work in it. What differs: Tab moves between cells instead of nesting (in the last cell it adds a
//! row), Enter types a line break, Backspace and Delete never merge a cell into its neighbour, Up and
//! Down keep to the column, and a selection from one cell into another covers the rectangle of cells
//! between them. Commands that would split, merge, convert or move a cell are never issued; anything
//! that acts on blocks acts on the whole table instead.

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, Marks, PageCommand, PageDoc, TextSpan};

use crate::{EditorError, Fragment, FragmentBlock, Key, KeyOutcome, PageEditor};

/// Where a cell sits in its table.
pub(crate) struct CellPlace {
    /// The table.
    pub table: BlockId,
    /// Row and column, from 0.
    pub row: usize,
    pub column: usize,
    /// The table's cells, row by row.
    pub grid: Vec<Vec<BlockId>>,
}

/// Where `block` sits if it is a table cell.
pub(crate) fn cell_place(doc: &PageDoc, block: BlockId) -> Result<Option<CellPlace>, EditorError> {
    let Some(table) = table_of(doc, block)? else {
        return Ok(None);
    };
    if doc.kind(block)? != BlockKind::TableCell {
        return Ok(None);
    }
    let grid = doc.table_cells(table)?;
    for (row, cells) in grid.iter().enumerate() {
        if let Some(column) = cells.iter().position(|&c| c == block) {
            return Ok(Some(CellPlace {
                table,
                row,
                column,
                grid,
            }));
        }
    }
    Ok(None)
}

/// The table a row or cell belongs to; `None` for any other block.
pub(crate) fn table_of(doc: &PageDoc, block: BlockId) -> Result<Option<BlockId>, EditorError> {
    let mut current = block;
    if !doc.kind(current)?.is_table_part() {
        return Ok(None);
    }
    while let Some(parent) = doc.parent(current)? {
        if matches!(doc.kind(parent)?, BlockKind::Table { .. }) {
            return Ok(Some(parent));
        }
        current = parent;
    }
    Ok(None)
}

/// `blocks` with every row or cell replaced by its table, without repeats, in the given order: what
/// block-level actions (nesting, the block menu, dragging) act on.
pub(crate) fn whole_tables(doc: &PageDoc, blocks: &[BlockId]) -> Result<Vec<BlockId>, EditorError> {
    let mut out: Vec<BlockId> = Vec::with_capacity(blocks.len());
    for &block in blocks {
        let block = table_of(doc, block)?.unwrap_or(block);
        if !out.contains(&block) {
            out.push(block);
        }
    }
    Ok(out)
}

/// The cells from `a` to `b` when both are cells of one table: the rectangle they span, row by row.
/// `None` otherwise.
pub(crate) fn cell_rect(
    doc: &PageDoc,
    a: BlockId,
    b: BlockId,
) -> Result<Option<(BlockId, Vec<BlockId>)>, EditorError> {
    let (Some(from), Some(to)) = (cell_place(doc, a)?, cell_place(doc, b)?) else {
        return Ok(None);
    };
    if from.table != to.table {
        return Ok(None);
    }
    let rows = from.row.min(to.row)..=from.row.max(to.row);
    let columns = from.column.min(to.column)..=from.column.max(to.column);
    let cells = from.grid[rows]
        .iter()
        .flat_map(|row| row.iter().enumerate())
        .filter(|(column, _)| columns.contains(column))
        .map(|(_, &cell)| cell)
        .collect();
    Ok(Some((from.table, cells)))
}

/// Empties every cell in `cells`; returns the start of the first.
pub(crate) fn clear_cells(doc: &mut PageDoc, cells: &[BlockId]) -> Result<Option<Caret>, EditorError> {
    for &cell in cells {
        let len = text_len(doc, cell)?;
        if len > 0 {
            doc.apply(PageCommand::DeleteText {
                id: cell,
                range: 0..len,
            })?;
        }
    }
    Ok(cells.first().map(|&block| Caret { block, at: 0 }))
}

/// `fragment` with every table turned into one paragraph per cell, row by row, keeping the cells'
/// formatting: what pasting does with a table until tables go on the clipboard (E16-S04).
pub(crate) fn without_tables(fragment: &Fragment) -> Fragment {
    fn flatten(blocks: &[FragmentBlock], out: &mut Vec<FragmentBlock>) {
        for block in blocks {
            match block.kind {
                BlockKind::Table { .. } | BlockKind::TableRow => flatten(&block.children, out),
                BlockKind::TableCell => out.push(FragmentBlock {
                    kind: BlockKind::Paragraph,
                    text: block.text.clone(),
                    children: Vec::new(),
                }),
                _ => {
                    let mut children = Vec::new();
                    flatten(&block.children, &mut children);
                    out.push(FragmentBlock {
                        kind: block.kind.clone(),
                        text: block.text.clone(),
                        children,
                    });
                }
            }
        }
    }
    match fragment {
        Fragment::Text(_) => fragment.clone(),
        Fragment::Blocks(blocks) => {
            let mut out = Vec::new();
            flatten(blocks, &mut out);
            Fragment::Blocks(out)
        }
    }
}

/// The text of `blocks` and everything nested in them, one line per block, formatting kept: what
/// pasting blocks into a cell puts there.
pub(crate) fn as_lines(blocks: &[FragmentBlock]) -> Vec<TextSpan> {
    fn walk(blocks: &[FragmentBlock], out: &mut Vec<TextSpan>, first: &mut bool) {
        for block in blocks {
            if !std::mem::take(first) {
                out.push(TextSpan {
                    text: "\n".into(),
                    marks: Marks::default(),
                });
            }
            out.extend(block.text.iter().cloned());
            walk(&block.children, out, first);
        }
    }
    let mut out = Vec::new();
    walk(blocks, &mut out, &mut true);
    out
}

fn text_len(doc: &PageDoc, block: BlockId) -> Result<usize, EditorError> {
    Ok(doc.plain_text(block)?.map_or(0, |t| t.chars().count()))
}

impl PageEditor {
    /// The keys that act differently in a cell (see the module docs), with no text selected across
    /// cells. `None` when the caret isn't in a cell or the key is the usual one there (Backspace and
    /// Delete inside the text).
    pub(crate) fn cell_key(&mut self, key: Key, caret: Caret) -> Result<Option<KeyOutcome>, EditorError> {
        let Some(place) = cell_place(&self.doc, caret.block)? else {
            return Ok(None);
        };
        let len = text_len(&self.doc, caret.block)?;
        let end_of = |block| -> Result<Caret, EditorError> {
            Ok(Caret {
                block,
                at: text_len(&self.doc, block)?,
            })
        };
        let flat: Vec<BlockId> = place.grid.concat();
        let here = flat.iter().position(|&c| c == caret.block).unwrap_or(0);
        let outcome = match key {
            Key::Tab => match flat.get(here + 1) {
                Some(&next) => end_of(next)?,
                None => self.add_row_below(place.table, place.grid.len())?,
            },
            Key::ShiftTab => match here.checked_sub(1).and_then(|i| flat.get(i)) {
                Some(&previous) => end_of(previous)?,
                None => caret,
            },
            Key::Enter | Key::ModEnter => {
                let range = self.text_range()?.map_or(caret.at..caret.at, |s| s.range);
                let at = range.start + 1;
                self.doc.apply(PageCommand::ReplaceText {
                    id: caret.block,
                    range,
                    text: "\n".into(),
                })?;
                Caret { at, ..caret }
            }
            Key::Backspace if caret.at == 0 && self.text_range()?.is_none() => {
                if !self.table_is_empty(&flat)? {
                    return Ok(Some(KeyOutcome::Caret(caret)));
                }
                return Ok(Some(match self.delete_blocks(&[place.table])? {
                    Some(at) => KeyOutcome::Caret(at),
                    None => KeyOutcome::Nothing,
                }));
            }
            Key::Delete if caret.at == len && self.text_range()?.is_none() => caret,
            Key::Backspace | Key::Delete => return Ok(None),
        };
        Ok(Some(KeyOutcome::Caret(outcome)))
    }

    /// Adds a row at the bottom of `table` (which has `rows` rows) and returns the start of its first
    /// cell.
    fn add_row_below(&mut self, table: BlockId, rows: usize) -> Result<Caret, EditorError> {
        self.doc
            .apply(PageCommand::InsertTableRow { table, index: rows })?;
        let first = self
            .doc
            .table_cells(table)?
            .last()
            .and_then(|row| row.first().copied());
        match first {
            Some(block) => Ok(Caret { block, at: 0 }),
            None => Err(EditorError::Model(leaf_model::ModelError::EmptyTable)),
        }
    }

    fn table_is_empty(&self, cells: &[BlockId]) -> Result<bool, EditorError> {
        for &cell in cells {
            if text_len(&self.doc, cell)? > 0 {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
