//! Tests for the inline database block in the page model (E09-S09, ADR-066 answer 4): what it stores, that it
//! never turns into another kind, how it is repaired, and that duplicating it links to the same database.

use leaf_core::PageId;
use loro::LoroValue;

use super::PageDoc;
use super::tests::insert;
use crate::block_kind::keys;
use crate::{BlockKind, ModelError, PAGE_LINK, PageCommand, PageOutcome};

fn meta(page: &PageDoc, id: leaf_core::BlockId) -> loro::LoroMap {
    page.tree.get_meta(page.node(id).unwrap()).unwrap()
}

#[test]
fn an_inline_database_stores_its_page_and_has_no_text_or_children() {
    let mut page = PageDoc::new();
    let database = PageId::new();
    let block = insert(&mut page, BlockKind::Database { page: database }, "");

    assert_eq!(page.kind(block).unwrap(), BlockKind::Database { page: database });
    assert_eq!(page.plain_text(block).unwrap(), None);
    assert_eq!(
        meta(&page, block)
            .get(keys::TYPE)
            .and_then(|v| v.into_value().ok()),
        Some(LoroValue::String("database".into()))
    );
    assert!(!BlockKind::Database { page: database }.allows_children());
    assert_eq!(page.check_invariants(), Ok(()));

    let reloaded = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    assert_eq!(
        reloaded.kind(block).unwrap(),
        BlockKind::Database { page: database }
    );
}

#[test]
fn an_inline_database_never_turns_into_another_kind_or_from_one() {
    let mut page = PageDoc::new();
    let block = insert(&mut page, BlockKind::Database { page: PageId::new() }, "");
    let text = insert(&mut page, BlockKind::Paragraph, "");

    let out = page.apply(PageCommand::SetBlockKind {
        id: block,
        kind: BlockKind::Paragraph,
    });
    assert!(matches!(out, Err(ModelError::InvalidKind(_))), "{out:?}");
    let out = page.apply(PageCommand::SetBlockKind {
        id: text,
        kind: BlockKind::Database { page: PageId::new() },
    });
    assert!(matches!(out, Err(ModelError::InvalidKind(_))), "{out:?}");
    assert_eq!(page.kind(text).unwrap(), BlockKind::Paragraph);
}

#[test]
fn an_inline_database_that_lost_its_page_becomes_a_paragraph() {
    let mut page = PageDoc::new();
    let block = insert(&mut page, BlockKind::Database { page: PageId::new() }, "");
    meta(&page, block).delete(keys::PAGE).unwrap();
    assert!(page.check_invariants().is_err());

    assert!(page.repair().unwrap());
    assert_eq!(page.check_invariants(), Ok(()));
    assert_eq!(page.kind(block).unwrap(), BlockKind::Paragraph);
}

#[test]
fn a_page_id_that_does_not_read_is_reported() {
    let mut page = PageDoc::new();
    let block = insert(&mut page, BlockKind::Database { page: PageId::new() }, "");
    meta(&page, block).insert(keys::PAGE, "not an id").unwrap();
    assert!(matches!(
        BlockKind::read(&meta(&page, block)),
        Err((keys::PAGE, _))
    ));
    meta(&page, block).insert(keys::PAGE, 7_i64).unwrap();
    assert!(matches!(
        BlockKind::read(&meta(&page, block)),
        Err((keys::PAGE, _))
    ));
}

#[test]
fn duplicating_an_inline_database_links_to_the_same_database() {
    let mut page = PageDoc::new();
    let database = PageId::new();
    let block = insert(&mut page, BlockKind::Database { page: database }, "");

    let PageOutcome::BlocksCreated(created) = page
        .apply(PageCommand::DuplicateBlocks { ids: vec![block] })
        .unwrap()
    else {
        panic!("expected new blocks");
    };
    let copy = created[0];
    assert_eq!(page.kind(copy).unwrap(), BlockKind::Paragraph);
    assert_eq!(page.plain_text(copy).unwrap(), Some(PAGE_LINK.to_string()));
    assert_eq!(page.page_links(), vec![(copy, database)]);
    assert!(meta(&page, copy).get(keys::PAGE).is_none());
    assert_eq!(page.kind(block).unwrap(), BlockKind::Database { page: database });
    assert_eq!(page.check_invariants(), Ok(()));
}
