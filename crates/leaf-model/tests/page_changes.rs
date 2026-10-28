//! Which blocks a change touched, for the editor's view (E04-S01).

use leaf_model::{BlockKind, Changes, PageCommand, PageDoc};

#[test]
fn changes_name_the_touched_blocks_and_whether_the_outline_moved() {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    let insert = |page: &mut PageDoc, text: &str| match page.apply(PageCommand::InsertBlock {
        parent: None,
        index: 0,
        kind: BlockKind::Paragraph,
        text: text.into(),
    }) {
        Ok(leaf_model::PageOutcome::BlockCreated(id)) => id,
        other => panic!("{other:?}"),
    };
    let a = insert(&mut page, "a");
    let changes = page.take_changes();
    assert!(changes.structure && changes.blocks.contains(&a));
    assert_eq!(page.take_changes(), Changes::default(), "taken once");

    let b = insert(&mut page, "b");
    page.take_changes();
    page.apply(PageCommand::InsertText {
        id: a,
        pos: 1,
        text: "!".into(),
    })
    .unwrap();
    let changes = page.take_changes();
    assert_eq!(changes.blocks, [a].into_iter().collect());
    assert!(!changes.structure, "typing doesn't move blocks");

    page.apply(PageCommand::DeleteBlocks { ids: vec![b] }).unwrap();
    let changes = page.take_changes();
    assert!(
        changes.structure && !changes.blocks.contains(&b),
        "deleted blocks aren't listed"
    );

    page.undo().unwrap();
    let changes = page.take_changes();
    assert!(
        changes.structure && changes.blocks.contains(&b),
        "undo brings b back, under its id"
    );

    let loaded = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    assert_eq!(
        loaded.take_changes(),
        Changes::default(),
        "loading isn't a change"
    );
}
