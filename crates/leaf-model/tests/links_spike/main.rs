//! E10-S01 spike: proves how a link to a page (Phase 1, `[[page links]]`) lives in a block's Loro text before
//! `leaf-model` wraps it.
//!
//! Throwaway quality on purpose, like `loro_spike.rs` and `database_spike/`: these tests drive Loro directly.
//! Findings and the proposed model are in ADR-071.
//!
//! The model under test: a page link is **one placeholder character** (U+FFFC, the object replacement
//! character) in the block's `text`, carrying a `page` mark whose value is the target's page ID. The mark
//! never expands, so typing next to the link never makes more link. The title is not stored: it is looked up
//! when drawing, so renaming the target updates every link to it.

mod merges;
mod perf;

use loro::{
    ExpandType, ExportMode, LoroDoc, LoroError, LoroResult, LoroText, LoroValue, StyleConfig, StyleConfigMap,
    TextDelta, UndoManager,
};

/// The placeholder character a page link occupies.
pub(crate) const ATOM: char = '\u{FFFC}';

/// The mark table of today's `leaf-model` (`text.rs`), plus `page` when `knows_links` is set. Without it, the
/// doc behaves like a version of Leaf from before links: unknown marks get the default style, which never
/// expands.
pub(crate) fn configure(doc: &LoroDoc, knows_links: bool) {
    let mut styles = StyleConfigMap::new();
    let mut table = vec![
        ("bold", ExpandType::After),
        ("italic", ExpandType::After),
        ("strike", ExpandType::After),
        ("code", ExpandType::None),
        ("link", ExpandType::None),
    ];
    if knows_links {
        table.push(("page", ExpandType::None));
    }
    for (key, expand) in table {
        styles.insert(key.into(), StyleConfig { expand });
    }
    doc.config_text_style(styles);
    doc.config_default_text_style(Some(StyleConfig {
        expand: ExpandType::None,
    }));
}

/// A peer's doc with one block text, `text`.
pub(crate) fn peer(id: u64, knows_links: bool) -> LoroResult<LoroDoc> {
    let doc = LoroDoc::new();
    doc.set_peer_id(id)?;
    configure(&doc, knows_links);
    Ok(doc)
}

/// A second peer starting from `doc`'s state.
pub(crate) fn fork(doc: &LoroDoc, id: u64, knows_links: bool) -> LoroResult<LoroDoc> {
    let other = peer(id, knows_links)?;
    other.import(&export(doc)?)?;
    Ok(other)
}

pub(crate) fn export(doc: &LoroDoc) -> LoroResult<Vec<u8>> {
    doc.export(ExportMode::all_updates())
        .map_err(|err| LoroError::ArgErr(err.to_string().into()))
}

/// Brings two peers together both ways.
pub(crate) fn sync(a: &LoroDoc, b: &LoroDoc) -> LoroResult<()> {
    a.import(&export(b)?)?;
    b.import(&export(a)?)?;
    Ok(())
}

/// Inserts a link to `page` at `pos`, in one commit: the placeholder, then its mark.
pub(crate) fn insert_link(doc: &LoroDoc, text: &LoroText, pos: usize, page: &str) -> LoroResult<()> {
    text.insert(pos, &ATOM.to_string())?;
    text.mark(pos..pos + 1, "page", page)?;
    doc.commit();
    Ok(())
}

/// The text with each link written as `[[page]]`, and other marks dropped: what a test compares.
pub(crate) fn render(text: &LoroText) -> String {
    let mut out = String::new();
    for delta in text.to_delta() {
        if let TextDelta::Insert { insert, attributes } = delta {
            match attributes.as_ref().and_then(|a| a.get("page")) {
                Some(LoroValue::String(page)) => {
                    for c in insert.chars() {
                        if c == ATOM {
                            out.push_str("[[");
                            out.push_str(page.as_str());
                            out.push_str("]]");
                        } else {
                            // A marked character that is not the placeholder: the mark leaked onto text.
                            out.push_str("<leak ");
                            out.push(c);
                            out.push('>');
                        }
                    }
                }
                _ => out.push_str(&insert.replace(ATOM, "<bare>")),
            }
        }
    }
    out
}

