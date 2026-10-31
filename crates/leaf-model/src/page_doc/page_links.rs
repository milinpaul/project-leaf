//! Links to pages inside block text (ADR 0071): inserting them, listing them, and the rule that keeps them
//! whole, a [`PAGE_LINK`] placeholder always carrying a `page` mark and the mark sitting only on
//! placeholders (invariant 10).
//!
//! Commands keep the rule by construction: plain text can't bring the placeholder in, replacing text never
//! carries the mark onto new characters, and the mark only re-points existing links. Only a merge with an
//! older version of Leaf, which doesn't know the mark, can break it; [`repair_links`] puts it back.

use leaf_core::{BlockId, PageId};
use loro::{LoroText, LoroValue, TextDelta};

use super::PageDoc;
use super::text_ops::check_range;
use crate::text::{PAGE_LINK, marks};
use crate::{ModelError, PageOutcome};

impl PageDoc {
    pub(super) fn insert_page_link(
        &mut self,
        id: BlockId,
        pos: usize,
        page: PageId,
    ) -> Result<PageOutcome, ModelError> {
        let container = self.editable_text(id)?;
        let kind = self.kind(id)?;
        if !kind.allows_marks() {
            return Err(ModelError::MarksNotAllowed {
                kind: kind.type_name().to_owned(),
            });
        }
        check_range(&container, pos..pos)?;
        container.insert(pos, &PAGE_LINK.to_string())?;
        container.mark(pos..pos + 1, marks::PAGE, page.to_string())?;
        Ok(PageOutcome::Changed)
    }

    /// Every link to a page on this page, as (block, target), in reading order. Placeholders whose
    /// target doesn't read as a page ID are left out (they are kept in the document all the same).
    #[must_use]
    pub fn page_links(&self) -> Vec<(BlockId, PageId)> {
        let mut out = Vec::new();
        let mut stack: Vec<BlockId> = self.root_blocks().into_iter().rev().collect();
        while let Some(block) = stack.pop() {
            if let Ok(Some(text)) = self.text_container(block) {
                out.extend(link_targets(&text).into_iter().map(|page| (block, page)));
            }
            stack.extend(self.children(block).unwrap_or_default().into_iter().rev());
        }
        out
    }
}

/// The targets of the links in `text`, in order.
fn link_targets(text: &LoroText) -> Vec<PageId> {
    text.to_delta()
        .into_iter()
        .filter_map(|delta| match delta {
            TextDelta::Insert {
                insert,
                attributes: Some(attributes),
            } => match attributes.get(marks::PAGE) {
                Some(LoroValue::String(page)) => {
                    let page = page.parse::<PageId>().ok()?;
                    Some(vec![page; insert.chars().filter(|&c| c == PAGE_LINK).count()])
                }
                _ => None,
            },
            _ => None,
        })
        .flatten()
        .collect()
}

/// Refuses plain text holding the placeholder: it would be a link to nothing.
pub(super) fn check_plain(text: &str) -> Result<(), ModelError> {
    if text.contains(PAGE_LINK) {
        return Err(ModelError::PageLinkCharInText);
    }
    Ok(())
}

/// Each run of `text` as (start, length in characters, page mark value), in order.
fn runs(text: &LoroText) -> Vec<(usize, String, Option<LoroValue>)> {
    let mut at = 0;
    text.to_delta()
        .into_iter()
        .filter_map(|delta| match delta {
            TextDelta::Insert { insert, attributes } => {
                let start = at;
                at += insert.chars().count();
                let page = attributes
                    .and_then(|mut attrs| attrs.remove(marks::PAGE))
                    .filter(|value| !matches!(value, LoroValue::Null));
                Some((start, insert, page))
            }
            _ => None,
        })
        .collect()
}

/// What breaks invariant 10 in `text`, if anything.
pub(super) fn link_problem(text: &LoroText) -> Option<String> {
    runs(text).into_iter().find_map(|(start, insert, page)| {
        match (page.is_some(), insert.chars().all(|c| c == PAGE_LINK)) {
            (true, false) => Some(format!("page mark on text at {start}")),
            (false, _) if insert.contains(PAGE_LINK) => Some(format!("page link without a page at {start}")),
            _ => None,
        }
    })
}

/// Marks `range` of `text` with `key`. A page mark goes on each placeholder separately: Loro gives text typed
/// *inside* one mark's span that mark, whatever its expand rule, so two neighbouring links marked at once
/// would take in text typed between them.
pub(super) fn mark_run(
    text: &LoroText,
    range: std::ops::Range<usize>,
    key: &str,
    value: LoroValue,
) -> Result<(), ModelError> {
    if key == marks::PAGE {
        for at in range {
            text.mark(at..at + 1, key, value.clone())?;
        }
    } else {
        text.mark(range, key, value)?;
    }
    Ok(())
}

/// Whether every character in `range` of `text` is a placeholder (so the page mark may go there).
pub(super) fn all_placeholders(text: &LoroText, range: std::ops::Range<usize>) -> bool {
    text.to_string()
        .chars()
        .skip(range.start)
        .take(range.len())
        .all(|c| c == PAGE_LINK)
}

/// Puts invariant 10 back after a merge: the page mark comes off text that isn't a placeholder, and
/// placeholders without a target go (they'd be links to nothing). With `keep_links` false (code text, which
/// takes no marks) every placeholder goes. Returns whether anything changed.
pub(super) fn repair_links(text: &LoroText, keep_links: bool) -> Result<bool, ModelError> {
    let mut changed = false;
    // Back to front, so deleting a placeholder doesn't move the runs still to visit.
    for (start, insert, page) in runs(text).into_iter().rev() {
        let mut offset = insert.chars().count();
        for c in insert.chars().rev() {
            offset -= 1;
            let at = start + offset;
            if c == PAGE_LINK {
                if !keep_links || page.is_none() {
                    text.delete(at, 1)?;
                    changed = true;
                }
            } else if page.is_some() {
                text.unmark(at..at + 1, marks::PAGE)?;
                changed = true;
            }
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests;
