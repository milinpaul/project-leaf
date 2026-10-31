//! Tests for web bookmarks in the page model (E15-S02, ADR-080): the address and preview, the preview
//! command kept off the undo history, turning to and from linked text, and two devices.

use leaf_core::{AssetId, BlockId};
use loro::LoroValue;

use super::PageDoc;
use super::tests::{insert, load_raw, raw_block};
use crate::block_kind::keys;
use crate::{BlockKind, BlockProp, Mark, ModelError, PageCommand, PageOutcome, TextSpan, WebPreview};

const LEAF: &str = "https://leaf.app/blog";

fn bookmark(url: &str) -> BlockKind {
    BlockKind::WebBookmark {
        url: url.into(),
        preview: WebPreview::default(),
    }
}

fn fetched() -> WebPreview {
    WebPreview {
        title: Some("The Leaf blog".into()),
        description: Some("Notes from building a local-first editor.".into()),
        image: Some(AssetId::of(b"cover")),
        favicon: Some(AssetId::of(b"icon")),
        fetched_at: Some(1_793_000_000_000),
    }
}

fn set_preview(page: &mut PageDoc, id: BlockId, url: &str, preview: WebPreview) -> PageOutcome {
    page.apply(PageCommand::SetWebBookmarkPreview {
        id,
        url: url.into(),
        preview,
    })
    .unwrap()
}

fn preview_of(page: &PageDoc, id: BlockId) -> WebPreview {
    match page.kind(id).unwrap() {
        BlockKind::WebBookmark { preview, .. } => preview,
        other => panic!("expected a web bookmark, got {other:?}"),
    }
}

/// A paragraph whose whole text is a link to `url`.
fn link_paragraph(page: &mut PageDoc, text: &str, url: &str) -> BlockId {
    let id = insert(page, BlockKind::Paragraph, text);
    page.apply(PageCommand::SetMark {
        id,
        range: 0..text.chars().count(),
        mark: Mark::Link(url.into()),
    })
    .unwrap();
    id
}

#[test]
fn a_web_bookmark_has_an_address_and_no_text_or_children() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, bookmark(LEAF), "");
    assert_eq!(page.kind(id).unwrap(), bookmark(LEAF));
    assert_eq!(page.text(id).unwrap(), None, "no text");
    assert!(preview_of(&page, id).is_empty(), "nothing fetched yet");
    let with_text = page.apply(PageCommand::InsertBlock {
        parent: None,
        index: 0,
        kind: bookmark(LEAF),
        text: "words".into(),
    });
    assert!(matches!(with_text, Err(ModelError::TextNotAllowed { .. })));
    let child = page.apply(PageCommand::InsertBlock {
        parent: Some(id),
        index: 0,
        kind: BlockKind::Paragraph,
        text: String::new(),
    });
    assert!(matches!(child, Err(ModelError::ChildrenNotAllowed { .. })));
    page.check_invariants().unwrap();
}

#[test]
fn only_web_addresses_are_taken() {
    let mut page = PageDoc::new();
    for bad in [
        "leaf.app",
        "javascript:alert(1)",
        "file:///etc/hosts",
        "https://",
        "https://me:pw@leaf.app",
    ] {
        let made = page.apply(PageCommand::InsertBlock {
            parent: None,
            index: 0,
            kind: bookmark(bad),
            text: String::new(),
        });
        assert!(matches!(made, Err(ModelError::InvalidWebAddress(_))), "{bad}");
    }
    let id = insert(&mut page, bookmark(LEAF), "");
    let changed = page.apply(PageCommand::SetProp {
        id,
        prop: BlockProp::WebAddress("ftp://leaf.app".into()),
    });
    assert!(matches!(changed, Err(ModelError::InvalidWebAddress(_))));
    assert_eq!(
        page.kind(id).unwrap(),
        bookmark(LEAF),
        "a refused change leaves it as it was"
    );
    assert_eq!(page.len(), 1, "no bad bookmark was made");
}

