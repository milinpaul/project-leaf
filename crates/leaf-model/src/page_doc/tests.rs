//! Tests for the block schema and the page read API (E02-S01).

use leaf_core::{AssetId, BlockId};
use loro::{LoroDoc, LoroMap, LoroTree, LoroValue, ValueOrContainer};

use super::{BLOCKS, PageDoc};
use crate::block_kind::keys;
use crate::{BlockKind, DueDate, DueTime, Marks, ModelError, PageCommand, PageOutcome, Priority, TextSpan};

fn all_known_kinds() -> Vec<BlockKind> {
    vec![
        BlockKind::Paragraph,
        BlockKind::Heading { level: 1 },
        BlockKind::Heading { level: 3 },
        BlockKind::BulletedList,
        BlockKind::NumberedList,
        BlockKind::Todo { checked: false },
        BlockKind::Todo { checked: true },
        BlockKind::Quote,
        BlockKind::Code {
            language: "rust".into(),
        },
        BlockKind::Divider,
        BlockKind::Image {
            asset: AssetId::of(b"png"),
            width: Some(320.5),
            alt: "A leaf".into(),
        },
        BlockKind::Image {
            asset: AssetId::of(b"jpg"),
            width: None,
            alt: String::new(),
        },
        BlockKind::Task {
            done: false,
            due: None,
            time: None,
            priority: None,
        },
        BlockKind::Task {
            done: true,
            due: DueDate::new(2028, 2, 29).ok(),
            time: DueTime::new(8, 15).ok(),
            priority: Some(Priority::Medium),
        },
    ]
}

pub(super) fn insert(page: &mut PageDoc, kind: BlockKind, text: &str) -> BlockId {
    let index = page.root_blocks().len();
    match page
        .apply(PageCommand::InsertBlock {
            parent: None,
            index,
            kind,
            text: text.into(),
        })
        .unwrap()
    {
        PageOutcome::BlockCreated(block) => block,
        other => panic!("expected a new block, got {other:?}"),
    }
}

/// Builds a page with raw Loro writes (for data the commands refuse to create), then loads it.
pub(super) fn load_raw(build: impl FnOnce(&LoroTree)) -> Result<PageDoc, ModelError> {
    let doc = LoroDoc::new();
    let tree = doc.get_tree(BLOCKS);
    tree.enable_fractional_index(0);
    build(&tree);
    doc.commit();
    PageDoc::from_bytes(&doc.export(loro::ExportMode::Snapshot).unwrap())
}

/// A raw block node with an id and type; returns its meta for further writes.
pub(super) fn raw_block(tree: &LoroTree, id: BlockId, type_name: &str) -> LoroMap {
    let meta = tree.get_meta(tree.create(None).unwrap()).unwrap();
    meta.insert(keys::ID, id.to_string()).unwrap();
    meta.insert(keys::TYPE, type_name).unwrap();
    meta
}

#[test]
fn a_new_page_is_empty() {
    for page in [PageDoc::new(), PageDoc::default()] {
        assert!(page.is_empty());
        assert_eq!(page.len(), 0);
        assert_eq!(page.root_blocks(), Vec::<BlockId>::new());
    }
}

#[test]
fn every_kind_round_trips_through_save_and_load() {
    let mut page = PageDoc::new();
    let blocks: Vec<(BlockId, BlockKind)> = all_known_kinds()
        .into_iter()
        .map(|kind| {
            let text = if kind.has_text() { "words" } else { "" };
            (insert(&mut page, kind.clone(), text), kind)
        })
        .collect();

    let loaded = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    assert_eq!(
        loaded.root_blocks(),
        blocks.iter().map(|(b, _)| *b).collect::<Vec<_>>()
    );
    for (block, kind) in blocks {
        assert_eq!(loaded.kind(block).unwrap(), kind);
        let expected = kind.has_text().then(|| "words".to_owned());
        assert_eq!(loaded.plain_text(block).unwrap(), expected, "{kind:?}");
    }
}

#[test]
fn the_kind_table_matches_block_model() {
    let kinds = all_known_kinds();
    let table: Vec<(&str, bool, bool, bool)> = kinds
        .iter()
        .map(|k| (k.type_name(), k.allows_children(), k.has_text(), k.allows_marks()))
        .collect();
    assert_eq!(
        table,
        vec![
            ("paragraph", true, true, true),
            ("heading", false, true, true),
            ("heading", false, true, true),
            ("bulleted_list", true, true, true),
            ("numbered_list", true, true, true),
            ("todo", true, true, true),
            ("todo", true, true, true),
            ("quote", true, true, true),
            ("code", false, true, false),
            ("divider", false, false, false),
            ("image", false, true, true),
            ("image", false, true, true),
            ("task", true, true, true),
            ("task", true, true, true),
        ]
    );
    // Tables: only rows and cells inside, and only through the table commands (ADR-061).
    let tables: Vec<(&str, bool, bool, bool, bool)> = [
        BlockKind::Table { header_row: true },
        BlockKind::TableRow,
        BlockKind::TableCell,
    ]
    .iter()
    .map(|k| {
        (
            k.type_name(),
            k.allows_children(),
            k.has_text(),
            k.allows_marks(),
            k.is_table_part(),
        )
    })
    .collect();
    assert_eq!(
        tables,
        vec![
            ("table", false, false, false, false),
            ("table_row", false, false, false, true),
            ("table_cell", false, true, true, true),
        ]
    );
    let unknown = BlockKind::Unknown {
        type_name: "kanban".into(),
    };
    assert_eq!(
        (unknown.type_name(), unknown.allows_children(), unknown.has_text()),
        ("kanban", false, false)
    );
}

