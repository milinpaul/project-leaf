//! Tests for reshaping a table from its controls (E16-S03).

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, PageCommand, PageDoc, PageOutcome};

use super::moved;
use crate::{PageEditor, TableEdit};

/// A page holding one `rows` × `columns` table whose cells read "r0c0", "r0c1", ...
fn page(rows: usize, columns: usize) -> (PageEditor, BlockId) {
    let mut doc = PageDoc::new();
    let Ok(PageOutcome::BlockCreated(table)) = doc.apply(PageCommand::InsertTable {
        parent: None,
        index: 0,
        rows,
        columns,
        header_row: false,
    }) else {
        panic!("expected a table");
    };
    for (r, row) in doc.table_cells(table).unwrap().into_iter().enumerate() {
        for (c, cell) in row.into_iter().enumerate() {
            doc.apply(PageCommand::InsertText {
                id: cell,
                pos: 0,
                text: format!("r{r}c{c}"),
            })
            .unwrap();
        }
    }
    let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    doc.set_undo_merge_interval(0);
    (PageEditor::new(doc), table)
}

fn grid(editor: &PageEditor, table: BlockId) -> Vec<Vec<String>> {
    editor
        .doc()
        .table_cells(table)
        .unwrap()
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|c| editor.doc().plain_text(c).unwrap().unwrap())
                .collect()
        })
        .collect()
}

fn cell(editor: &PageEditor, table: BlockId, row: usize, column: usize) -> BlockId {
    editor.doc().table_cells(table).unwrap()[row][column]
}

fn caret_at(editor: &mut PageEditor, table: BlockId, row: usize, column: usize, at: usize) {
    let block = cell(editor, table, row, column);
    editor.set_caret(Caret { block, at }).unwrap();
}

#[test]
fn inserting_puts_the_caret_in_the_new_cell_and_undoes_in_one_step() {
    let (mut editor, table) = page(2, 2);
    caret_at(&mut editor, table, 1, 1, 2);
    let caret = editor
        .edit_table(table, TableEdit::InsertRow(2))
        .unwrap()
        .unwrap();
    assert_eq!(grid(&editor, table)[2], ["", ""]);
    assert_eq!(
        caret,
        Caret {
            block: cell(&editor, table, 2, 1),
            at: 0
        },
        "same column, new row"
    );
    let caret = editor
        .edit_table(table, TableEdit::InsertColumn(0))
        .unwrap()
        .unwrap();
    assert_eq!(grid(&editor, table)[0], ["", "r0c0", "r0c1"]);
    assert_eq!(
        caret,
        Caret {
            block: cell(&editor, table, 2, 0),
            at: 0
        },
        "same row, new column"
    );

    assert!(editor.undo().unwrap());
    assert_eq!(grid(&editor, table)[0], ["r0c0", "r0c1"], "one undo per edit");
    assert!(editor.undo().unwrap());
    assert_eq!(grid(&editor, table).len(), 2);
}

#[test]
fn with_no_caret_in_the_table_an_insert_goes_to_the_first_row_or_column() {
    let (mut editor, table) = page(2, 2);
    let caret = editor
        .edit_table(table, TableEdit::InsertColumn(2))
        .unwrap()
        .unwrap();
    assert_eq!(caret.block, cell(&editor, table, 0, 2));
    // Now the caret is in the table: a new row keeps its column.
    let caret = editor
        .edit_table(table, TableEdit::InsertRow(0))
        .unwrap()
        .unwrap();
    assert_eq!(caret.block, cell(&editor, table, 0, 2));
    let (mut editor, table) = page(2, 2);
    let caret = editor
        .edit_table(table, TableEdit::InsertRow(1))
        .unwrap()
        .unwrap();
    assert_eq!(caret.block, cell(&editor, table, 1, 0));
}

#[test]
fn moving_takes_the_caret_along_with_its_cell() {
    let (mut editor, table) = page(3, 3);
    caret_at(&mut editor, table, 0, 1, 3);
    let caret = editor
        .edit_table(table, TableEdit::MoveRow { from: 0, to: 2 })
        .unwrap()
        .unwrap();
    assert_eq!(grid(&editor, table)[2], ["r0c0", "r0c1", "r0c2"]);
    assert_eq!(
        caret,
        Caret {
            block: cell(&editor, table, 2, 1),
            at: 3
        }
    );
    let caret = editor
        .edit_table(table, TableEdit::MoveColumn { from: 1, to: 0 })
        .unwrap()
        .unwrap();
    assert_eq!(grid(&editor, table)[2], ["r0c1", "r0c0", "r0c2"]);
    assert_eq!(
        caret,
        Caret {
            block: cell(&editor, table, 2, 0),
            at: 3
        }
    );
    // A row moved past the caret's row shifts it.
    let caret = editor
        .edit_table(table, TableEdit::MoveRow { from: 0, to: 2 })
        .unwrap()
        .unwrap();
    assert_eq!(caret.block, cell(&editor, table, 1, 0));
    assert_eq!(editor.doc().plain_text(caret.block).unwrap().unwrap(), "r0c1");
}

#[test]
fn deleting_the_caret_s_row_or_column_keeps_it_at_the_same_place_in_the_grid() {
    let (mut editor, table) = page(3, 3);
    caret_at(&mut editor, table, 2, 2, 4);
    let caret = editor
        .edit_table(table, TableEdit::DeleteRow(2))
        .unwrap()
        .unwrap();
    assert_eq!(
        caret,
        Caret {
            block: cell(&editor, table, 1, 2),
            at: 4
        },
        "clamped to the last row"
    );
    let caret = editor
        .edit_table(table, TableEdit::DeleteColumn(2))
        .unwrap()
        .unwrap();
    assert_eq!(
        caret,
        Caret {
            block: cell(&editor, table, 1, 1),
            at: 4
        }
    );
    assert_eq!(grid(&editor, table), [["r0c0", "r0c1"], ["r1c0", "r1c1"]]);
}

#[test]
fn the_header_row_toggles() {
    let (mut editor, table) = page(1, 1);
    editor.edit_table(table, TableEdit::ToggleHeaderRow).unwrap();
    assert_eq!(
        editor.doc().kind(table).unwrap(),
        BlockKind::Table { header_row: true }
    );
    editor.edit_table(table, TableEdit::ToggleHeaderRow).unwrap();
    assert_eq!(
        editor.doc().kind(table).unwrap(),
        BlockKind::Table { header_row: false }
    );
}

#[test]
fn edits_the_table_can_t_take_are_refused_and_change_nothing() {
    let (mut editor, table) = page(1, 1);
    assert!(editor.edit_table(table, TableEdit::DeleteRow(0)).is_err());
    assert!(editor.edit_table(table, TableEdit::DeleteColumn(0)).is_err());
    assert!(editor.edit_table(table, TableEdit::InsertRow(5)).is_err());
    assert_eq!(grid(&editor, table), [["r0c0"]]);
    let not_a_table = cell(&editor, table, 0, 0);
    assert!(editor.edit_table(not_a_table, TableEdit::InsertRow(0)).is_err());
}

#[test]
fn a_moved_position_follows_the_move() {
    // Items 0..5, item 1 moves to 3: 0 2 3 1 4.
    assert_eq!([0, 1, 2, 3, 4].map(|at| moved(at, 1, 3)), [0, 3, 1, 2, 4]);
    // Item 3 moves to 1: 0 3 1 2 4.
    assert_eq!([0, 1, 2, 3, 4].map(|at| moved(at, 3, 1)), [0, 2, 3, 1, 4]);
}
