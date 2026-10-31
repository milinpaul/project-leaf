//! Tests for the page's side of inline databases (E09-S09): the slash menu's "Database", where the block goes,
//! the comings and goings reported to the workspace, and copying one as a link.

use leaf_core::{BlockId, PageId};
use leaf_model::{BlockKind, Caret, Marks, PAGE_LINK, PageCommand, PageDoc, PageOutcome, TextSpan};

use super::InlineDatabaseChanges;
use crate::{Fragment, FragmentBlock, Key, PageEditor};

fn page(texts: &[&str]) -> (PageEditor, Vec<BlockId>) {
    let mut doc = PageDoc::new();
    let ids = texts
        .iter()
        .map(|text| {
            let index = doc.root_blocks().len();
            let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
                parent: None,
                index,
                kind: BlockKind::Paragraph,
                text: (*text).into(),
            }) else {
                panic!("insert failed");
            };
            id
        })
        .collect();
    let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    doc.set_undo_merge_interval(0);
    (PageEditor::new(doc), ids)
}

fn outline(editor: &PageEditor) -> Vec<(BlockKind, String)> {
    editor
        .rows()
        .unwrap()
        .into_iter()
        .map(|row| (editor.doc().kind(row.id).unwrap(), row.text.unwrap_or_default()))
        .collect()
}

fn type_keys(editor: &mut PageEditor, keys: &str) {
    for ch in keys.chars() {
        editor.insert_text(&ch.to_string()).unwrap();
    }
}

fn database(page: PageId) -> BlockKind {
    BlockKind::Database { page }
}

#[test]
fn the_slash_menu_asks_for_a_database_where_it_was_typed() {
    let (mut editor, ids) = page(&["", "Elsewhere"]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    type_keys(&mut editor, "/database");
    assert_eq!(editor.slash_menu().unwrap().unwrap().items[0].label, "Database");
    assert_eq!(editor.take_database_request().unwrap(), None, "nothing asked yet");
    editor.press(Key::Enter).unwrap();
    assert_eq!(
        outline(&editor)[0],
        (BlockKind::Paragraph, String::new()),
        "`/database` is gone"
    );

    // The caret wanders before the workspace answers; the database still goes where it was asked.
    editor.set_caret(Caret { block: ids[1], at: 3 }).unwrap();
    let at = editor.take_database_request().unwrap().unwrap();
    assert_eq!(at, Caret { block: ids[0], at: 0 });
    assert_eq!(editor.take_database_request().unwrap(), None, "only once");

    let db = PageId::new();
    let caret = editor.insert_inline_database(at, db).unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            (database(db), String::new()),
            (BlockKind::Paragraph, String::new()),
            (BlockKind::Paragraph, "Elsewhere".into()),
        ],
        "the empty line gave way, and a new one waits below"
    );
    assert_eq!(caret, editor.caret().unwrap().unwrap());
    assert_eq!(caret.block, editor.rows().unwrap()[1].id);
    assert_eq!(editor.inline_databases().unwrap(), vec![db]);
}