#[test]
fn text_reads_back_as_spans_with_marks_and_expands_as_configured_after_load() {
    let mut page = PageDoc::new();
    let block = insert(&mut page, BlockKind::Paragraph, "bold link plain");
    let text = page.text_container(block).unwrap().unwrap();
    text.mark(0..4, "bold", true).unwrap();
    text.mark(5..9, "link", "https://leaf.app").unwrap();
    page.doc.commit();

    let loaded = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    let bold = Marks {
        bold: true,
        ..Marks::default()
    };
    let link = Marks {
        link: Some("https://leaf.app".into()),
        ..Marks::default()
    };
    let span = |text: &str, marks: &Marks| TextSpan {
        text: text.into(),
        marks: marks.clone(),
    };
    assert_eq!(
        loaded.text(block).unwrap().unwrap(),
        vec![
            span("bold", &bold),
            span(" ", &Marks::default()),
            span("link", &link),
            span(" plain", &Marks::default())
        ]
    );

    // The mark table must be configured on load too: bold grows when typing after it, links don't.
    let text = loaded.text_container(block).unwrap().unwrap();
    text.insert(9, "!").unwrap();
    text.insert(4, "er").unwrap();
    loaded.doc.commit();
    let spans = loaded.text(block).unwrap().unwrap();
    assert_eq!((spans[0].text.as_str(), &spans[0].marks), ("bolder", &bold));
    assert_eq!((spans[2].text.as_str(), &spans[2].marks), ("link", &link));
}

#[test]
fn unknown_block_types_are_read_and_preserved_untouched() {
    let block = BlockId::new();
    let page = load_raw(|tree| {
        let meta = raw_block(tree, block, "kanban");
        meta.ensure_mergeable_map("props") // a future version's own nested data
            .unwrap()
            .insert("columns", 3_i64)
            .unwrap();
        meta.insert("from_the_future", true).unwrap();
    })
    .unwrap();

    assert_eq!(
        page.kind(block).unwrap(),
        BlockKind::Unknown {
            type_name: "kanban".into()
        }
    );
    assert_eq!(page.text(block).unwrap(), None);

    let again = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    let meta = again.tree.get_meta(again.node(block).unwrap()).unwrap();
    assert!(matches!(
        meta.get("from_the_future"),
        Some(ValueOrContainer::Value(LoroValue::Bool(true)))
    ));
    let Some(ValueOrContainer::Container(loro::Container::Map(props))) = meta.get("props") else {
        panic!("a future version's nested data must survive");
    };
    assert!(matches!(
        props.get("columns"),
        Some(ValueOrContainer::Value(LoroValue::I64(3)))
    ));
}