/// Every run that carries `key`, so tests can see exactly where a mark ends.
pub(crate) fn marked_runs(text: &LoroText, key: &str) -> Vec<String> {
    text.to_delta()
        .into_iter()
        .filter_map(|delta| match delta {
            TextDelta::Insert { insert, attributes }
                if attributes.as_ref().is_some_and(|a| a.contains_key(key)) =>
            {
                Some(insert)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_link_is_one_character_and_typing_beside_it_never_grows_it() -> LoroResult<()> {
    let doc = peer(1, true)?;
    let text = doc.get_text("text");
    text.insert(0, "See  for dates")?;
    insert_link(&doc, &text, 4, "trip")?;
    assert_eq!(render(&text), "See [[trip]] for dates");

    text.insert(5, "!")?; // right after the link
    text.insert(4, ">")?; // right before it
    assert_eq!(render(&text), "See >[[trip]]! for dates");
    assert_eq!(marked_runs(&text, "page"), vec![ATOM.to_string()]);
    Ok(())
}

#[test]
fn deleting_across_a_link_removes_it_whole() -> LoroResult<()> {
    let doc = peer(1, true)?;
    let text = doc.get_text("text");
    text.insert(0, "ab")?;
    insert_link(&doc, &text, 1, "trip")?;
    text.delete(0, 2)?; // "a" and the link
    assert_eq!(render(&text), "b");
    assert_eq!(marked_runs(&text, "page"), Vec::<String>::new());
    Ok(())
}

#[test]
fn formatting_over_a_link_keeps_it_a_link() -> LoroResult<()> {
    let doc = peer(1, true)?;
    let text = doc.get_text("text");
    text.insert(0, "go  now")?;
    insert_link(&doc, &text, 3, "trip")?;
    text.mark(0..text.len_unicode(), "bold", true)?;
    assert_eq!(render(&text), "go [[trip]] now");
    // The link's own mark splits the run, but every character is bold.
    assert_eq!(marked_runs(&text, "bold").concat(), format!("go {ATOM} now"));
    Ok(())
}

#[test]
fn an_older_version_edits_around_a_link_and_it_survives() -> LoroResult<()> {
    let new = peer(1, true)?;
    let text = new.get_text("text");
    text.insert(0, "Book  today")?;
    insert_link(&new, &text, 5, "ryokan")?;

    // A version of Leaf from before links: `page` is an unknown mark to it.
    let old = fork(&new, 2, false)?;
    let old_text = old.get_text("text");
    old_text.insert(6, "!")?;
    old_text.insert(5, "the ")?;
    old_text.mark(0..4, "italic", true)?;
    old.commit();
    // The older version still sees the mark and its value (it just doesn't draw it).
    assert_eq!(render(&old_text), "Book the [[ryokan]]! today");

    sync(&new, &old)?;
    assert_eq!(render(&text), "Book the [[ryokan]]! today");
    assert_eq!(marked_runs(&text, "page"), vec![ATOM.to_string()]);
    Ok(())
}

#[test]
fn undoing_a_deleted_link_brings_back_the_same_link() -> LoroResult<()> {
    let doc = peer(1, true)?;
    let text = doc.get_text("text");
    let mut undo = UndoManager::new(&doc);
    undo.set_merge_interval(0);
    text.insert(0, "x y")?;
    doc.commit();
    insert_link(&doc, &text, 2, "trip")?;
    let before = doc.get_deep_value();

    text.delete(2, 1)?;
    doc.commit();
    assert_eq!(render(&text), "x y");

    undo.undo()?;
    assert_eq!(render(&text), "x [[trip]]y");
    assert_eq!(doc.get_deep_value(), before);
    Ok(())
}

#[test]
fn a_copied_link_pasted_elsewhere_points_at_the_same_page() -> LoroResult<()> {
    let from = peer(1, true)?;
    let text = from.get_text("text");
    text.insert(0, "see ")?;
    insert_link(&from, &text, 4, "trip")?;

    // The rich clipboard carries runs with their marks; pasting re-applies them.
    let copied = text.slice_delta(0, text.len_unicode(), loro::cursor::PosType::Unicode)?;
    let to = peer(2, true)?;
    let other = to.get_text("text");
    other.insert(0, "→ ")?;
    let mut at = other.len_unicode();
    for delta in copied {
        if let TextDelta::Insert { insert, attributes } = delta {
            let len = insert.chars().count();
            other.insert(at, &insert)?;
            for (key, value) in attributes.unwrap_or_default() {
                other.mark(at..at + len, &key, value)?;
            }
            at += len;
        }
    }
    assert_eq!(render(&other), "→ see [[trip]]");
    Ok(())
}

#[test]
fn the_alternative_a_link_mark_over_the_title_can_be_edited_into_nonsense() -> LoroResult<()> {
    // Rejected alternative: the title's words carrying today's `link` mark with a `leaf://` address. Typing
    // inside the run joins the link (Loro gives an insert inside a styled run that style), so a link can be
    // half edited, and the words go stale when the page is renamed.
    let doc = peer(1, true)?;
    let text = doc.get_text("text");
    text.insert(0, "Japan")?;
    text.mark(0..5, "link", "leaf://page/trip")?;
    text.insert(2, "XX")?;
    assert_eq!(marked_runs(&text, "link"), vec!["JaXXpan"]);
    Ok(())
}
