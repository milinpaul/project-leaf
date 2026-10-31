//! Reading a page's searchable text out of the documents.

use leaf_core::{BlockId, PageId};
use leaf_model::{PAGE_LINK, PageDoc, Workspace};

use crate::{BlockText, LinkText, PageText, SearchError};

/// What an untitled page is indexed as, matching what the sidebar shows, so typing "untitled"
/// finds the pages the owner sees under that name.
pub const UNTITLED: &str = "Untitled";

/// The rows to index for `page`, whose blocks are in `doc`, or `None` if it is in the trash
/// (directly or under a trashed page), since trashed pages are never searched.
///
/// Only reads: nothing in either document changes. Blocks come in document order, nested blocks
/// right after their parent. Blocks with no words (dividers, empty blocks, a lone page link) give no row. Blocks of
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
    let mut links = Vec::new();
    // Depth-first, children before later siblings: a stack holding siblings in reverse.
    let mut stack: Vec<_> = doc.root_blocks().into_iter().rev().collect();
    while let Some(block) = stack.pop() {
        // Page links are left out (ADR-071): their placeholder isn't a word, and pages are found by
        // their own titles.
        if let Some(text) = doc.plain_text(block)?.map(|text| text.replace(PAGE_LINK, ""))
            && !text.trim().is_empty()
        {
            let kind = doc.kind(block)?.type_name().to_owned();
            blocks.push(BlockText { block, kind, text });
        }
        links.extend(links_in(workspace, doc, block)?);
        stack.extend(doc.children(block)?.into_iter().rev());
    }
    Ok(Some(PageText {
        page,
        title,
        blocks,
        links,
    }))
}

/// How many characters of a block's text a backlink shows on each side of the link.
pub const LINK_CONTEXT: usize = 60;

/// The links in `block`, one per target (its first), each with the words around it.
fn links_in(workspace: &Workspace, doc: &PageDoc, block: BlockId) -> Result<Vec<LinkText>, SearchError> {
    let Some(spans) = doc.text(block)? else {
        return Ok(Vec::new());
    };
    // The block's characters, each with the page it links to (`None` for ordinary text).
    let chars: Vec<(char, Option<PageId>)> = spans
        .iter()
        .flat_map(|span| {
            span.text
                .chars()
                .map(|c| (c, span.marks.page.filter(|_| c == PAGE_LINK)))
        })
        .collect();
    // Other links in the words around a link read as their pages' titles (as they are when indexed).
    let words = |range: &[(char, Option<PageId>)]| -> String {
        range
            .iter()
            .map(|(c, page)| match page {
                Some(page) => link_name(workspace, *page),
                None if *c == PAGE_LINK => String::new(),
                None => c.to_string(),
            })
            .collect()
    };
    let mut links: Vec<LinkText> = Vec::new();
    for (at, (_, target)) in chars.iter().enumerate() {
        let Some(target) = *target else { continue };
        if links.iter().any(|link| link.target == target) {
            continue;
        }
        links.push(LinkText {
            block,
            target,
            before: tail(&words(&chars[..at])),
            after: head(&words(&chars[at + 1..])),
        });
    }
    Ok(links)
}

/// What a link to `page` reads as in a backlink's words: the page's title, "Untitled", or "Deleted page".
fn link_name(workspace: &Workspace, page: PageId) -> String {
    match workspace.page(page) {
        Ok(meta) if meta.title.trim().is_empty() => UNTITLED.to_owned(),
        Ok(meta) => meta.title,
        Err(_) => "Deleted page".to_owned(),
    }
}

/// The last [`LINK_CONTEXT`] characters of `text`, with "…" in front when it was cut.
fn tail(text: &str) -> String {
    let count = text.chars().count();
    if count <= LINK_CONTEXT {
        return text.to_owned();
    }
    let kept: String = text.chars().skip(count - LINK_CONTEXT).collect();
    format!("…{}", kept.trim_start())
}

/// The first [`LINK_CONTEXT`] characters of `text`, with "…" after when it was cut.
fn head(text: &str) -> String {
    if text.chars().count() <= LINK_CONTEXT {
        return text.to_owned();
    }
    let kept: String = text.chars().take(LINK_CONTEXT).collect();
    format!("{}…", kept.trim_end())
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
