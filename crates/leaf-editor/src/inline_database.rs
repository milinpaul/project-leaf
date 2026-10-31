//! Inline databases on a page (E09-S09, ADR-066 answer 4): the page's side.
//!
//! `/database` asks for one where it was typed; the workspace makes the database page (under this page) and
//! hands it to [`PageEditor::insert_inline_database`], which places the block. The block is the database's
//! place on the page, so its database follows it: deleting the block trashes the database, and undo, which
//! brings the block back, brings the database back. [`PageEditor::inline_database_changes`] reports those
//! comings and goings for the workspace to act on.

use std::collections::HashSet;

use leaf_core::PageId;
use leaf_model::{BlockKind, Caret, PageCommand, PageDoc};

use crate::page_editor::insert_after;
use crate::{BlockStyle, EditorError, PageEditor};

/// What the page's inline databases did since they were last looked at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InlineDatabaseChanges {
    /// Databases whose block left the page (deleted, cut, or undone).
    pub gone: Vec<PageId>,
    /// Databases whose block came back (undo, redo).
    pub back: Vec<PageId>,
}

impl PageEditor {
    /// Remembers that a database was asked for at `caret` (the slash menu's "Database").
    pub(crate) fn request_database(&mut self, caret: Caret) -> Result<(), EditorError> {
        self.database_request = Some(self.doc.anchor(caret)?);
        Ok(())
    }

    /// Where a database was asked for, once: the workspace makes it and calls
    /// [`PageEditor::insert_inline_database`]. `None` with no request, or when its place is gone.
    pub fn take_database_request(&mut self) -> Result<Option<Caret>, EditorError> {
        match self.database_request.take() {
            Some(at) => Ok(self.doc.resolve(&at)?),
            None => Ok(None),
        }
    }

    /// Puts the block showing `database` at `at`: an empty text block gives way to it, with the caret on a new
    /// line below; otherwise it goes below the block (below the table, for a cell) and the caret stays. One undo
    /// step. Returns the caret.
    pub fn insert_inline_database(&mut self, at: Caret, database: PageId) -> Result<Caret, EditorError> {
        self.begin()?;
        let caret = self
            .doc
            .as_separate_step(|doc| place_database(doc, at, database))?;
        let caret = self.set_caret(caret)?;
        self.finish()?;
        if let Some(seen) = &mut self.inline_seen {
            // Made here, not brought back: nothing for the workspace to restore.
            seen.insert(database);
        }
        Ok(caret)
    }

    /// The databases shown on the page, in reading order.
    pub fn inline_databases(&mut self) -> Result<Vec<PageId>, EditorError> {
        Ok(self
            .view()?
            .iter()
            .filter_map(|row| match row.style {
                BlockStyle::Database { page } => Some(page),
                _ => None,
            })
            .collect())
    }

    /// Which inline databases left the page or came back since the last call. The first call only takes note of
    /// what is there.
    pub fn inline_database_changes(&mut self) -> Result<InlineDatabaseChanges, EditorError> {
        let now: HashSet<PageId> = self.inline_databases()?.into_iter().collect();
        let Some(before) = self.inline_seen.replace(now.clone()) else {
            return Ok(InlineDatabaseChanges::default());
        };
        let mut changes = InlineDatabaseChanges {
            gone: before.difference(&now).copied().collect(),
            back: now.difference(&before).copied().collect(),
        };
        changes.gone.sort_unstable();
        changes.back.sort_unstable();
        Ok(changes)
    }
}

fn place_database(doc: &mut PageDoc, at: Caret, database: PageId) -> Result<Caret, EditorError> {
    let block = at.block;
    let kind = doc.kind(block)?;
    let below = crate::table::table_of(doc, block)?.unwrap_or(block);
    let inserted = insert_after(doc, below, BlockKind::Database { page: database })?;
    let empty = doc.plain_text(block)?.is_some_and(|t| t.is_empty());
    let gives_way = empty
        && below == block
        && kind.has_text()
        && !matches!(kind, BlockKind::Image { .. } | BlockKind::Code { .. })
        && doc.children(block)?.is_empty();
    if !gives_way {
        return Ok(at);
    }
    doc.apply(PageCommand::DeleteBlocks { ids: vec![block] })?;
    Ok(Caret {
        block: insert_after(doc, inserted, BlockKind::Paragraph)?,
        at: 0,
    })
}

#[cfg(test)]
mod tests;