#[test]
fn the_preview_is_stored_in_the_blocks_meta() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, bookmark(LEAF), "");
    assert_eq!(set_preview(&mut page, id, LEAF, fetched()), PageOutcome::Changed);
    assert_eq!(preview_of(&page, id), fetched());
    assert_eq!(
        set_preview(&mut page, id, LEAF, fetched()),
        PageOutcome::Unchanged,
        "same preview"
    );
    let reopened = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    assert_eq!(preview_of(&reopened, id), fetched(), "kept when saved");
    let meta = page.tree.get_meta(page.node(id).unwrap()).unwrap();
    assert_eq!(
        super::super::page_meta::value(&meta, keys::URL),
        Some(LoroValue::String(LEAF.into()))
    );
    // A new fetch with less in it replaces the whole preview.
    let fewer = WebPreview {
        title: Some("Leaf".into()),
        ..WebPreview::default()
    };
    set_preview(&mut page, id, LEAF, fewer.clone());
    assert_eq!(preview_of(&page, id), fewer);
}

#[test]
fn a_late_preview_for_a_block_that_moved_on_changes_nothing() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, bookmark(LEAF), "");
    assert_eq!(
        set_preview(&mut page, id, "https://other.example", fetched()),
        PageOutcome::Unchanged
    );
    let paragraph = insert(&mut page, BlockKind::Paragraph, "");
    assert_eq!(
        set_preview(&mut page, paragraph, LEAF, fetched()),
        PageOutcome::Unchanged
    );
    page.apply(PageCommand::DeleteBlocks { ids: vec![id] }).unwrap();
    assert_eq!(
        set_preview(&mut page, id, LEAF, fetched()),
        PageOutcome::Unchanged,
        "gone"
    );
    assert!(preview_of_paragraph_is_untouched(&page, paragraph));
}

fn preview_of_paragraph_is_untouched(page: &PageDoc, id: BlockId) -> bool {
    let meta = page.tree.get_meta(page.node(id).unwrap()).unwrap();
    meta.get(keys::TITLE).is_none() && page.kind(id).unwrap() == BlockKind::Paragraph
}

#[test]
fn a_new_address_clears_the_old_preview_and_undo_brings_both_back() {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    let id = insert(&mut page, bookmark(LEAF), "");
    set_preview(&mut page, id, LEAF, fetched());
    page.apply(PageCommand::SetProp {
        id,
        prop: BlockProp::WebAddress("https://example.com".into()),
    })
    .unwrap();
    assert_eq!(page.kind(id).unwrap(), bookmark("https://example.com"));
    page.undo().unwrap();
    assert_eq!(
        page.kind(id).unwrap(),
        BlockKind::WebBookmark {
            url: LEAF.into(),
            preview: fetched()
        }
    );
    // The same address keeps the preview.
    page.apply(PageCommand::SetProp {
        id,
        prop: BlockProp::WebAddress(LEAF.into()),
    })
    .unwrap();
    assert_eq!(preview_of(&page, id), fetched());
}

#[test]
fn the_preview_is_kept_off_the_undo_history() {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    let id = insert(&mut page, bookmark(LEAF), "");
    let other = insert(&mut page, BlockKind::Paragraph, "next");
    set_preview(&mut page, id, LEAF, fetched());
    // Undo takes back the owner's last edit, not the preview.
    page.undo().unwrap();
    assert!(page.node(other).is_err(), "the paragraph went");
    assert_eq!(preview_of(&page, id), fetched(), "the preview stayed");
    // Undoing the bookmark's creation takes the preview with it; redo brings both.
    page.undo().unwrap();
    assert!(page.is_empty());
    page.redo().unwrap();
    assert_eq!(preview_of(&page, id), fetched());
    page.check_invariants().unwrap();
}

#[test]
fn a_preview_for_the_old_address_after_a_change_is_dropped() {
    // The fetch for the old address finishes after the owner typed a new one.
    let mut page = PageDoc::new();
    let id = insert(&mut page, bookmark(LEAF), "");
    page.apply(PageCommand::SetProp {
        id,
        prop: BlockProp::WebAddress("https://example.com".into()),
    })
    .unwrap();
    assert_eq!(
        set_preview(&mut page, id, LEAF, fetched()),
        PageOutcome::Unchanged
    );
    assert!(preview_of(&page, id).is_empty());
}

