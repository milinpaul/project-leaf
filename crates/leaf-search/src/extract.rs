//! Reading a page's searchable text out of the documents.

use leaf_core::PageId;
use leaf_model::{PageDoc, Workspace};

use crate::{BlockText, PageText, SearchError};

/// What an untitled page is indexed as, matching what the sidebar shows, so typing "untitled"
/// finds the pages the owner sees under that name.
pub const UNTITLED: &str = "Untitled";

/// The rows to index for `page`, whose blocks are in `doc`, or `None` if it is in the trash
/// (directly or under a trashed page), since trashed pages are never searched.
///
/// Only reads: nothing in either document changes. Blocks come in document order, nested blocks
/// right after their parent. Blocks with no words (dividers, empty blocks) give no row. Blocks of
/// kinds this version doesn't know are still indexed if they carry text, so a page from a newer
/// Leaf stays findable.
pub fn page_text(
    workspace: &Workspace,
    page: PageId,
    doc: &PageDoc,
) -> Result<Option<PageText>, SearchError> {
    if workspace.is_in_trash(page)? {
        return Ok(None);
    }
    let title = title(workspace, page)?;

    let mut blocks = Vec::new();
    // Depth-first, children before later siblings: a stack holding siblings in reverse.
    let mut stack: Vec<_> = doc.root_blocks().into_iter().rev().collect();
    while let Some(block) = stack.pop() {
        if let Some(text) = doc.plain_text(block)?
            && !text.trim().is_empty()
        {
            let kind = doc.kind(block)?.type_name().to_owned();
            blocks.push(BlockText { block, kind, text });
        }
        stack.extend(doc.children(block)?.into_iter().rev());
    }
    Ok(Some(PageText { page, title, blocks }))
}

/// The title `page` is indexed under: as typed, or [`UNTITLED`] if blank.
pub(crate) fn title(workspace: &Workspace, page: PageId) -> Result<String, SearchError> {
    let title = workspace.page(page)?.title;
    Ok(if title.trim().is_empty() {
        UNTITLED.to_owned()
    } else {
        title
    })
}
