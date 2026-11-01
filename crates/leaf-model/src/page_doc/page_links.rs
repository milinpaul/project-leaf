//! Links to pages inside block text (ADR-071): inserting them, listing them, and the rule that keeps them
//! whole, a [`PAGE_LINK`] placeholder always carrying a `page` mark and the mark sitting only on
//! placeholders (invariant 10).
//!
//! Commands keep the rule by construction: plain text can't bring the placeholder in, replacing text never
//! carries the mark onto new characters, and the mark only re-points existing links. Only a merge with an
//! older version of Leaf, which doesn't know the mark, can break it; [`repair_links`] puts it back.
//!
//! A link to a block (E10-S08) is a page link whose placeholder also carries a `block` mark with the
//! block's ID. The same rule holds for it: the block mark only sits on a page link.

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
        block: Option<BlockId>,
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
        container.mark(pos..pos.saturating_add(1), marks::PAGE, page.to_string())?;
        if let Some(block) = block {
            container.mark(pos..pos.saturating_add(1), marks::BLOCK, block.to_string())?;
        }
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

impl PageDoc {
    /// Every link to a block on this page, as (block holding it, target page, target block), in reading
    /// order: the UI loads those pages so each link can show its block's words.
    #[must_use]
    pub fn block_links(&self) -> Vec<(BlockId, PageId, BlockId)> {
        let mut out = Vec::new();
        let mut stack: Vec<BlockId> = self.root_blocks().into_iter().rev().collect();
        while let Some(block) = stack.pop() {
            if let Ok(Some(spans)) = self.text(block) {
                for span in spans {
                    if let (Some(page), Some(target)) = (span.marks.page, span.marks.block) {
                        let count = span.text.chars().filter(|&c| c == PAGE_LINK).count();
                        out.extend(std::iter::repeat_n((block, page, target), count));
                    }
                }
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

/// One run of a block's text, as invariant 10 sees it.
struct Run {
    /// Where it starts, in characters.
    start: usize,
    /// Its characters.
    insert: String,
    /// Whether it carries a page mark.
    page: bool,
    /// Whether it carries a block mark.
    block: bool,
}

/// Each run of `text`, in order.
fn runs(text: &LoroText) -> Vec<Run> {
    let mut at = 0_usize;
    text.to_delta()
        .into_iter()
        .filter_map(|delta| match delta {
            TextDelta::Insert { insert, attributes } => {
                let start = at;
                at = at.saturating_add(insert.chars().count());
                let set = |key: &str| {
                    attributes
                        .as_ref()
                        .and_then(|attrs| attrs.get(key))
                        .is_some_and(|value| !matches!(value, LoroValue::Null))
                };
                let (page, block) = (set(marks::PAGE), set(marks::BLOCK));
                Some(Run {
                    start,
                    insert,
                    page,
                    block,
                })
            }
            _ => None,
        })
        .collect()
}

/// What breaks invariant 10 in `text`, if anything.
pub(super) fn link_problem(text: &LoroText) -> Option<String> {
    runs(text).into_iter().find_map(|run| {
        let Run {
            start,
            insert,
            page,
            block,
        } = run;
        match (page, insert.chars().all(|c| c == PAGE_LINK)) {
            (true, false) => Some(format!("page mark on text at {start}")),
            (false, _) if insert.contains(PAGE_LINK) => Some(format!("page link without a page at {start}")),
            (false, _) if block => Some(format!("block mark off a page link at {start}")),
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
    if key == marks::PAGE || key == marks::BLOCK {
        for at in range {
            text.mark(at..at.saturating_add(1), key, value.clone())?;
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

/// Puts invariant 10 back after a merge: the page and block marks come off text that isn't a placeholder,
/// the block mark off anything that isn't a page link, and placeholders without a target go (they'd be links to nothing). With `keep_links` false (code text, which
/// takes no marks) every placeholder goes. Returns whether anything changed.
pub(super) fn repair_links(text: &LoroText, keep_links: bool) -> Result<bool, ModelError> {
    let mut changed = false;
    // Back to front, so deleting a placeholder doesn't move the runs still to visit.
    for run in runs(text).into_iter().rev() {
        let chars: Vec<char> = run.insert.chars().collect();
        for (offset, &c) in chars.iter().enumerate().rev() {
            let at = run.start.saturating_add(offset);
            let here = at..at.saturating_add(1);
            if c == PAGE_LINK {
                if !keep_links || !run.page {
                    text.delete(at, 1)?;
                    changed = true;
                }
            } else {
                for (on, key) in [(run.page, marks::PAGE), (run.block, marks::BLOCK)] {
                    if on {
                        text.unmark(here.clone(), key)?;
                        changed = true;
                    }
                }
            }
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod block_tests;
#[cfg(test)]
mod tests;
