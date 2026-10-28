//! Tests for changing a block's kind and properties (E02-S04).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use leaf_core::{AssetId, BlockId};
use loro::{Container, LoroMap, ValueOrContainer};

use crate::block_kind::keys;
use crate::{BlockKind, BlockProp, Mark, ModelError, PageCommand, PageDoc, PageOutcome};

fn add(page: &mut PageDoc, parent: Option<BlockId>, kind: BlockKind, text: &str) -> BlockId {
    let index = match parent {
        Some(p) => page.children(p).unwrap().len(),
        None => page.root_blocks().len(),
    };
    match page
        .apply(PageCommand::InsertBlock {
            parent,
            index,
            kind,
            text: text.into(),
        })
        .unwrap()
    {
        PageOutcome::BlockCreated(id) => id,
        other => panic!("expected a new block, got {other:?}"),
    }
}

fn set_kind(page: &mut PageDoc, id: BlockId, kind: BlockKind) -> Result<PageOutcome, ModelError> {
    page.apply(PageCommand::SetBlockKind { id, kind })
}

fn set_prop(page: &mut PageDoc, id: BlockId, prop: BlockProp) -> Result<PageOutcome, ModelError> {
    page.apply(PageCommand::SetProp { id, prop })
}

fn props(page: &PageDoc, id: BlockId) -> LoroMap {
    let meta = page.tree.get_meta(page.node(id).unwrap()).unwrap();
    match meta.get(keys::PROPS) {
        Some(ValueOrContainer::Container(Container::Map(props))) => props,
        other => panic!("no props map: {other:?}"),
    }
}

fn commit_counter(page: &PageDoc) -> (Arc<AtomicUsize>, loro::Subscription) {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&count);
    let sub = page.doc.subscribe_local_update(Box::new(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        true
    }));
    (count, sub)
}

#[test]
fn text_is_kept_across_kinds_that_have_text() {
    let mut page = PageDoc::new();
    let id = add(&mut page, None, BlockKind::Paragraph, "Keep me");
    for kind in [
        BlockKind::Heading { level: 2 },
        BlockKind::Quote,
        BlockKind::Todo { checked: false },
        BlockKind::Code {
            language: "text".into(),
        },
        BlockKind::BulletedList,
        BlockKind::Paragraph,
    ] {
        assert_eq!(
            set_kind(&mut page, id, kind.clone()).unwrap(),
            PageOutcome::Changed,
            "{kind:?}"
        );
        assert_eq!(page.kind(id).unwrap(), kind);
        assert_eq!(page.plain_text(id).unwrap().as_deref(), Some("Keep me"));
    }
}

#[test]
fn kinds_without_children_let_their_children_out_in_order() {
    let mut page = PageDoc::new();
    let parent = add(&mut page, None, BlockKind::Paragraph, "Parent");
    let after = add(&mut page, None, BlockKind::Paragraph, "After");
    let [first, second] = ["one", "two"].map(|t| add(&mut page, Some(parent), BlockKind::Paragraph, t));

    set_kind(&mut page, parent, BlockKind::Heading { level: 1 }).unwrap();
    assert_eq!(page.root_blocks(), vec![parent, first, second, after]);
    assert_eq!(page.children(parent).unwrap(), Vec::<BlockId>::new());
}

#[test]
fn converting_to_code_strips_every_mark() {
    let mut page = PageDoc::new();
    let id = add(&mut page, None, BlockKind::Paragraph, "styled text");
    page.apply(PageCommand::SetMark {
        id,
        range: 0..6,
        mark: Mark::Bold,
    })
    .unwrap();
    page.apply(PageCommand::SetMark {
        id,
        range: 7..11,
        mark: Mark::Link("https://a.b".into()),
    })
    .unwrap();
    // A mark from a newer version that this one doesn't know.
    page.text_container(id)
        .unwrap()
        .unwrap()
        .mark(0..11, "highlight", "yellow")
        .unwrap();
    page.doc.commit();

    set_kind(
        &mut page,
        id,
        BlockKind::Code {
            language: String::new(),
        },
    )
    .unwrap();
    let delta = page.text_container(id).unwrap().unwrap().to_delta();
    assert!(
        delta
            .iter()
            .all(|d| matches!(d, loro::TextDelta::Insert { attributes: None, .. })),
        "no marks left at all: {delta:?}"
    );
    set_kind(&mut page, id, BlockKind::Paragraph).unwrap();
    assert_eq!(page.text(id).unwrap().unwrap().len(), 1, "one plain run");
}

