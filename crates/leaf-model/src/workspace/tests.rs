//! Tests for the workspace read API. Pages are written with raw Loro calls here because the page
//! commands arrive in E01-S03; each fixture is then loaded through `from_bytes`, the real load path.

use leaf_core::PageId;
use loro::{LoroDoc, LoroTree, LoroValue, TreeID, ValueOrContainer};

use super::{PAGES, Workspace};
use crate::page_meta::keys;
use crate::{ModelError, PageMeta};

/// Writes a minimal valid page node and returns its Loro node.
fn raw_page(tree: &LoroTree, parent: Option<TreeID>, id: PageId, title: &str) -> TreeID {
    let node = tree.create(parent).unwrap();
    let meta = tree.get_meta(node).unwrap();
    meta.insert(keys::ID, id.to_string()).unwrap();
    meta.insert(keys::TITLE, title).unwrap();
    meta.insert(keys::CREATED_AT, 1_700_000_000_000_i64).unwrap();
    node
}

/// Builds a document with `build`, exports it, and loads it as a `Workspace`.
fn load(build: impl FnOnce(&LoroTree)) -> Result<Workspace, ModelError> {
    let doc = LoroDoc::new();
    let tree = doc.get_tree(PAGES);
    tree.enable_fractional_index(0);
    build(&tree);
    doc.commit();
    Workspace::from_bytes(&doc.export(loro::ExportMode::Snapshot).unwrap())
}

#[test]
fn a_new_workspace_is_empty() {
    for ws in [Workspace::new(), Workspace::default()] {
        assert!(ws.is_empty());
        assert_eq!(ws.len(), 0);
        assert_eq!(ws.root_pages().unwrap(), Vec::<PageId>::new());
    }
}

#[test]
fn lists_root_pages_and_children_in_order() {
    let [a, b, c, a1, a2] = [(); 5].map(|()| PageId::new());
    let ws = load(|tree| {
        let na = raw_page(tree, None, a, "A");
        raw_page(tree, None, b, "B");
        let nc = raw_page(tree, None, c, "C");
        tree.mov_before(nc, na).unwrap(); // order is C, A, B
        raw_page(tree, Some(na), a1, "A1");
        raw_page(tree, Some(na), a2, "A2");
    })
    .unwrap();

    assert_eq!(ws.root_pages().unwrap(), vec![c, a, b]);
    assert_eq!(ws.children(a).unwrap(), vec![a1, a2]);
    assert_eq!(ws.children(b).unwrap(), Vec::<PageId>::new());
    assert_eq!(ws.len(), 5);
    assert!(ws.contains(a2));
}

#[test]
fn reads_page_meta_with_and_without_optional_fields() {
    let (plain, decorated, restored) = (PageId::new(), PageId::new(), PageId::new());
    let ws = load(|tree| {
        raw_page(tree, None, plain, "Plain");
        let node = raw_page(tree, None, decorated, "Ideas");
        let meta = tree.get_meta(node).unwrap();
        meta.insert(keys::ICON, "💡").unwrap();
        meta.insert(keys::TRASHED_AT, 1_700_000_000_500_i64).unwrap();
        let node = raw_page(tree, None, restored, "Back");
        tree.get_meta(node)
            .unwrap()
            .insert(keys::TRASHED_AT, LoroValue::Null)
            .unwrap();
    })
    .unwrap();

    assert_eq!(
        ws.page(plain).unwrap(),
        PageMeta {
            id: plain,
            title: "Plain".into(),
            icon: None,
            created_at: 1_700_000_000_000,
            trashed_at: None
        }
    );
    let page = ws.page(decorated).unwrap();
    assert_eq!(page.icon.as_deref(), Some("💡"));
    assert_eq!(page.trashed_at, Some(1_700_000_000_500));
    assert_eq!(
        ws.page(restored).unwrap().trashed_at,
        None,
        "null means not trashed"
    );
}

#[test]
fn trashed_pages_are_hidden_from_listings_but_still_readable() {
    let (a, b) = (PageId::new(), PageId::new());
    let ws = load(|tree| {
        let node = raw_page(tree, None, a, "Gone");
        tree.get_meta(node)
            .unwrap()
            .insert(keys::TRASHED_AT, 1_i64)
            .unwrap();
        raw_page(tree, Some(node), b, "Inside");
    })
    .unwrap();
    assert_eq!(
        ws.root_pages().unwrap(),
        Vec::<PageId>::new(),
        "a trashed page is not listed"
    );
    assert_eq!(
        ws.children(a).unwrap(),
        vec![b],
        "its contents can still be read, for the trash view"
    );
    assert!(ws.contains(a) && ws.contains(b));
    assert_eq!(ws.trash().unwrap(), vec![a]);
}