#[test]
fn a_web_bookmark_turns_into_its_address_as_linked_text() {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    let id = insert(&mut page, bookmark(LEAF), "");
    set_preview(&mut page, id, LEAF, fetched());
    page.apply(PageCommand::SetBlockKind {
        id,
        kind: BlockKind::Paragraph,
    })
    .unwrap();
    let spans = page.text(id).unwrap().unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, LEAF);
    assert_eq!(spans[0].marks.link.as_deref(), Some(LEAF));
    let meta = page.tree.get_meta(page.node(id).unwrap()).unwrap();
    for key in [
        keys::URL,
        keys::TITLE,
        keys::DESCRIPTION,
        keys::IMAGE,
        keys::FAVICON,
        keys::FETCHED_AT,
    ] {
        assert!(meta.get(key).is_none(), "{key} removed");
    }
    page.check_invariants().unwrap();
    page.undo().unwrap();
    assert_eq!(
        page.kind(id).unwrap(),
        BlockKind::WebBookmark {
            url: LEAF.into(),
            preview: fetched()
        },
        "undo brings the bookmark back whole"
    );
    // Into code: the address as plain text, since code takes no marks.
    page.apply(PageCommand::SetBlockKind {
        id,
        kind: BlockKind::Code {
            language: String::new(),
        },
    })
    .unwrap();
    let spans: Vec<TextSpan> = page.text(id).unwrap().unwrap();
    assert_eq!(spans[0].text, LEAF);
    assert_eq!(spans[0].marks.link, None);
}

#[test]
fn a_block_that_is_only_a_link_turns_into_a_web_bookmark() {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    let id = link_paragraph(&mut page, "Leaf's blog", LEAF);
    page.apply(PageCommand::SetBlockKind {
        id,
        kind: bookmark(LEAF),
    })
    .unwrap();
    assert_eq!(page.kind(id).unwrap(), bookmark(LEAF));
    assert_eq!(page.text(id).unwrap(), None);
    page.check_invariants().unwrap();
    page.undo().unwrap();
    assert_eq!(
        page.text(id).unwrap().unwrap()[0].text,
        "Leaf's blog",
        "undo brings the words back"
    );

    // An empty block may become one too.
    let empty = insert(&mut page, BlockKind::Paragraph, "");
    page.apply(PageCommand::SetBlockKind {
        id: empty,
        kind: bookmark(LEAF),
    })
    .unwrap();

    // Anything else would lose words: refused, and the block is untouched.
    let mixed = link_paragraph(&mut page, "Leaf", LEAF);
    page.apply(PageCommand::InsertText {
        id: mixed,
        pos: 4,
        text: " and more".into(),
    })
    .unwrap();
    let other_link = link_paragraph(&mut page, "Example", "https://example.com");
    let plain = insert(&mut page, BlockKind::Paragraph, "https://leaf.app/blog");
    for block in [mixed, other_link, plain] {
        let before = page.text(block).unwrap();
        let refused = page.apply(PageCommand::SetBlockKind {
            id: block,
            kind: bookmark(LEAF),
        });
        assert!(matches!(refused, Err(ModelError::InvalidKind(_))), "{refused:?}");
        assert_eq!(page.text(block).unwrap(), before);
        assert_eq!(page.kind(block).unwrap(), BlockKind::Paragraph);
    }
}

#[test]
fn children_step_out_of_a_block_that_becomes_a_web_bookmark() {
    let mut page = PageDoc::new();
    let id = link_paragraph(&mut page, "Leaf", LEAF);
    page.apply(PageCommand::InsertBlock {
        parent: Some(id),
        index: 0,
        kind: BlockKind::Paragraph,
        text: "nested".into(),
    })
    .unwrap();
    page.apply(PageCommand::SetBlockKind {
        id,
        kind: bookmark(LEAF),
    })
    .unwrap();
    assert_eq!(page.root_blocks().len(), 2);
    page.check_invariants().unwrap();
}