#[test]
fn heading_levels_are_checked() {
    let mut page = PageDoc::new();
    let id = add(&mut page, None, BlockKind::Heading { level: 1 }, "H");
    for level in [0, 4] {
        assert!(matches!(
            set_kind(&mut page, id, BlockKind::Heading { level }),
            Err(ModelError::InvalidKind(_))
        ));
        assert!(matches!(
            set_prop(&mut page, id, BlockProp::HeadingLevel(level)),
            Err(ModelError::InvalidKind(_))
        ));
    }
    set_prop(&mut page, id, BlockProp::HeadingLevel(3)).unwrap();
    assert_eq!(page.kind(id).unwrap(), BlockKind::Heading { level: 3 });
}

#[test]
fn dividers_never_swallow_text_and_give_it_back_empty() {
    let mut page = PageDoc::new();
    let worded = add(&mut page, None, BlockKind::Paragraph, "words");
    let err = set_kind(&mut page, worded, BlockKind::Divider).unwrap_err();
    assert!(
        matches!(&err, ModelError::TextNotAllowed { kind } if kind == "divider"),
        "{err}"
    );
    assert_eq!(page.plain_text(worded).unwrap().as_deref(), Some("words"));

    let empty = add(&mut page, None, BlockKind::Paragraph, "");
    set_kind(&mut page, empty, BlockKind::Divider).unwrap();
    assert_eq!(
        page.plain_text(empty).unwrap(),
        None,
        "dividers have no text container (invariant 2)"
    );
    set_kind(&mut page, empty, BlockKind::Paragraph).unwrap();
    assert_eq!(
        page.plain_text(empty).unwrap().as_deref(),
        Some(""),
        "a text kind gets a fresh container"
    );
}

#[test]
fn stale_props_are_removed_but_props_from_newer_versions_are_kept() {
    let mut page = PageDoc::new();
    let id = add(&mut page, None, BlockKind::Heading { level: 2 }, "H");
    props(&page, id).insert("from_the_future", true).unwrap();
    page.doc.commit();

    set_kind(&mut page, id, BlockKind::Todo { checked: true }).unwrap();
    let after = props(&page, id);
    assert!(after.get(keys::LEVEL).is_none(), "a to-do has no level");
    assert!(after.get(keys::CHECKED).is_some());
    assert!(
        after.get("from_the_future").is_some(),
        "unknown data is never dropped"
    );
}

#[test]
fn unknown_kinds_are_neither_created_nor_converted() {
    let mut page = PageDoc::new();
    let id = add(&mut page, None, BlockKind::Paragraph, "");
    let unknown = BlockKind::Unknown {
        type_name: "kanban".into(),
    };
    assert!(matches!(
        set_kind(&mut page, id, unknown),
        Err(ModelError::InvalidKind(_))
    ));

    let future = BlockId::new();
    let doc = loro::LoroDoc::new();
    let tree = doc.get_tree("blocks");
    tree.enable_fractional_index(0);
    let meta = tree.get_meta(tree.create(None).unwrap()).unwrap();
    meta.insert(keys::ID, future.to_string()).unwrap();
    meta.insert(keys::TYPE, "kanban").unwrap();
    doc.commit();
    let mut page = PageDoc::from_bytes(&doc.export(loro::ExportMode::Snapshot).unwrap()).unwrap();
    let err = set_kind(&mut page, future, BlockKind::Paragraph).unwrap_err();
    assert!(
        matches!(&err, ModelError::InvalidKind(reason) if reason.contains("would be rewritten")),
        "{err}"
    );
}

