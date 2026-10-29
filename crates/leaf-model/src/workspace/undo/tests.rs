//! Tests for workspace undo and redo. States are compared as outlines (every page, its parent and
//! its metadata, trash included), never by Loro `TreeID`s, which undo is allowed to change (ADR 0006).

use leaf_core::PageId;

use crate::{Outcome, PageMeta, Workspace, WorkspaceCommand};

/// Every page with its parent and metadata: the visible tree, then each trashed page's subtree.
fn outline(ws: &Workspace) -> Vec<(Option<PageId>, PageMeta)> {
    fn walk(
        ws: &Workspace,
        parent: Option<PageId>,
        pages: Vec<PageId>,
        out: &mut Vec<(Option<PageId>, PageMeta)>,
    ) {
        for page in pages {
            out.push((parent, ws.page(page).unwrap()));
            walk(ws, Some(page), ws.children(page).unwrap(), out);
        }
    }
    let mut out = Vec::new();
    walk(ws, None, ws.root_pages().unwrap(), &mut out);
    for page in ws.trash().unwrap() {
        walk(ws, None, vec![page], &mut out);
    }
    out
}

fn fresh() -> Workspace {
    let mut ws = Workspace::new();
    ws.set_undo_merge_interval(0);
    ws
}

fn create(ws: &mut Workspace, parent: Option<PageId>, index: usize, title: &str) -> PageId {
    let command = WorkspaceCommand::CreatePage {
        parent,
        index,
        title: title.into(),
        created_at: 0,
    };
    match ws.apply(command).unwrap() {
        Outcome::PageCreated(page) => page,
        other => panic!("expected a new page, got {other:?}"),
    }
}

/// Applies `command`, then checks undo restores the outline before it and redo the one after it,
/// and that page lookups still work afterwards (the index is rebuilt).
fn assert_undoable(ws: &mut Workspace, command: &WorkspaceCommand) {
    let before = outline(ws);
    ws.apply(command.clone()).unwrap();
    let after = outline(ws);
    assert!(ws.undo().unwrap(), "{command:?} must be undoable");
    assert_eq!(outline(ws), before, "undo of {command:?}");
    assert!(ws.redo().unwrap());
    assert_eq!(outline(ws), after, "redo of {command:?}");
}

#[test]
fn every_page_command_undoes_and_redoes_exactly() {
    let mut ws = fresh();
    let a = create(&mut ws, None, 0, "A");
    let b = create(&mut ws, None, 1, "B");
    let child = create(&mut ws, Some(a), 0, "Child");

    assert_undoable(
        &mut ws,
        &WorkspaceCommand::CreatePage {
            parent: Some(b),
            index: 0,
            title: "New".into(),
            created_at: 5,
        },
    );
    assert_undoable(
        &mut ws,
        &WorkspaceCommand::RenamePage {
            page: a,
            title: "Alpha".into(),
        },
    );
    assert_undoable(
        &mut ws,
        &WorkspaceCommand::SetPageIcon {
            page: a,
            icon: Some("🌱".into()),
        },
    );
    assert_undoable(&mut ws, &WorkspaceCommand::SetPageIcon { page: a, icon: None });
    assert_undoable(
        &mut ws,
        &WorkspaceCommand::SetPageCover {
            page: a,
            cover: Some("builtin:dawn".into()),
        },
    );
    assert_undoable(&mut ws, &WorkspaceCommand::SetPageCover { page: a, cover: None });
    assert_undoable(
        &mut ws,
        &WorkspaceCommand::MovePage {
            page: child,
            new_parent: Some(b),
            index: 0,
        },
    );
    assert_undoable(
        &mut ws,
        &WorkspaceCommand::MovePage {
            page: b,
            new_parent: None,
            index: 0,
        },
    );
    assert_undoable(&mut ws, &WorkspaceCommand::TrashPage { page: a, at: 10 });
    assert_undoable(&mut ws, &WorkspaceCommand::RestorePage { page: a });
    ws.apply(WorkspaceCommand::TrashPage { page: b, at: 20 }).unwrap();
    assert_undoable(&mut ws, &WorkspaceCommand::DeletePagePermanently { page: b });
}

#[test]
fn undo_brings_back_deleted_pages_under_their_ids() {
    let mut ws = fresh();
    let page = create(&mut ws, None, 0, "Gone");
    let child = create(&mut ws, Some(page), 0, "Child");
    ws.apply(WorkspaceCommand::TrashPage { page, at: 1 }).unwrap();
    ws.apply(WorkspaceCommand::DeletePagePermanently { page })
        .unwrap();
    assert!(!ws.contains(page));

    assert!(ws.undo().unwrap());
    assert!(
        ws.contains(page) && ws.contains(child),
        "same PageIds, even though Loro re-created the nodes"
    );
    assert_eq!(ws.children(page).unwrap(), vec![child]);
    ws.apply(WorkspaceCommand::RenamePage {
        page: child,
        title: "Still editable".into(),
    })
    .unwrap();
    assert_eq!(ws.page(child).unwrap().title, "Still editable");
}

#[test]
fn undo_and_redo_report_when_there_is_nothing_to_do() {
    let mut ws = fresh();
    assert!(!ws.undo().unwrap());
    assert!(!ws.redo().unwrap());
    create(&mut ws, None, 0, "A");
    assert!(ws.undo().unwrap());
    assert!(!ws.undo().unwrap());
    assert!(ws.redo().unwrap());
    assert!(!ws.redo().unwrap());
}

#[test]
fn quick_commands_share_one_undo_step_by_default() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "");
    for title in ["I", "Id", "Ide", "Idea"] {
        ws.apply(WorkspaceCommand::RenamePage {
            page,
            title: title.into(),
        })
        .unwrap();
    }
    assert!(ws.undo().unwrap());
    assert!(
        ws.is_empty(),
        "typing within the merge interval undoes together with creating the page"
    );
}

#[test]
fn loaded_history_is_not_undoable() {
    let mut ws = fresh();
    create(&mut ws, None, 0, "Saved");
    let mut loaded = Workspace::from_bytes(&ws.to_bytes().unwrap()).unwrap();
    assert!(!loaded.undo().unwrap(), "undo starts from the loaded state");
    assert_eq!(loaded.len(), 1);
}
