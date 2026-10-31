//! Page links across a fixture workspace (E10-S06): each page linking to others from some of its
//! blocks, for the backlinks budget.

use leaf_core::{BlockId, PageId};
use leaf_model::{PageCommand, PageDoc};

use crate::{Fixture, FixtureError, Rng};

/// Adds `per_page` links to every page of `fixture` that has blocks, each in a random block that takes
/// links, at a random place in its text, to a random page of the fixture (never itself). Returns how
/// many links went in.
pub fn add_links(fixture: &mut Fixture, per_page: usize, seed: u64) -> Result<usize, FixtureError> {
    let mut rng = Rng::new(seed);
    let pages: Vec<PageId> = fixture.pages.iter().map(|(page, _)| *page).collect();
    let mut added = 0;
    for (page, doc) in &mut fixture.pages {
        let blocks = linkable(doc)?;
        if blocks.is_empty() || pages.len() < 2 {
            continue;
        }
        for _ in 0..per_page {
            let target = loop {
                let candidate = pages[rng.below(pages.len())];
                if candidate != *page {
                    break candidate;
                }
            };
            let block = blocks[rng.below(blocks.len())];
            let len = doc.plain_text(block)?.map_or(0, |text| text.chars().count());
            doc.apply(PageCommand::InsertPageLink {
                id: block,
                pos: rng.below(len + 1),
                page: target,
            })?;
            added += 1;
        }
    }
    Ok(added)
}

/// The blocks of `doc` whose text can hold a page link, in reading order.
fn linkable(doc: &PageDoc) -> Result<Vec<BlockId>, FixtureError> {
    let mut out = Vec::new();
    let mut stack: Vec<BlockId> = doc.root_blocks().into_iter().rev().collect();
    while let Some(block) = stack.pop() {
        let kind = doc.kind(block)?;
        if kind.has_text() && kind.allows_marks() {
            out.push(block);
        }
        stack.extend(doc.children(block)?.into_iter().rev());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::add_links;
    use crate::big_workspace;

    #[test]
    fn every_page_gets_its_links_and_none_to_itself() {
        let mut fixture = big_workspace(7, 20, 10).unwrap();
        assert_eq!(add_links(&mut fixture, 3, 7).unwrap(), 60);
        for (page, doc) in &fixture.pages {
            let links = doc.page_links();
            assert_eq!(links.len(), 3);
            assert!(links.iter().all(|(_, target)| target != page));
            assert_eq!(doc.check_invariants(), Ok(()));
        }
    }
}