#[test]
fn next_to_text_or_in_a_table_the_database_goes_below() {
    let (mut editor, ids) = page(&["Notes"]);
    let db = PageId::new();
    let at = Caret { block: ids[0], at: 5 };
    assert_eq!(
        editor.insert_inline_database(at, db).unwrap(),
        at,
        "the caret stays"
    );
    assert_eq!(
        outline(&editor),
        vec![
            (BlockKind::Paragraph, "Notes".into()),
            (database(db), String::new())
        ]
    );

    let (mut editor, ids) = page(&["", "After"]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    type_keys(&mut editor, "/table");
    editor.slash_apply(None).unwrap();
    let cell = editor.caret().unwrap().unwrap();
    let table = editor.rows().unwrap()[0].id;
    let db = PageId::new();
    editor.insert_inline_database(cell, db).unwrap();
    let rows = editor.rows().unwrap();
    let after_table = rows
        .iter()
        .position(|r| editor.doc().parent(r.id).unwrap().is_none() && r.id != table && r.text.is_none())
        .unwrap();
    assert_eq!(editor.doc().kind(rows[after_table].id).unwrap(), database(db));
    assert_eq!(
        editor.doc().parent(rows[after_table].id).unwrap(),
        None,
        "below the table, not in it"
    );
}

#[test]
fn deleting_and_undoing_the_block_is_reported_once_each() {
    let (mut editor, ids) = page(&["Notes"]);
    assert_eq!(
        editor.inline_database_changes().unwrap(),
        InlineDatabaseChanges::default()
    );
    let db = PageId::new();
    editor
        .insert_inline_database(Caret { block: ids[0], at: 5 }, db)
        .unwrap();
    assert_eq!(
        editor.inline_database_changes().unwrap(),
        InlineDatabaseChanges::default(),
        "a new database isn't one coming back"
    );

    let block = editor.rows().unwrap()[1].id;
    editor.delete_blocks(&[block]).unwrap();
    assert_eq!(
        editor.inline_database_changes().unwrap(),
        InlineDatabaseChanges {
            gone: vec![db],
            back: vec![]
        }
    );
    assert_eq!(
        editor.inline_database_changes().unwrap(),
        InlineDatabaseChanges::default()
    );

    assert!(editor.undo().unwrap());
    assert_eq!(
        editor.inline_database_changes().unwrap(),
        InlineDatabaseChanges {
            gone: vec![],
            back: vec![db]
        }
    );
    assert!(editor.redo().unwrap());
    assert_eq!(editor.inline_database_changes().unwrap().gone, vec![db]);
}

#[test]
fn the_first_look_only_takes_note() {
    let (mut editor, ids) = page(&["Notes"]);
    let db = PageId::new();
    editor
        .insert_inline_database(Caret { block: ids[0], at: 5 }, db)
        .unwrap();
    let reopened = PageDoc::from_bytes(&editor.doc().to_bytes().unwrap()).unwrap();
    let mut editor = PageEditor::new(reopened);
    assert_eq!(
        editor.inline_database_changes().unwrap(),
        InlineDatabaseChanges::default()
    );
    assert_eq!(editor.inline_databases().unwrap(), vec![db]);
}

#[test]
fn undo_takes_the_database_block_away_and_gives_back_the_typed_query() {
    let (mut editor, ids) = page(&[""]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    type_keys(&mut editor, "/database");
    editor.slash_apply(None).unwrap();
    let at = editor.take_database_request().unwrap().unwrap();
    editor.insert_inline_database(at, PageId::new()).unwrap();
    assert!(editor.undo().unwrap(), "the database goes");
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, String::new())]);
    assert!(editor.undo().unwrap(), "then the query comes back");
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, "/database".into())]);
}

#[test]
fn copying_the_block_copies_a_link_to_the_same_database() {
    let (mut editor, ids) = page(&["Notes", ""]);
    let db = PageId::new();
    editor
        .insert_inline_database(Caret { block: ids[0], at: 5 }, db)
        .unwrap();
    let block = editor.rows().unwrap()[1].id;
    editor.select_blocks(&[block]).unwrap();
    let copied = editor.copy().unwrap().unwrap();
    let link = FragmentBlock::link_to(db);
    assert_eq!(copied, Fragment::Blocks(vec![link.clone()]));
    assert_eq!(
        link.text,
        vec![TextSpan {
            text: PAGE_LINK.to_string(),
            marks: Marks {
                page: Some(db),
                ..Marks::default()
            },
        }]
    );
    // Through the system clipboard too, and a hand-built database block travels the same way.
    let built = Fragment::Blocks(vec![FragmentBlock {
        kind: database(db),
        text: Vec::new(),
        children: Vec::new(),
    }]);
    assert_eq!(Fragment::from_json(&built.to_json()), Some(copied.clone()));

    // Pasted on the empty line: a link, never a second table.
    editor.set_caret(Caret { block: ids[1], at: 0 }).unwrap();
    editor.paste(&copied).unwrap();
    assert_eq!(editor.inline_databases().unwrap(), vec![db]);
    assert_eq!(editor.doc().page_links().len(), 1);
}