#[test]
fn unknown_props_and_unreadable_preview_parts_are_kept() {
    let id = BlockId::new();
    let mut page = load_raw(|tree| {
        let meta = raw_block(tree, id, "web_bookmark");
        meta.insert(keys::URL, LEAF).unwrap();
        meta.insert(keys::TITLE, "Leaf").unwrap();
        meta.insert(keys::IMAGE, 42_i64).unwrap(); // from a newer version, say
        meta.insert("site_name", "Leaf").unwrap();
    })
    .unwrap();
    assert_eq!(
        preview_of(&page, id),
        WebPreview {
            title: Some("Leaf".into()),
            ..WebPreview::default()
        },
        "an unreadable image reads as none"
    );
    page.apply(PageCommand::SetProp {
        id,
        prop: BlockProp::WebAddress(LEAF.into()),
    })
    .unwrap();
    let meta = page.tree.get_meta(page.node(id).unwrap()).unwrap();
    assert_eq!(
        super::super::page_meta::value(&meta, "site_name"),
        Some(LoroValue::String("Leaf".into())),
        "a key this version doesn't know stays"
    );
    assert_eq!(
        super::super::page_meta::value(&meta, keys::IMAGE),
        Some(LoroValue::I64(42))
    );
}

#[test]
fn a_web_bookmark_that_lost_its_address_in_a_merge_is_repaired_into_a_paragraph() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, bookmark(LEAF), "");
    set_preview(&mut page, id, LEAF, fetched());
    page.tree
        .get_meta(page.node(id).unwrap())
        .unwrap()
        .delete(keys::URL)
        .unwrap();
    assert!(page.check_invariants().is_err());
    assert!(page.repair().unwrap());
    assert_eq!(page.check_invariants(), Ok(()));
    assert_eq!(page.kind(id).unwrap(), BlockKind::Paragraph);
    let meta = page.tree.get_meta(page.node(id).unwrap()).unwrap();
    assert!(meta.get(keys::TITLE).is_none(), "no preview left on a paragraph");
}

#[test]
fn two_devices_turning_and_fetching_at_once_agree() {
    let mut a = PageDoc::new();
    let id = insert(&mut a, bookmark(LEAF), "");
    let mut b = PageDoc::from_bytes(&a.to_bytes().unwrap()).unwrap();
    // One turns it into a link while the other's preview arrives.
    a.apply(PageCommand::SetBlockKind {
        id,
        kind: BlockKind::Paragraph,
    })
    .unwrap();
    set_preview(&mut b, id, LEAF, fetched());
    let (from_a, from_b) = (a.to_bytes().unwrap(), b.to_bytes().unwrap());
    a.import_changes(&from_b).unwrap();
    b.import_changes(&from_a).unwrap();
    assert_eq!(a.kind(id).unwrap(), b.kind(id).unwrap());
    assert_eq!(a.text(id).unwrap(), b.text(id).unwrap());
    assert_eq!(
        a.kind(id).unwrap(),
        BlockKind::Paragraph,
        "the owner's change wins"
    );
    a.check_invariants().unwrap();
    b.check_invariants().unwrap();
}

#[test]
fn two_devices_changing_the_address_and_fetching_agree() {
    let mut a = PageDoc::new();
    let id = insert(&mut a, bookmark(LEAF), "");
    let mut b = PageDoc::from_bytes(&a.to_bytes().unwrap()).unwrap();
    a.apply(PageCommand::SetProp {
        id,
        prop: BlockProp::WebAddress("https://example.com".into()),
    })
    .unwrap();
    set_preview(&mut b, id, LEAF, fetched());
    let (from_a, from_b) = (a.to_bytes().unwrap(), b.to_bytes().unwrap());
    a.import_changes(&from_b).unwrap();
    b.import_changes(&from_a).unwrap();
    assert_eq!(a.kind(id).unwrap(), b.kind(id).unwrap());
    a.check_invariants().unwrap();
}
