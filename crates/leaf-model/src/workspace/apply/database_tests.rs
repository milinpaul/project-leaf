//! Tests for database pages and their row pages in the workspace (E09-S03), and for commands applied
//! outside undo.

use leaf_core::PageId;

use super::tests::create;
use crate::{ModelError, Outcome, Workspace, WorkspaceCommand};

fn database_page(ws: &mut Workspace, title: &str) -> (PageId, leaf_core::DatabaseId) {
    match ws.apply(WorkspaceCommand::CreateDatabasePage {
        parent: None,
        index: ws.root_pages().unwrap().len(),
        title: title.into(),
        created_at: 5,
    }) {
        Ok(Outcome::DatabasePageCreated { page, database }) => (page, database),
        other => panic!("expected a database page, got {other:?}"),
    }
}

fn row_page(ws: &mut Workspace, database_page: PageId) -> PageId {
    match ws.apply_unrecorded(WorkspaceCommand::CreateRowPage {
        database_page,
        created_at: 6,
    }) {
        Ok(Outcome::PageCreated(page)) => page,
        other => panic!("expected a row page, got {other:?}"),
    }
}

#[test]
fn a_database_page_records_its_database_and_rows_record_theirs() {
    let mut ws = Workspace::new();
    let notes = create(&mut ws, None, 0, "Notes");
    let (tasks, database) = database_page(&mut ws, "Tasks");
    let meta = ws.page(tasks).unwrap();
    assert_eq!(
        (meta.title.as_str(), meta.database, meta.row_of),
        ("Tasks", Some(database), None)
    );
    assert_eq!(ws.page(notes).unwrap().database, None);

    let first = row_page(&mut ws, tasks);
    let second = row_page(&mut ws, tasks);
    assert_eq!(
        ws.children(tasks).unwrap(),
        [first, second],
        "rows go last, in order"
    );
    let row = ws.page(first).unwrap();
    assert_eq!(
        (row.title.as_str(), row.row_of, row.database),
        ("", Some(database), None)
    );
    assert!(matches!(
        ws.apply(WorkspaceCommand::CreateRowPage {
            database_page: notes,
            created_at: 7,
        }),
        Err(ModelError::NotADatabasePage(_))
    ));

    ws.apply(WorkspaceCommand::TrashPage { page: second, at: 8 })
        .unwrap();
    let mut rows = ws.row_pages(database).unwrap();
    rows.sort();
    assert_eq!(rows, [first, second], "trashed rows included");
    assert_eq!(ws.row_pages(leaf_core::DatabaseId::new()).unwrap(), Vec::new());

    let loaded = Workspace::from_bytes(&ws.to_bytes().unwrap()).unwrap();
    assert_eq!(loaded.page(first).unwrap().row_of, Some(database));
    assert_eq!(loaded.page(tasks).unwrap().database, Some(database));
}

#[test]
fn creating_a_database_page_is_one_undo_step() {
    let mut ws = Workspace::new();
    ws.set_undo_merge_interval(0);
    let (tasks, _) = database_page(&mut ws, "Tasks");
    assert!(ws.undo().unwrap());
    assert!(ws.page(tasks).is_err());
    assert!(ws.redo().unwrap());
    assert!(ws.page(tasks).unwrap().database.is_some());
}

#[test]
fn unrecorded_commands_are_skipped_by_undo() {
    let mut ws = Workspace::new();
    ws.set_undo_merge_interval(0);
    let a = create(&mut ws, None, 0, "A");
    let b = create(&mut ws, None, 1, "B");
    ws.apply(WorkspaceCommand::RenamePage {
        page: a,
        title: "A2".into(),
    })
    .unwrap();
    ws.apply_unrecorded(WorkspaceCommand::TrashPage { page: b, at: 9 })
        .unwrap();
    assert!(ws.undo().unwrap(), "undoes the rename, not the trash");
    assert_eq!(ws.page(a).unwrap().title, "A");
    assert!(ws.is_in_trash(b).unwrap(), "the unrecorded trash stays");
    // A refused unrecorded command leaves the next command recorded as usual.
    assert!(
        ws.apply_unrecorded(WorkspaceCommand::RenamePage {
            page: PageId::new(),
            title: "x".into(),
        })
        .is_err()
    );
    ws.apply(WorkspaceCommand::RenamePage {
        page: a,
        title: "A3".into(),
    })
    .unwrap();
    assert!(ws.undo().unwrap());
    assert_eq!(ws.page(a).unwrap().title, "A");
}

#[test]
fn a_database_field_that_is_not_an_id_is_an_error_not_ignored() {
    let doc = loro::LoroDoc::new();
    let tree = doc.get_tree("pages");
    tree.enable_fractional_index(0);
    let node = tree.create(loro::TreeParentId::Root).unwrap();
    let meta = tree.get_meta(node).unwrap();
    let page = PageId::new();
    meta.insert("id", page.to_string()).unwrap();
    meta.insert("title", "Odd").unwrap();
    meta.insert("created_at", 1_i64).unwrap();
    meta.insert("row_of", "nope").unwrap();
    doc.commit();
    let ws = Workspace::from_bytes(&doc.export(loro::ExportMode::Snapshot).unwrap()).unwrap();
    assert!(matches!(
        ws.page(page),
        Err(ModelError::InvalidPageMeta { field: "row_of", .. })
    ));
}
