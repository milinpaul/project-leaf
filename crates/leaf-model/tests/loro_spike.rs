//! E00-S05 spike: proves the Loro features `docs/architecture/BLOCK_MODEL.md` relies on.
//!
//! Throwaway quality on purpose: these tests pin down Loro's behaviour before `leaf-model` wraps it.
//! Delete or fold them into real model tests once E02 covers the same ground. Findings are in ADR 0001.

use loro::{
    Container, ExpandType, ExportMode, LoroDoc, LoroError, LoroResult, LoroText, LoroTree, LoroValue,
    StyleConfig, StyleConfigMap, TextDelta, TreeID, TreeParentId, UndoManager, ValueOrContainer,
};

/// The mark table from `BLOCK_MODEL.md`: formatting grows when typing after it; code and links never grow.
fn configure_marks(doc: &LoroDoc) {
    let mut styles = StyleConfigMap::new();
    for (key, expand) in [
        ("bold", ExpandType::After),
        ("italic", ExpandType::After),
        ("strike", ExpandType::After),
        ("code", ExpandType::None),
        ("link", ExpandType::None),
    ] {
        styles.insert(key.into(), StyleConfig { expand });
    }
    doc.config_text_style(styles);
}

/// The text of every run that carries `key`, so tests can see exactly where a mark ends.
fn marked_runs(text: &LoroText, key: &str) -> Vec<String> {
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

/// Adds a block the way `BLOCK_MODEL.md` describes: a tree node whose meta holds `type` and a `text` container.
fn add_block(tree: &LoroTree, parent: impl Into<TreeParentId>, content: &str) -> LoroResult<TreeID> {
    let id = tree.create(parent)?;
    let meta = tree.get_meta(id)?;
    meta.insert("type", "paragraph")?;
    // Mergeable: if two peers ever create it concurrently they get the same container, not two.
    let text = meta.ensure_mergeable_text("text")?;
    text.insert(0, content)?;
    Ok(id)
}

/// Reads a block's existing `text` container without creating one.
fn text_of(tree: &LoroTree, id: TreeID) -> LoroResult<LoroText> {
    match tree.get_meta(id)?.get("text") {
        Some(ValueOrContainer::Container(Container::Text(text))) => Ok(text),
        _ => Err(LoroError::ArgErr("block has no text container".into())),
    }
}

/// A block's `id` stored in its meta, if any.
fn meta_id(tree: &LoroTree, id: TreeID) -> LoroResult<Option<String>> {
    Ok(match tree.get_meta(id)?.get("id") {
        Some(ValueOrContainer::Value(LoroValue::String(s))) => Some(s.to_string()),
        _ => None,
    })
}

/// Export errors are a different type from `LoroError`; fold them in so tests can use `?`.
fn export(doc: &LoroDoc, mode: ExportMode) -> LoroResult<Vec<u8>> {
    doc.export(mode)
        .map_err(|e| LoroError::ArgErr(e.to_string().into()))
}

#[test]
fn tree_keeps_sibling_order_and_moves_nodes() -> LoroResult<()> {
    let doc = LoroDoc::new();
    let tree = doc.get_tree("blocks");
    tree.enable_fractional_index(0);

    let a = add_block(&tree, None, "a")?;
    let b = add_block(&tree, None, "b")?;
    let c = add_block(&tree, None, "c")?;
    let d = tree.create_at(None, 0)?;
    assert_eq!(tree.roots(), vec![d, a, b, c], "create_at inserts at an index");

    tree.mov_after(d, c)?;
    assert_eq!(tree.roots(), vec![a, b, c, d], "mov_after reorders siblings");

    tree.mov_to(b, a, 0)?;
    assert_eq!(tree.roots(), vec![a, c, d], "mov_to nests under a new parent");
    assert_eq!(tree.children(a), Some(vec![b]));
    assert_eq!(
        text_of(&tree, b)?.to_string(),
        "b",
        "a moved block keeps its meta and text"
    );

    let (fa, fc) = (tree.fractional_index(a), tree.fractional_index(c));
    assert!(
        fa.is_some() && fa < fc,
        "fractional indexes sort in sibling order: {fa:?} < {fc:?}"
    );
    Ok(())
}

#[test]
fn marks_follow_the_configured_expand_rules() -> LoroResult<()> {
    let doc = LoroDoc::new();
    configure_marks(&doc);

    let bold = doc.get_text("bold");
    bold.insert(0, "ab")?;
    bold.mark(0..2, "bold", true)?;
    bold.insert(2, "c")?;
    assert_eq!(
        marked_runs(&bold, "bold"),
        vec!["abc"],
        "bold grows when typing right after it"
    );

    let code = doc.get_text("code");
    code.insert(0, "xy")?;
    code.mark(0..2, "code", true)?;
    code.insert(2, "z")?;
    assert_eq!(
        marked_runs(&code, "code"),
        vec!["xy"],
        "inline code does not grow"
    );

    let link = doc.get_text("link");
    link.insert(0, "site")?;
    link.mark(0..4, "link", "https://example.com")?;
    link.insert(4, "!")?;
    link.insert(0, ">")?;
    assert_eq!(
        marked_runs(&link, "link"),
        vec!["site"],
        "links grow on neither side"
    );

    bold.unmark(0..1, "bold")?;
    assert_eq!(
        marked_runs(&bold, "bold"),
        vec!["bc"],
        "unmark removes a mark from part of a range"
    );
    Ok(())
}

#[test]
fn undo_is_exact_but_redo_recreates_blocks_with_new_tree_ids() -> LoroResult<()> {
    let doc = LoroDoc::new();
    let tree = doc.get_tree("blocks");
    tree.enable_fractional_index(0);
    let mut undo = UndoManager::new(&doc);
    undo.set_merge_interval(0); // every commit is its own undo step
    let v0 = doc.get_deep_value();

    let a = add_block(&tree, None, "Hello")?;
    doc.commit();
    let v1 = doc.get_deep_value();

    text_of(&tree, a)?.insert(5, ", world")?;
    doc.commit();
    let v2 = doc.get_deep_value();

    let b = add_block(&tree, None, "child")?;
    tree.mov_to(b, a, 0)?;
    doc.commit();

    assert!(undo.undo()?);
    assert_eq!(doc.get_deep_value(), v2, "undo reverses a tree insert + move");
    assert!(undo.undo()?);
    assert_eq!(doc.get_deep_value(), v1, "undo reverses a text insert");
    assert!(undo.undo()?);
    assert_eq!(doc.get_deep_value(), v0, "undo reverses block creation");
    assert!(!undo.undo()?, "nothing left to undo");

    for _ in 0..3 {
        assert!(undo.redo()?);
    }
    // FINDING: content and shape come back, but re-created nodes get new TreeIDs.
    let roots = tree.roots();
    assert_eq!(roots.len(), 1);
    assert_ne!(roots[0], a, "redo re-creates the block under a new TreeID");
    assert_eq!(text_of(&tree, roots[0])?.to_string(), "Hello, world");
    let children = tree.children(roots[0]).unwrap_or_default();
    assert_eq!(children.len(), 1);
    assert_eq!(text_of(&tree, children[0])?.to_string(), "child");
    Ok(())
}

#[test]
fn quick_commits_merge_into_one_undo_step() -> LoroResult<()> {
    let doc = LoroDoc::new();
    let text = doc.get_text("t");
    let mut undo = UndoManager::new(&doc);
    undo.set_merge_interval(60_000); // typing within the interval groups into one step
    for (i, ch) in ["a", "b", "c"].into_iter().enumerate() {
        text.insert(i, ch)?;
        doc.commit();
    }
    assert!(undo.undo()?);
    assert_eq!(text.to_string(), "", "one undo removes the whole burst of typing");
    Ok(())
}

#[test]
fn two_peers_converge_by_exchanging_updates() -> LoroResult<()> {
    let (left, right) = (LoroDoc::new(), LoroDoc::new());
    left.set_peer_id(1)?;
    right.set_peer_id(2)?;
    for doc in [&left, &right] {
        configure_marks(doc);
        doc.get_tree("blocks").enable_fractional_index(0);
    }

    // Shared starting point: two blocks, x and y, both peers have them.
    let tree = left.get_tree("blocks");
    let x = add_block(&tree, None, "x")?;
    let y = add_block(&tree, None, "y")?;
    left.commit();
    right.import(&export(&left, ExportMode::all_updates())?)?;

    // Concurrent edits, including moves that would form a cycle if both applied naively.
    left.get_tree("blocks").mov_to(x, y, 0)?;
    add_block(&left.get_tree("blocks"), None, "from left")?;
    right.get_tree("blocks").mov_to(y, x, 0)?;
    add_block(&right.get_tree("blocks"), None, "from right")?;
    left.commit();
    right.commit();

    let to_right = export(&left, ExportMode::updates(&right.oplog_vv()))?;
    let to_left = export(&right, ExportMode::updates(&left.oplog_vv()))?;
    right.import(&to_right)?;
    left.import(&to_left)?;

    assert_eq!(
        left.get_deep_value(),
        right.get_deep_value(),
        "both peers end in the same state"
    );
    let tree = left.get_tree("blocks");
    let x_under_y = tree.children(y).unwrap_or_default().contains(&x);
    let y_under_x = tree.children(x).unwrap_or_default().contains(&y);
    assert!(!(x_under_y && y_under_x), "concurrent moves never create a cycle");
    assert_eq!(
        tree.roots().len(),
        3,
        "one of x/y stays a root, plus one new block from each peer"
    );
    Ok(())
}

#[test]
fn undoing_a_delete_restores_content_and_meta_but_not_tree_ids() -> LoroResult<()> {
    let doc = LoroDoc::new();
    let tree = doc.get_tree("blocks");
    tree.enable_fractional_index(0);
    let mut undo = UndoManager::new(&doc);
    undo.set_merge_interval(0);
    let a = add_block(&tree, None, "parent")?;
    let child = add_block(&tree, a, "child")?;
    tree.get_meta(a)?.insert("id", "stable-a")?;
    doc.commit();

    tree.delete(a)?;
    doc.commit();
    assert!(undo.undo()?);

    let roots = tree.roots();
    assert_eq!(roots.len(), 1, "one undo restores the block and its children");
    let restored = roots[0];
    let restored_child = tree.children(restored).unwrap_or_default();
    assert_eq!(text_of(&tree, restored)?.to_string(), "parent");
    assert_eq!(restored_child.len(), 1);
    assert_eq!(text_of(&tree, restored_child[0])?.to_string(), "child");
    // FINDING: the TreeIDs change, but an ID stored in the block's meta survives.
    assert_ne!(restored, a);
    assert_ne!(restored_child[0], child);
    assert_eq!(meta_id(&tree, restored)?, Some("stable-a".to_owned()));

    assert!(undo.redo()?);
    assert!(undo.undo()?);
    let again = tree.roots();
    assert_ne!(again[0], restored, "every undo of a delete mints new TreeIDs");
    assert_eq!(
        meta_id(&tree, again[0])?,
        Some("stable-a".to_owned()),
        "the meta ID is stable across cycles"
    );
    Ok(())
}
