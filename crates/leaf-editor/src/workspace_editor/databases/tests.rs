//! Database pages and their rows in the workspace editor (E09-S03).

use leaf_core::PageId;
use leaf_model::{Database, Workspace, WorkspaceCommand};

use crate::{EditorError, WorkspaceEditor};

/// An editor with a page "Notes" and a database page "Tasks" (rows A, B, C) after it.
fn editor() -> (WorkspaceEditor, PageId, Vec<PageId>) {
    let mut ws = Workspace::new();
    ws.set_undo_merge_interval(0);
    let mut editor = WorkspaceEditor::new(ws);
    editor.new_page_in(None, 1).unwrap();
    let (tasks, database) = editor.new_database_page(None, 2).unwrap();
    editor
        .databases
        .get_mut(&database)
        .unwrap()
        .set_undo_merge_interval(0);
    let rows = (0..3).map(|i| editor.add_row(tasks, i, 3).unwrap()).collect();
    (editor, tasks, rows)
}

fn rows_of(editor: &WorkspaceEditor, page: PageId) -> Vec<PageId> {
    let id = editor.database_of(page).unwrap();
    editor.database(id).unwrap().rows()
}

fn trashed(editor: &WorkspaceEditor, page: PageId) -> bool {
    editor.workspace().is_in_trash(page).unwrap()
}

#[test]
fn a_database_page_has_its_database_ready_and_rows_are_pages_under_it() {
    let (editor, tasks, rows) = editor();
    assert_eq!(rows_of(&editor, tasks), rows);
    for &row in &rows {
        let meta = editor.workspace().page(row).unwrap();
        assert_eq!(meta.row_of, Some(editor.database_of(tasks).unwrap()));
        assert_eq!(editor.workspace().parent(row).unwrap(), Some(tasks));
    }
    assert_eq!(editor.loaded_databases().count(), 1);
    // A new database starts with a Tags column, and nothing to undo.
    let id = editor.database_of(tasks).unwrap();
    let columns = editor.database(id).unwrap().columns().unwrap();
    assert_eq!(columns.len(), 1);
    assert_eq!(
        (columns[0].name.as_str(), &columns[0].kind),
        ("Tags", &leaf_model::PropertyType::MultiSelect)
    );
    // Not a database page: no rows.
    let notes = editor.workspace().root_pages().unwrap()[0];
    assert!(matches!(
        editor.database_of(notes),
        Err(EditorError::Model(leaf_model::ModelError::NotADatabasePage(_)))
    ));
}

#[test]
fn the_sidebar_leaves_rows_out() {
    let (mut editor, tasks, _) = editor();
    editor.set_expanded(tasks, true);
    let labels: Vec<(String, bool)> = editor
        .sidebar()
        .unwrap()
        .into_iter()
        .map(|item| (item.label, item.has_children))
        .collect();
    assert_eq!(
        labels,
        [("Untitled".to_owned(), false), ("Untitled".to_owned(), false)],
        "Notes and Tasks only; Tasks shows no children of its own"
    );
}

#[test]
fn deleting_a_row_trashes_its_page_and_undo_and_redo_follow_in_one_step() {
    let (mut editor, tasks, rows) = editor();
    editor.delete_row(tasks, rows[1], 10).unwrap();
    assert_eq!(rows_of(&editor, tasks), [rows[0], rows[2]]);
    assert!(trashed(&editor, rows[1]));
    assert_eq!(editor.workspace().page(rows[1]).unwrap().trashed_at, Some(10));

    assert!(editor.undo_database(tasks, 11).unwrap());
    assert_eq!(rows_of(&editor, tasks), rows, "back in place");
    assert!(!trashed(&editor, rows[1]), "its page comes back with it");
    assert!(editor.redo_database(tasks, 12).unwrap());
    assert!(trashed(&editor, rows[1]));

    // Undoing the adds removes the rows and trashes their pages.
    assert!(editor.undo_database(tasks, 13).unwrap()); // the delete again
    for _ in 0..3 {
        assert!(editor.undo_database(tasks, 14).unwrap());
    }
    assert_eq!(rows_of(&editor, tasks), Vec::new());
    assert!(rows.iter().all(|&row| trashed(&editor, row)));
    // The workspace's own undo never touches the row pages' bookkeeping.
    assert!(
        editor.workspace.undo().unwrap(),
        "undoes the database page itself"
    );
}

#[test]
fn restoring_a_row_page_from_the_trash_restores_its_row_where_it_was() {
    let (mut editor, tasks, rows) = editor();
    editor.delete_row(tasks, rows[0], 10).unwrap();
    editor.restore(rows[0]).unwrap();
    assert_eq!(rows_of(&editor, tasks), rows, "first again");
    assert!(!trashed(&editor, rows[0]));

    // A row whose add was undone comes back at the end.
    let extra = editor.add_row(tasks, 0, 20).unwrap();
    editor.undo_database(tasks, 21).unwrap();
    assert!(trashed(&editor, extra));
    editor.restore(extra).unwrap();
    assert_eq!(rows_of(&editor, tasks).last(), Some(&extra));
}

#[test]
fn row_actions_need_the_database_loaded_and_a_valid_place() {
    let mut ws = Workspace::new();
    let leaf_model::Outcome::DatabasePageCreated { page, database } = ws
        .apply(WorkspaceCommand::CreateDatabasePage {
            parent: None,
            index: 0,
            title: "Saved".into(),
            created_at: 1,
        })
        .unwrap()
    else {
        panic!("expected a database page");
    };
    let mut editor = WorkspaceEditor::new(ws);
    assert!(matches!(
        editor.add_row(page, 0, 2),
        Err(EditorError::DatabaseNotLoaded(id)) if id == database
    ));
    editor.insert_database(database, Database::new());
    assert!(matches!(
        editor.add_row(page, 1, 2),
        Err(EditorError::Model(leaf_model::ModelError::IndexOutOfRange { .. }))
    ));
    assert_eq!(
        editor.workspace().row_pages(database).unwrap(),
        Vec::new(),
        "nothing half-made"
    );
    let row = editor.add_row(page, 0, 2).unwrap();
    // A copy loaded later doesn't replace the live one.
    editor.insert_database(database, Database::new());
    assert_eq!(editor.database(database).unwrap().rows(), [row]);
}
