//! Inline databases in the workspace editor (E09-S09): made under the page they are on, and trashed and brought
//! back with their block.

use leaf_core::PageId;
use leaf_model::{Caret, Workspace};

use crate::{Key, WorkspaceEditor};

/// An editor with a page "Notes" open, `/database` chosen on its first line, and the database made.
fn editor() -> (WorkspaceEditor, PageId, PageId) {
    let mut ws = Workspace::new();
    ws.set_undo_merge_interval(0);
    let mut editor = WorkspaceEditor::new(ws);
    let notes = editor.new_page_in(None, 1).unwrap();
    editor.select(notes).unwrap();
    let page = editor.open_page().unwrap().unwrap();
    page.doc.set_undo_merge_interval(0);
    let first = page.ensure_block().unwrap();
    page.set_caret(Caret { block: first, at: 0 }).unwrap();
    assert!(
        !editor.sync_inline_databases(2).unwrap(),
        "the first look only takes note"
    );
    for ch in "/database".chars() {
        editor
            .open_page()
            .unwrap()
            .unwrap()
            .insert_text(&ch.to_string())
            .unwrap();
    }
    editor.open_page().unwrap().unwrap().press(Key::Enter).unwrap();
    let db = editor.make_requested_database(3).unwrap().unwrap();
    assert_eq!(editor.make_requested_database(4).unwrap(), None, "made once");
    (editor, notes, db)
}

fn trashed(editor: &WorkspaceEditor, page: PageId) -> bool {
    editor.workspace().is_in_trash(page).unwrap()
}

#[test]
fn the_database_is_made_under_its_page_and_listed_there() {
    let (mut editor, notes, db) = editor();
    assert_eq!(editor.workspace().parent(db).unwrap(), Some(notes));
    assert!(editor.workspace().page(db).unwrap().database.is_some());
    assert!(
        editor.database(editor.database_of(db).unwrap()).is_some(),
        "loaded and ready"
    );
    assert_eq!(
        editor.open_page().unwrap().unwrap().inline_databases().unwrap(),
        vec![db]
    );
    assert!(!editor.sync_inline_databases(5).unwrap(), "nothing moved");
    let sidebar = editor.sidebar().unwrap();
    assert!(
        sidebar.iter().any(|row| row.page == db && row.depth == 1),
        "the sidebar lists it under its page"
    );
    // Making it isn't a workspace undo step (the new page "Notes" is): the page's undo takes the block away.
    assert!(editor.workspace.undo().unwrap());
    assert!(!editor.workspace.contains(notes));
    assert!(!editor.workspace.undo().unwrap(), "nothing more to undo");
}

#[test]
fn deleting_the_block_trashes_the_database_and_its_rows_and_undo_brings_them_back() {
    let (mut editor, _, db) = editor();
    let row = editor.add_row(db, 0, 5).unwrap();
    let page = editor.open_page().unwrap().unwrap();
    let block = page.rows().unwrap()[0].id;
    page.delete_blocks(&[block]).unwrap();
    assert!(editor.sync_inline_databases(6).unwrap());
    assert!(trashed(&editor, db));
    assert!(trashed(&editor, row), "its rows go with it");
    assert_eq!(editor.trash_items().unwrap()[0].page, db);

    assert!(editor.open_page().unwrap().unwrap().undo().unwrap());
    assert!(editor.sync_inline_databases(7).unwrap());
    assert!(!trashed(&editor, db));
    assert!(!trashed(&editor, row));
    let id = editor.database_of(db).unwrap();
    assert_eq!(editor.database(id).unwrap().rows(), vec![row], "rows and all");

    assert!(editor.open_page().unwrap().unwrap().redo().unwrap());
    assert!(editor.sync_inline_databases(8).unwrap());
    assert!(trashed(&editor, db));
}

#[test]
fn a_database_deleted_for_good_stays_gone_when_undo_brings_its_block_back() {
    let (mut editor, _, db) = editor();
    let page = editor.open_page().unwrap().unwrap();
    let block = page.rows().unwrap()[0].id;
    page.delete_blocks(&[block]).unwrap();
    editor.sync_inline_databases(6).unwrap();
    editor.delete_forever(db).unwrap();
    editor
        .select(editor.workspace().root_pages().unwrap()[0])
        .unwrap();
    assert!(editor.open_page().unwrap().unwrap().undo().unwrap());
    assert!(!editor.sync_inline_databases(7).unwrap());
    assert!(!editor.workspace().contains(db));
}

#[test]
fn with_no_page_open_or_drawn_there_is_nothing_to_do() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    assert_eq!(editor.make_requested_database(1).unwrap(), None);
    assert!(!editor.sync_inline_databases(1).unwrap());
    let page = editor.new_page_in(None, 1).unwrap();
    editor.select(page).unwrap();
    assert_eq!(
        editor.make_requested_database(2).unwrap(),
        None,
        "the page isn't loaded"
    );
    assert!(!editor.sync_inline_databases(2).unwrap());
    editor.open_page().unwrap();
    assert_eq!(
        editor.make_requested_database(3).unwrap(),
        None,
        "nothing asked for"
    );
}