/// A block type, the field expected to be reported, and an optional prop to store.
type BadData = (&'static str, &'static str, Option<(&'static str, LoroValue)>);

#[test]
fn known_kinds_with_bad_data_are_errors_not_guesses() {
    let cases: [BadData; 14] = [
        ("type missing", "", None),
        ("heading", "level", Some(("level", LoroValue::I64(7)))),
        ("heading", "level", None),
        ("todo", "checked", Some(("checked", LoroValue::I64(1)))),
        ("code", "language", Some(("language", LoroValue::I64(1)))),
        ("image", "asset", None),
        (
            "image",
            "asset",
            Some(("asset", LoroValue::String("not-a-hash".into()))),
        ),
        (
            "image",
            "width",
            Some(("width", LoroValue::String("wide".into()))),
        ),
        ("image", "alt", Some(("alt", LoroValue::I64(0)))),
        ("task", "done", Some(("done", LoroValue::String("yes".into())))),
        (
            "task",
            "due",
            Some(("due", LoroValue::String("2026-02-30".into()))),
        ),
        ("task", "due", Some(("due", LoroValue::I64(20_261_031)))),
        (
            "task",
            "due_time",
            Some(("due_time", LoroValue::String("25:00".into()))),
        ),
        (
            "task",
            "priority",
            Some(("priority", LoroValue::String("urgent".into()))),
        ),
    ];
    for (type_name, field, prop) in cases {
        let block = BlockId::new();
        let page = load_raw(|tree| {
            let meta = tree.get_meta(tree.create(None).unwrap()).unwrap();
            meta.insert(keys::ID, block.to_string()).unwrap();
            if type_name != "type missing" {
                meta.insert(keys::TYPE, type_name).unwrap();
            }
            let props = &meta; // properties sit directly in the block's meta (ADR-007)
            if type_name == "image" && field != "asset" {
                props.insert(keys::ASSET, AssetId::of(b"x").to_string()).unwrap();
            }
            if let Some((key, value)) = prop {
                props.insert(key, value).unwrap();
            }
        })
        .unwrap();
        let err = page.kind(block).unwrap_err();
        let expected_field = if type_name == "type missing" {
            "type"
        } else {
            field
        };
        assert!(
            matches!(&err, ModelError::InvalidBlockData { block: b, field: f, .. } if *b == block && *f == expected_field),
            "{type_name}/{field}: {err}"
        );
    }
    let block = BlockId::new();
    let page = load_raw(|tree| {
        raw_block(tree, block, "paragraph");
        let meta = tree.get_meta(tree.roots()[0]).unwrap();
        meta.insert(keys::TYPE, 5_i64).unwrap();
    })
    .unwrap();
    assert!(matches!(
        page.kind(block),
        Err(ModelError::InvalidBlockData { field: "type", .. })
    ));
}

#[test]
fn loading_rejects_blocks_without_a_usable_id() {
    let missing = load_raw(|tree| {
        tree.create(None).unwrap();
    });
    assert!(matches!(missing, Err(ModelError::InvalidBlock { ref reason, .. }) if reason == "id is missing"));
    let not_text = load_raw(|tree| {
        tree.get_meta(tree.create(None).unwrap())
            .unwrap()
            .insert(keys::ID, 1_i64)
            .unwrap();
    });
    assert!(
        matches!(not_text, Err(ModelError::InvalidBlock { ref reason, .. }) if reason == "id is not text")
    );
    let garbage = load_raw(|tree| {
        tree.get_meta(tree.create(None).unwrap())
            .unwrap()
            .insert(keys::ID, "nope")
            .unwrap();
    });
    assert!(
        matches!(garbage, Err(ModelError::InvalidBlock { ref reason, .. }) if reason.contains("invalid block id"))
    );
    let twin = BlockId::new();
    let duplicate = load_raw(|tree| {
        raw_block(tree, twin, "paragraph");
        raw_block(tree, twin, "paragraph");
    });
    assert!(matches!(duplicate, Err(ModelError::DuplicateBlock(b)) if b == twin));
    assert!(matches!(
        PageDoc::from_bytes(b"not loro"),
        Err(ModelError::Loro(_))
    ));
}

#[test]
fn unknown_blocks_are_reported() {
    let page = PageDoc::new();
    let ghost = BlockId::new();
    assert!(!page.contains(ghost));
    let not_found = |r: Result<(), ModelError>| matches!(r, Err(ModelError::BlockNotFound(b)) if b == ghost);
    assert!(not_found(page.children(ghost).map(|_| ())));
    assert!(not_found(page.parent(ghost).map(|_| ())));
    assert!(not_found(page.kind(ghost).map(|_| ())));
    assert!(not_found(page.text(ghost).map(|_| ())));
    assert!(not_found(page.plain_text(ghost).map(|_| ())));
}

#[test]
fn block_errors_explain_themselves() {
    let (block, parent) = (BlockId::new(), BlockId::new());
    let messages = [
        ModelError::BlockNotFound(block).to_string(),
        ModelError::DuplicateBlock(block).to_string(),
        ModelError::InvalidBlock {
            node: "0@1".into(),
            reason: "id is missing".into(),
        }
        .to_string(),
        ModelError::InvalidBlockData {
            block,
            field: "level",
            reason: "must be 1, 2 or 3".into(),
        }
        .to_string(),
        ModelError::InvalidKind("nope".into()).to_string(),
        ModelError::TextNotAllowed {
            kind: "divider".into(),
        }
        .to_string(),
        ModelError::ChildrenNotAllowed {
            parent,
            kind: "heading".into(),
        }
        .to_string(),
        ModelError::BlockMoveIntoOwnSubtree {
            block,
            new_parent: parent,
        }
        .to_string(),
    ];
    assert_eq!(messages[0], format!("block {block} not found"));
    assert!(messages[1].contains("appears twice"));
    assert_eq!(messages[2], "invalid block node 0@1: id is missing");
    assert_eq!(messages[3], format!("block {block}: \"level\" must be 1, 2 or 3"));
    assert_eq!(messages[4], "invalid block kind: nope");
    assert_eq!(messages[5], "divider blocks have no text");
    assert_eq!(
        messages[6],
        format!("block {parent} is a heading, which can't contain other blocks")
    );
    assert!(messages[7].starts_with(&format!("cannot move block {block} into {parent}")));
}
