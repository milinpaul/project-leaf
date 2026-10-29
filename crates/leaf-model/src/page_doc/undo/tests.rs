//! Tests for page undo and redo with caret metadata (E02-S08). States are compared as outlines
//! (block IDs, nesting, kinds, formatted text), never by Loro `TreeID`s, which undo may change.

use super::{Caret, History, SavedSelection};
use crate::{BlockKind, BlockProp, Mark, MarkKind, PageCommand, PageDoc, PageOutcome, TextSpan};
use leaf_core::BlockId;

type Outline = Vec<(usize, BlockId, BlockKind, Option<Vec<TextSpan>>)>;

fn outline(page: &PageDoc) -> Outline {
    fn walk(page: &PageDoc, blocks: Vec<BlockId>, depth: usize, out: &mut Outline) {
        for block in blocks {
            out.push((depth, block, page.kind(block).unwrap(), page.text(block).unwrap()));
            walk(page, page.children(block).unwrap(), depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(page, page.root_blocks(), 0, &mut out);
    out
}

fn fresh() -> PageDoc {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    page
}

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

/// Applies `command`, then checks undo restores the outline before it and redo the one after,
/// with the invariants holding throughout.
fn assert_undoable(page: &mut PageDoc, command: &PageCommand) {
    let before = outline(page);
    page.apply(command.clone()).unwrap();
    let after = outline(page);
    assert!(
        matches!(page.undo().unwrap(), History::Done { .. }),
        "{command:?} must be undoable"
    );
    assert_eq!(outline(page), before, "undo of {command:?}");
    assert_eq!(page.check_invariants(), Ok(()));
    assert!(matches!(page.redo().unwrap(), History::Done { .. }));
    assert_eq!(outline(page), after, "redo of {command:?}");
    assert_eq!(page.check_invariants(), Ok(()));
}

#[test]
fn every_block_command_undoes_and_redoes_exactly() {
    let mut page = fresh();
    let a = add(&mut page, None, BlockKind::Paragraph, "alpha beta");
    let b = add(&mut page, None, BlockKind::BulletedList, "bullet");
    let c = add(&mut page, Some(b), BlockKind::Paragraph, "child");

    let commands = [
        PageCommand::InsertBlock {
            parent: Some(b),
            index: 0,
            kind: BlockKind::Quote,
            text: "q".into(),
        },
        PageCommand::InsertText {
            id: a,
            pos: 5,
            text: " 👋🏽".into(),
        },
        PageCommand::DeleteText { id: a, range: 0..2 },
        PageCommand::SetMark {
            id: a,
            range: 0..3,
            mark: Mark::Link("https://leaf.app".into()),
        },
        PageCommand::RemoveMark {
            id: a,
            range: 0..1,
            kind: MarkKind::Link,
        },
        PageCommand::MoveBlocks {
            ids: vec![c],
            new_parent: None,
            index: 0,
        },
        PageCommand::Indent { ids: vec![b] },
        PageCommand::Outdent { ids: vec![b] },
        PageCommand::SetBlockKind {
            id: b,
            kind: BlockKind::Heading { level: 2 },
        },
        PageCommand::SetProp {
            id: b,
            prop: BlockProp::HeadingLevel(3),
        },
        PageCommand::SplitBlock { id: a, pos: 3 },
        PageCommand::DeleteBlocks { ids: vec![b] },
    ];
    for command in &commands {
        assert_undoable(&mut page, command);
    }
    let second = page.root_blocks()[1];
    assert_undoable(&mut page, &PageCommand::MergeIntoPrevious { id: second });
}

#[test]
fn undoing_a_delete_brings_blocks_back_under_their_ids_and_editable() {
    let mut page = fresh();
    let parent = add(&mut page, None, BlockKind::Paragraph, "parent");
    let child = add(
        &mut page,
        Some(parent),
        BlockKind::Todo { checked: true },
        "child",
    );
    page.apply(PageCommand::DeleteBlocks { ids: vec![parent] })
        .unwrap();
    assert!(!page.contains(child));

    page.undo().unwrap();
    assert!(
        page.contains(parent) && page.contains(child),
        "same BlockIds, though Loro re-created the nodes"
    );
    assert_eq!(page.children(parent).unwrap(), vec![child]);
    page.apply(PageCommand::InsertText {
        id: child,
        pos: 5,
        text: "!".into(),
    })
    .unwrap();
    assert_eq!(page.plain_text(child).unwrap().as_deref(), Some("child!"));
}

#[test]
fn quick_typing_undoes_as_one_step_by_default() {
    let mut page = PageDoc::new();
    let id = add(&mut page, None, BlockKind::Paragraph, "");
    for (pos, ch) in ["l", "e", "a", "f"].into_iter().enumerate() {
        page.apply(PageCommand::InsertText {
            id,
            pos,
            text: ch.into(),
        })
        .unwrap();
    }
    assert!(matches!(page.undo().unwrap(), History::Done { .. }));
    assert!(
        page.is_empty(),
        "the burst undoes together with creating the block"
    );
}

#[test]
fn undo_hands_back_the_selection_before_a_step_and_redo_the_one_after() {
    let mut page = fresh();
    let id = add(&mut page, None, BlockKind::Paragraph, "");
    let caret = |at| Caret { block: id, at };

    page.set_caret(Some(caret(0)));
    page.apply(PageCommand::InsertText {
        id,
        pos: 0,
        text: "ab".into(),
    })
    .unwrap();
    page.set_caret(Some(caret(2)));
    page.apply(PageCommand::InsertText {
        id,
        pos: 2,
        text: "cd".into(),
    })
    .unwrap();
    page.record_after(Some(SavedSelection::at(caret(4))));

    // Where the caret is when undo is pressed doesn't matter.
    page.set_caret(Some(caret(1)));
    assert_eq!(
        page.undo().unwrap(),
        History::Done {
            selection: Some(SavedSelection::at(caret(2)))
        },
        "back to before the change"
    );
    page.set_caret(Some(caret(1)));
    assert_eq!(
        page.redo().unwrap(),
        History::Done {
            selection: Some(SavedSelection::at(caret(4)))
        },
        "back to right after the change"
    );

    page.set_caret(None);
    page.apply(PageCommand::InsertText {
        id,
        pos: 0,
        text: "x".into(),
    })
    .unwrap();
    assert_eq!(
        page.undo().unwrap(),
        History::Done { selection: None },
        "no caret recorded, none returned"
    );
}

#[test]
fn nothing_to_undo_or_redo_and_loaded_history_is_not_undoable() {
    let mut page = fresh();
    assert_eq!(page.undo().unwrap(), History::Nothing);
    assert_eq!(page.redo().unwrap(), History::Nothing);
    add(&mut page, None, BlockKind::Paragraph, "saved");
    let mut loaded = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    assert_eq!(
        loaded.undo().unwrap(),
        History::Nothing,
        "undo starts from the loaded state"
    );
    assert_eq!(loaded.len(), 1);
}

#[test]
fn redoing_a_kind_change_on_a_re_created_block_keeps_its_properties() {
    // Regression (ADR 0007): undo both steps, then redo both. Redoing the insert re-creates the block
    // under a new TreeID; the kind change's properties must still land on it.
    for to in [
        BlockKind::Todo { checked: true },
        BlockKind::Heading { level: 2 },
        BlockKind::Code {
            language: "rust".into(),
        },
        BlockKind::Image {
            asset: leaf_core::AssetId::of(b"i"),
            width: Some(64.0),
            alt: "a".into(),
        },
    ] {
        let mut page = fresh();
        let id = add(&mut page, None, BlockKind::Paragraph, "");
        page.apply(PageCommand::SetBlockKind { id, kind: to.clone() })
            .unwrap();
        for _ in 0..2 {
            page.undo().unwrap();
        }
        for _ in 0..2 {
            page.redo().unwrap();
        }
        assert_eq!(page.kind(id).unwrap(), to);
        assert_eq!(page.check_invariants(), Ok(()));
    }
}

#[test]
fn several_commands_as_one_step_undo_and_redo_together() {
    let mut page = fresh();
    let id = add(&mut page, None, BlockKind::Paragraph, "hello world");
    let before = outline(&page);
    page.set_caret(Some(Caret { block: id, at: 5 }));
    let created = page
        .as_one_step(|page| {
            page.apply(PageCommand::DeleteText { id, range: 5..11 })?;
            page.apply(PageCommand::SplitBlock { id, pos: 5 })
        })
        .unwrap();
    assert!(matches!(created, PageOutcome::BlockCreated(_)));
    let after = outline(&page);
    assert_eq!(after.len(), 2);

    assert_eq!(
        page.undo().unwrap(),
        History::Done {
            selection: Some(SavedSelection::at(Caret { block: id, at: 5 }))
        }
    );
    assert_eq!(outline(&page), before, "both commands undone at once");
    page.redo().unwrap();
    assert_eq!(outline(&page), after);

    // The group ended: the next command is a step of its own.
    page.apply(PageCommand::InsertText {
        id,
        pos: 0,
        text: ">".into(),
    })
    .unwrap();
    page.undo().unwrap();
    assert_eq!(outline(&page), after);
}

#[test]
fn a_failing_step_still_ends_the_group() {
    let mut page = fresh();
    let id = add(&mut page, None, BlockKind::Paragraph, "abc");
    let result = page.as_one_step(|page| {
        page.apply(PageCommand::InsertText {
            id,
            pos: 3,
            text: "d".into(),
        })?;
        page.apply(PageCommand::DeleteText { id, range: 0..99 })
    });
    assert!(result.is_err());
    assert_eq!(
        page.plain_text(id).unwrap().as_deref(),
        Some("abcd"),
        "the first step stays"
    );
    // A new group can start, so the first one was closed.
    page.as_one_step(|page| {
        page.apply(PageCommand::InsertText {
            id,
            pos: 0,
            text: "_".into(),
        })
    })
    .unwrap();
    page.undo().unwrap();
    assert_eq!(page.plain_text(id).unwrap().as_deref(), Some("abcd"));
}

#[test]
fn a_whole_selection_survives_undo_redo_cycles_and_merged_steps() {
    let mut page = fresh();
    let a = add(&mut page, None, BlockKind::Paragraph, "hello");
    let b = add(&mut page, None, BlockKind::Paragraph, "world");
    let before = SavedSelection {
        caret: Caret { block: b, at: 2 },
        tail: Some(Caret { block: a, at: 1 }),
        blocks: Vec::new(),
    };
    let after = SavedSelection::at(Caret { block: a, at: 2 });
    page.set_selection(Some(before.clone()));
    page.as_one_step(|page| {
        page.apply(PageCommand::DeleteText { id: a, range: 1..5 })?;
        page.apply(PageCommand::DeleteText { id: b, range: 0..2 })
    })
    .unwrap();
    page.record_after(Some(after.clone()));
    for _ in 0..3 {
        assert_eq!(
            page.undo().unwrap(),
            History::Done {
                selection: Some(before.clone())
            }
        );
        assert_eq!(
            page.redo().unwrap(),
            History::Done {
                selection: Some(after.clone())
            }
        );
    }
    assert_eq!(
        History::Done {
            selection: Some(after)
        }
        .caret(),
        Some(Caret { block: a, at: 2 })
    );
    assert_eq!(History::Nothing.caret(), None);
}

#[test]
fn typing_merged_into_one_step_keeps_the_first_before_and_the_last_after() {
    let mut page = fresh();
    let a = add(&mut page, None, BlockKind::Paragraph, "ab");
    // A freshly loaded page has no history, so the typing below starts a step of its own.
    let mut page = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    page.set_undo_merge_interval(60_000);
    for (i, ch) in ["x", "y"].into_iter().enumerate() {
        page.set_caret(Some(Caret { block: a, at: 1 + i }));
        page.apply(PageCommand::InsertText {
            id: a,
            pos: 1 + i,
            text: ch.into(),
        })
        .unwrap();
        page.record_after(Some(SavedSelection::at(Caret { block: a, at: 2 + i })));
    }
    assert_eq!(page.undo().unwrap().caret(), Some(Caret { block: a, at: 1 }));
    assert_eq!(page.plain_text(a).unwrap().as_deref(), Some("ab"), "one step");
    assert_eq!(page.redo().unwrap().caret(), Some(Caret { block: a, at: 3 }));
}

#[test]
fn recording_after_with_no_steps_or_a_step_without_after_falls_back() {
    let mut page = fresh();
    page.record_after(Some(SavedSelection::at(Caret {
        block: BlockId::new(),
        at: 0,
    })));
    let id = add(&mut page, None, BlockKind::Paragraph, "");
    page.set_caret(Some(Caret { block: id, at: 0 }));
    page.apply(PageCommand::InsertText {
        id,
        pos: 0,
        text: "a".into(),
    })
    .unwrap();
    page.undo().unwrap();
    assert_eq!(
        page.redo().unwrap().caret(),
        Some(Caret { block: id, at: 0 }),
        "no 'after' recorded: redo uses the 'before'"
    );
}

#[test]
fn recording_after_an_action_that_changed_nothing_leaves_the_last_step_alone() {
    let mut page = fresh();
    let id = add(&mut page, None, BlockKind::Paragraph, "");
    page.set_caret(Some(Caret { block: id, at: 0 }));
    page.apply(PageCommand::InsertText {
        id,
        pos: 0,
        text: "abc".into(),
    })
    .unwrap();
    page.record_after(Some(SavedSelection::at(Caret { block: id, at: 3 })));
    // A key that did nothing: the caret moved, the page didn't.
    page.set_caret(Some(Caret { block: id, at: 0 }));
    page.record_after(Some(SavedSelection::at(Caret { block: id, at: 0 })));
    page.undo().unwrap();
    assert_eq!(page.redo().unwrap().caret(), Some(Caret { block: id, at: 3 }));
}

#[test]
fn a_separate_step_merges_with_neither_the_typing_before_nor_after_it() {
    let mut page = fresh();
    let id = add(&mut page, None, BlockKind::Paragraph, "");
    let mut page = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    page.set_undo_merge_interval(60_000);
    let typed = |page: &mut PageDoc, pos, text: &str| {
        page.apply(PageCommand::InsertText {
            id,
            pos,
            text: text.into(),
        })
        .unwrap();
    };
    typed(&mut page, 0, "#");
    typed(&mut page, 1, " ");
    page.as_separate_step(|page| {
        page.apply(PageCommand::DeleteText { id, range: 0..2 })?;
        page.apply(PageCommand::SetBlockKind {
            id,
            kind: BlockKind::Heading { level: 1 },
        })
    })
    .unwrap();
    typed(&mut page, 0, "T");
    typed(&mut page, 1, "o");

    page.undo().unwrap();
    assert_eq!(
        page.plain_text(id).unwrap().as_deref(),
        Some(""),
        "the typing after, as one step"
    );
    assert_eq!(page.kind(id).unwrap(), BlockKind::Heading { level: 1 });
    page.undo().unwrap();
    assert_eq!(
        page.plain_text(id).unwrap().as_deref(),
        Some("# "),
        "the conversion alone"
    );
    assert_eq!(page.kind(id).unwrap(), BlockKind::Paragraph);
    page.undo().unwrap();
    assert_eq!(
        page.plain_text(id).unwrap().as_deref(),
        Some(""),
        "the typing before"
    );
    assert_eq!(page.undo().unwrap(), History::Nothing);
}