#[test]
fn set_prop_changes_one_property_of_the_right_kind() {
    let mut page = PageDoc::new();
    let todo = add(&mut page, None, BlockKind::Todo { checked: false }, "Task");
    let code = add(
        &mut page,
        None,
        BlockKind::Code {
            language: String::new(),
        },
        "x",
    );
    let asset = AssetId::of(b"picture");
    let image = add(
        &mut page,
        None,
        BlockKind::Image {
            asset,
            width: Some(100.0),
            alt: String::new(),
        },
        "",
    );

    set_prop(&mut page, todo, BlockProp::Checked(true)).unwrap();
    assert_eq!(page.kind(todo).unwrap(), BlockKind::Todo { checked: true });
    assert_eq!(
        page.plain_text(todo).unwrap().as_deref(),
        Some("Task"),
        "ticking keeps the text"
    );
    set_prop(&mut page, code, BlockProp::Language("rust".into())).unwrap();
    assert_eq!(
        page.kind(code).unwrap(),
        BlockKind::Code {
            language: "rust".into()
        }
    );
    set_prop(&mut page, image, BlockProp::ImageAlt("A leaf".into())).unwrap();
    set_prop(&mut page, image, BlockProp::ImageWidth(None)).unwrap();
    assert_eq!(
        page.kind(image).unwrap(),
        BlockKind::Image {
            asset,
            width: None,
            alt: "A leaf".into()
        }
    );

    let err = set_prop(&mut page, code, BlockProp::Checked(true)).unwrap_err();
    assert!(
        matches!(&err, ModelError::PropNotForKind { prop: "checked", kind } if kind == "code"),
        "{err}"
    );
    assert_eq!(err.to_string(), "code blocks have no checked");
}

#[test]
fn setting_the_same_kind_or_value_changes_nothing() {
    let mut page = PageDoc::new();
    let todo = add(&mut page, None, BlockKind::Todo { checked: true }, "Task");
    let (commits, _sub) = commit_counter(&page);
    assert_eq!(
        set_prop(&mut page, todo, BlockProp::Checked(true)).unwrap(),
        PageOutcome::Unchanged
    );
    assert_eq!(
        set_kind(&mut page, todo, BlockKind::Todo { checked: true }).unwrap(),
        PageOutcome::Unchanged
    );
    assert!(matches!(
        set_kind(&mut page, BlockId::new(), BlockKind::Quote),
        Err(ModelError::BlockNotFound(_))
    ));
    assert_eq!(commits.load(Ordering::SeqCst), 0);
}

#[test]
fn prop_names_read_well_in_errors() {
    let names: Vec<&str> = [
        BlockProp::Checked(false),
        BlockProp::HeadingLevel(1),
        BlockProp::Language(String::new()),
        BlockProp::ImageWidth(None),
        BlockProp::ImageAlt(String::new()),
    ]
    .iter()
    .map(BlockProp::name)
    .collect();
    assert_eq!(
        names,
        [
            "checked",
            "heading level",
            "language",
            "image width",
            "image alt text"
        ]
    );
}

#[test]
fn a_mark_saved_by_a_newer_version_can_be_carried_and_stripped() {
    // A "newer Leaf" that knows a highlight mark saves a page.
    let id = BlockId::new();
    let newer = loro::LoroDoc::new();
    let mut styles = loro::StyleConfigMap::new();
    styles.insert(
        "highlight".into(),
        loro::StyleConfig {
            expand: loro::ExpandType::After,
        },
    );
    newer.config_text_style(styles);
    let tree = newer.get_tree("blocks");
    tree.enable_fractional_index(0);
    let meta = tree.get_meta(tree.create(None).unwrap()).unwrap();
    meta.insert(keys::ID, id.to_string()).unwrap();
    meta.insert(keys::TYPE, "paragraph").unwrap();
    let text = meta.ensure_mergeable_text(keys::TEXT).unwrap();
    text.insert(0, "glow").unwrap();
    text.mark(0..4, "highlight", "yellow").unwrap();
    newer.commit();

    let mut page = PageDoc::from_bytes(&newer.export(loro::ExportMode::Snapshot).unwrap()).unwrap();
    page.apply(PageCommand::InsertText {
        id,
        pos: 4,
        text: "!".into(),
    })
    .unwrap();
    assert_eq!(
        page.plain_text(id).unwrap().as_deref(),
        Some("glow!"),
        "text with an unknown mark stays editable"
    );
    set_kind(
        &mut page,
        id,
        BlockKind::Code {
            language: String::new(),
        },
    )
    .unwrap();
    let delta = page.text_container(id).unwrap().unwrap().to_delta();
    assert!(
        delta
            .iter()
            .all(|d| matches!(d, loro::TextDelta::Insert { attributes: None, .. })),
        "{delta:?}"
    );
}