#[test]
fn unknown_pages_are_reported() {
    let ws = Workspace::new();
    let ghost = PageId::new();
    assert!(!ws.contains(ghost));
    assert!(matches!(ws.children(ghost), Err(ModelError::PageNotFound(p)) if p == ghost));
    assert!(matches!(ws.page(ghost), Err(ModelError::PageNotFound(p)) if p == ghost));
}

#[test]
fn bytes_round_trip_keeps_pages_order_and_unknown_fields() {
    let (a, b) = (PageId::new(), PageId::new());
    let ws = load(|tree| {
        let node = raw_page(tree, None, a, "A");
        tree.get_meta(node)
            .unwrap()
            .insert("from_a_newer_version", true)
            .unwrap();
        raw_page(tree, Some(node), b, "B");
    })
    .unwrap();

    let again = Workspace::from_bytes(&ws.to_bytes().unwrap()).unwrap();
    assert_eq!(again.root_pages().unwrap(), vec![a]);
    assert_eq!(again.children(a).unwrap(), vec![b]);
    assert_eq!(again.page(b).unwrap().title, "B");
    let meta = again.tree.get_meta(again.node(a).unwrap()).unwrap();
    assert!(
        matches!(
            meta.get("from_a_newer_version"),
            Some(ValueOrContainer::Value(LoroValue::Bool(true)))
        ),
        "unknown data is preserved, never dropped"
    );
}

#[test]
fn loading_rejects_pages_without_a_usable_id() {
    let missing = load(|tree| {
        tree.create(None).unwrap();
    });
    assert!(matches!(missing, Err(ModelError::InvalidPage { ref reason, .. }) if reason == "id is missing"));

    let not_text = load(|tree| {
        let node = tree.create(None).unwrap();
        tree.get_meta(node).unwrap().insert(keys::ID, 7_i64).unwrap();
    });
    assert!(
        matches!(not_text, Err(ModelError::InvalidPage { ref reason, .. }) if reason == "id is not text")
    );

    let garbage = load(|tree| {
        let node = tree.create(None).unwrap();
        tree.get_meta(node).unwrap().insert(keys::ID, "nope").unwrap();
    });
    assert!(
        matches!(garbage, Err(ModelError::InvalidPage { ref reason, .. }) if reason.contains("invalid page id"))
    );

    let twin = PageId::new();
    let duplicate = load(|tree| {
        raw_page(tree, None, twin, "one");
        raw_page(tree, None, twin, "two");
    });
    assert!(matches!(duplicate, Err(ModelError::DuplicatePage(p)) if p == twin));
}

#[test]
fn loading_rejects_bytes_that_are_not_a_workspace() {
    assert!(matches!(
        Workspace::from_bytes(b"not loro"),
        Err(ModelError::Loro(_))
    ));
}

/// Breaks one field of a page's meta.
type Damage = fn(&loro::LoroMap);

#[test]
fn page_meta_with_wrong_or_missing_fields_is_an_error() {
    let cases: [(&str, Damage); 6] = [
        ("title", |m| {
            m.delete(keys::TITLE).unwrap();
        }),
        ("title", |m| {
            m.insert(keys::TITLE, 1_i64).unwrap();
        }),
        ("icon", |m| {
            m.insert(keys::ICON, true).unwrap();
        }),
        ("created_at", |m| {
            m.delete(keys::CREATED_AT).unwrap();
        }),
        ("created_at", |m| {
            m.insert(keys::CREATED_AT, "yesterday").unwrap();
        }),
        ("trashed_at", |m| {
            m.insert(keys::TRASHED_AT, "soon").unwrap();
        }),
    ];
    for (field, damage) in cases {
        let page = PageId::new();
        let ws = load(|tree| {
            let node = raw_page(tree, None, page, "Damaged");
            damage(&tree.get_meta(node).unwrap());
        })
        .unwrap();
        let err = ws.page(page).unwrap_err();
        assert!(
            matches!(&err, ModelError::InvalidPageMeta { field: f, .. } if *f == field),
            "expected {field} error, got {err}"
        );
    }
}

#[test]
fn errors_explain_themselves() {
    let page = PageId::new();
    let messages = [
        ModelError::PageNotFound(page).to_string(),
        ModelError::DuplicatePage(page).to_string(),
        ModelError::InvalidPage {
            node: "0@1".into(),
            reason: "id is missing".into(),
        }
        .to_string(),
        ModelError::InvalidPageMeta {
            page,
            field: "title",
            reason: "is missing".into(),
        }
        .to_string(),
    ];
    assert_eq!(messages[0], format!("page {page} not found"));
    assert!(messages[1].contains("appears twice"));
    assert_eq!(messages[2], "invalid page node 0@1: id is missing");
    assert_eq!(messages[3], format!("page {page}: field \"title\" is missing"));
}
