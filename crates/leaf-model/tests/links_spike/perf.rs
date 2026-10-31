//! How long finding every page link in a 5,000-page workspace takes, and a backlinks lookup. Ignored by
//! default; run in release:
//!
//! ```text
//! cargo nextest run -p leaf-model --release --test links_spike --run-ignored only --no-capture
//! ```

use std::collections::HashMap;
use std::time::Instant;

use loro::{
    Container, ExportMode, LoroDoc, LoroError, LoroResult, LoroValue, TextDelta, TreeParentId,
    ValueOrContainer,
};

use crate::{ATOM, configure};

const PAGES: usize = 5_000;
const BLOCKS: usize = 20;
/// One block in this many holds a link (so 25,000 links in all).
const EVERY: usize = 4;

/// A page doc shaped like the real one: a `blocks` tree whose nodes hold `id`, `type` and `text`.
fn page(seed: usize) -> LoroResult<LoroDoc> {
    let doc = LoroDoc::new();
    configure(&doc, true);
    let tree = doc.get_tree("blocks");
    tree.enable_fractional_index(0);
    for b in 0..BLOCKS {
        let node = tree.create(TreeParentId::Root)?;
        let meta = tree.get_meta(node)?;
        meta.insert("id", format!("block-{b}"))?;
        meta.insert("type", "paragraph")?;
        let text = meta.ensure_mergeable_text("text")?;
        text.insert(
            0,
            "A sentence of about the usual length for a block of notes, with  in the middle.",
        )?;
        if (seed + b).is_multiple_of(EVERY) {
            text.insert(66, &ATOM.to_string())?;
            text.mark(66..67, "page", format!("page-{}", (seed * 31 + b * 7) % PAGES))?;
        }
    }
    doc.commit();
    Ok(doc)
}

/// Every (block, target page) link in one page doc.
fn links(doc: &LoroDoc) -> LoroResult<Vec<(String, String)>> {
    let tree = doc.get_tree("blocks");
    let mut out = Vec::new();
    for node in tree.nodes() {
        let meta = tree.get_meta(node)?;
        let Some(ValueOrContainer::Container(Container::Text(text))) = meta.get("text") else {
            continue;
        };
        let block = match meta.get("id") {
            Some(ValueOrContainer::Value(LoroValue::String(id))) => id.to_string(),
            _ => return Err(LoroError::ArgErr("block without id".into())),
        };
        for delta in text.to_delta() {
            if let TextDelta::Insert {
                attributes: Some(attributes),
                ..
            } = delta
                && let Some(LoroValue::String(page)) = attributes.get("page")
            {
                out.push((block.clone(), page.to_string()));
            }
        }
    }
    Ok(out)
}

#[test]
#[ignore = "measurement; run in release, see the module docs"]
fn finding_every_link_in_5000_pages() -> LoroResult<()> {
    let snapshots: Vec<Vec<u8>> = (0..PAGES)
        .map(|seed| {
            page(seed)?
                .export(ExportMode::Snapshot)
                .map_err(|e| LoroError::ArgErr(e.to_string().into()))
        })
        .collect::<LoroResult<_>>()?;

    // Full rebuild: load every page doc and scan it (what rebuilding a lost index costs).
    let started = Instant::now();
    let mut backlinks: HashMap<String, Vec<(usize, String)>> = HashMap::new();
    let mut count = 0;
    for (source, bytes) in snapshots.iter().enumerate() {
        let doc = LoroDoc::new();
        configure(&doc, true);
        doc.import(bytes)?;
        for (block, target) in links(&doc)? {
            backlinks.entry(target).or_default().push((source, block));
            count += 1;
        }
    }
    let rebuild = started.elapsed();

    // One page after an edit: scan only that page (what saving a page costs on top of search indexing).
    let doc = LoroDoc::new();
    configure(&doc, true);
    doc.import(&snapshots[0])?;
    let started = Instant::now();
    let one = links(&doc)?;
    let one_page = started.elapsed();

    // A page the first page links to, so it has at least one backlink.
    let target = one.first().map(|(_, page)| page.clone()).unwrap_or_default();
    let started = Instant::now();
    let found = backlinks.get(&target).map_or(0, Vec::len);
    let lookup = started.elapsed();

    println!(
        "{PAGES} pages, {count} links: rebuild {rebuild:?}; one page ({} links) {one_page:?}; lookup ({found} backlinks) {lookup:?}",
        one.len()
    );
    assert_eq!(count, PAGES * BLOCKS / EVERY);
    assert!(found > 0, "the looked-up page has backlinks");
    Ok(())
}
