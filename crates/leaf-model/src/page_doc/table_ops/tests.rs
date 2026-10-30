//! Tests for tables (E16-S01): building, reshaping, the guards on rows and cells, merges and data
//! from newer versions.

use leaf_core::BlockId;
use loro::LoroValue;

use crate::block_kind::keys;
use crate::page_meta::value;
use crate::{BlockKind, BlockProp, History, Mark, ModelError, PageCommand, PageDoc, PageOutcome};

fn paragraph(page: &mut PageDoc, text: &str) -> BlockId {
    let index = page.root_blocks().len();
    match page.apply(PageCommand::InsertBlock {
        parent: None,
        index,
        kind: BlockKind::Paragraph,
        text: text.into(),
    }) {
        Ok(PageOutcome::BlockCreated(id)) => id,
        other => panic!("expected a new block, got {other:?}"),
    }
}

fn table(page: &mut PageDoc, rows: usize, columns: usize) -> BlockId {
    let index = page.root_blocks().len();
    match page.apply(PageCommand::InsertTable {
        parent: None,
        index,
        rows,
        columns,
        header_row: false,
    }) {
        Ok(PageOutcome::BlockCreated(id)) => id,
        other => panic!("expected a new table, got {other:?}"),
    }
}

/// Types `text` into every cell: "r0c0", "r0c1", ... so moves can be followed.
fn label_cells(page: &mut PageDoc, table: BlockId) {
    for (r, row) in page.table_cells(table).unwrap().into_iter().enumerate() {
        for (c, cell) in row.into_iter().enumerate() {
            page.apply(PageCommand::InsertText {
                id: cell,
                pos: 0,
                text: format!("r{r}c{c}"),
            })
            .unwrap();
        }
    }
}

/// The grid's text, row by row.
fn grid(page: &PageDoc, table: BlockId) -> Vec<Vec<String>> {
    page.table_cells(table)
        .unwrap()
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|cell| page.plain_text(cell).unwrap().unwrap())
                .collect()
        })
        .collect()
}

fn rows_of(page: &PageDoc, table: BlockId) -> Vec<BlockId> {
    page.children(table).unwrap()
}

#[test]
fn a_table_is_a_grid_of_empty_cells_made_in_one_step() {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    let t = table(&mut page, 2, 3);

    assert_eq!(page.kind(t).unwrap(), BlockKind::Table { header_row: false });
    assert_eq!(
        page.plain_text(t).unwrap(),
        None,
        "a table has no text of its own"
    );
    let cells = page.table_cells(t).unwrap();
    assert_eq!(cells.iter().map(Vec::len).collect::<Vec<_>>(), [3, 3]);
    for row in rows_of(&page, t) {
        assert_eq!(page.kind(row).unwrap(), BlockKind::TableRow);
        assert_eq!(page.plain_text(row).unwrap(), None);
    }
    for cell in cells.concat() {
        assert_eq!(page.kind(cell).unwrap(), BlockKind::TableCell);
        assert_eq!(page.plain_text(cell).unwrap().as_deref(), Some(""));
    }
    assert_eq!(page.len(), 1 + 2 + 6);
    page.check_invariants().unwrap();

    assert!(matches!(page.undo().unwrap(), History::Done { .. }));
    assert!(page.is_empty(), "one undo removes the whole table");
}

#[test]
fn a_table_needs_a_size_and_a_place() {
    let mut page = PageDoc::new();
    for (rows, columns) in [(0, 2), (2, 0)] {
        let made = page.apply(PageCommand::InsertTable {
            parent: None,
            index: 0,
            rows,
            columns,
            header_row: false,
        });
        assert!(matches!(made, Err(ModelError::EmptyTable)), "{rows}×{columns}");
    }
    let made = page.apply(PageCommand::InsertTable {
        parent: None,
        index: 5,
        rows: 1,
        columns: 1,
        header_row: true,
    });
    assert!(matches!(
        made,
        Err(ModelError::IndexOutOfRange { index: 5, len: 0 })
    ));
    let heading = paragraph(&mut page, "Title");
    page.apply(PageCommand::SetBlockKind {
        id: heading,
        kind: BlockKind::Heading { level: 1 },
    })
    .unwrap();
    let made = page.apply(PageCommand::InsertTable {
        parent: Some(heading),
        index: 0,
        rows: 1,
        columns: 1,
        header_row: true,
    });
    assert!(matches!(made, Err(ModelError::ChildrenNotAllowed { .. })));
    assert_eq!(page.len(), 1, "nothing written");

    let quote = paragraph(&mut page, "Quote");
    let PageOutcome::BlockCreated(nested) = page
        .apply(PageCommand::InsertTable {
            parent: Some(quote),
            index: 0,
            rows: 1,
            columns: 1,
            header_row: true,
        })
        .unwrap()
    else {
        panic!("expected a table");
    };
    assert_eq!(
        page.children(quote).unwrap(),
        [nested],
        "tables nest like any block"
    );
    assert_eq!(page.kind(nested).unwrap(), BlockKind::Table { header_row: true });
}

#[test]
fn cells_take_text_and_marks_like_any_text_block() {
    let mut page = PageDoc::new();
    let t = table(&mut page, 1, 2);
    let cell = page.table_cells(t).unwrap()[0][1];
    page.apply(PageCommand::InsertText {
        id: cell,
        pos: 0,
        text: "Price".into(),
    })
    .unwrap();
    page.apply(PageCommand::SetMark {
        id: cell,
        range: 0..5,
        mark: Mark::Bold,
    })
    .unwrap();
    let spans = page.text(cell).unwrap().unwrap();
    assert_eq!(spans.len(), 1);
    assert!(spans[0].marks.bold);

    for id in [t, rows_of(&page, t)[0]] {
        let typed = page.apply(PageCommand::InsertText {
            id,
            pos: 0,
            text: "x".into(),
        });
        assert!(
            matches!(typed, Err(ModelError::TextNotAllowed { .. })),
            "tables and rows have no text"
        );
    }
}

#[test]
fn rows_are_added_removed_and_moved_by_position() {
    let mut page = PageDoc::new();
    let t = table(&mut page, 2, 2);
    label_cells(&mut page, t);

    let PageOutcome::BlockCreated(row) = page
        .apply(PageCommand::InsertTableRow { table: t, index: 1 })
        .unwrap()
    else {
        panic!("expected a row");
    };
    assert_eq!(rows_of(&page, t)[1], row);
    assert_eq!(grid(&page, t), [["r0c0", "r0c1"], ["", ""], ["r1c0", "r1c1"]]);
    page.apply(PageCommand::InsertTableRow { table: t, index: 3 })
        .unwrap();
    assert_eq!(grid(&page, t).len(), 4, "the row count appends");
    let past = page.apply(PageCommand::InsertTableRow { table: t, index: 9 });
    assert!(matches!(
        past,
        Err(ModelError::IndexOutOfRange { index: 9, len: 4 })
    ));

    page.apply(PageCommand::MoveTableRow {
        table: t,
        from: 2,
        to: 0,
    })
    .unwrap();
    assert_eq!(
        grid(&page, t),
        [["r1c0", "r1c1"], ["r0c0", "r0c1"], ["", ""], ["", ""]]
    );
    page.apply(PageCommand::MoveTableRow {
        table: t,
        from: 0,
        to: 3,
    })
    .unwrap();
    assert_eq!(grid(&page, t)[3], ["r1c0", "r1c1"]);
    let same = page
        .apply(PageCommand::MoveTableRow {
            table: t,
            from: 1,
            to: 1,
        })
        .unwrap();
    assert_eq!(same, PageOutcome::Unchanged);

    let PageOutcome::BlocksDeleted(gone) = page
        .apply(PageCommand::DeleteTableRow { table: t, index: 0 })
        .unwrap()
    else {
        panic!("expected deleted blocks");
    };
    assert_eq!(gone.len(), 3, "the row and its two cells");
    for _ in 0..2 {
        page.apply(PageCommand::DeleteTableRow { table: t, index: 0 })
            .unwrap();
    }
    assert_eq!(grid(&page, t), [["r1c0", "r1c1"]]);
    let last = page.apply(PageCommand::DeleteTableRow { table: t, index: 0 });
    assert!(matches!(last, Err(ModelError::EmptyTable)), "the last row stays");
    page.check_invariants().unwrap();
}

#[test]
fn columns_are_added_removed_and_moved_in_every_row() {
    let mut page = PageDoc::new();
    let t = table(&mut page, 2, 2);
    label_cells(&mut page, t);

    let PageOutcome::BlocksCreated(cells) = page
        .apply(PageCommand::InsertTableColumn { table: t, index: 0 })
        .unwrap()
    else {
        panic!("expected new cells");
    };
    assert_eq!(
        cells,
        [
            page.table_cells(t).unwrap()[0][0],
            page.table_cells(t).unwrap()[1][0]
        ]
    );
    assert_eq!(grid(&page, t), [["", "r0c0", "r0c1"], ["", "r1c0", "r1c1"]]);
    let past = page.apply(PageCommand::InsertTableColumn { table: t, index: 4 });
    assert!(matches!(
        past,
        Err(ModelError::IndexOutOfRange { index: 4, len: 3 })
    ));

    page.apply(PageCommand::MoveTableColumn {
        table: t,
        from: 0,
        to: 2,
    })
    .unwrap();
    assert_eq!(grid(&page, t), [["r0c0", "r0c1", ""], ["r1c0", "r1c1", ""]]);
    page.apply(PageCommand::MoveTableColumn {
        table: t,
        from: 1,
        to: 0,
    })
    .unwrap();
    assert_eq!(grid(&page, t), [["r0c1", "r0c0", ""], ["r1c1", "r1c0", ""]]);

    page.apply(PageCommand::DeleteTableColumn { table: t, index: 2 })
        .unwrap();
    page.apply(PageCommand::DeleteTableColumn { table: t, index: 0 })
        .unwrap();
    assert_eq!(grid(&page, t), [["r0c0"], ["r1c0"]]);
    let last = page.apply(PageCommand::DeleteTableColumn { table: t, index: 0 });
    assert!(
        matches!(last, Err(ModelError::EmptyTable)),
        "the last column stays"
    );
    let past = page.apply(PageCommand::MoveTableColumn {
        table: t,
        from: 0,
        to: 1,
    });
    assert!(matches!(
        past,
        Err(ModelError::IndexOutOfRange { index: 1, len: 1 })
    ));
    page.check_invariants().unwrap();
}

#[test]
fn the_header_row_is_a_table_property() {
    let mut page = PageDoc::new();
    let t = table(&mut page, 2, 2);
    let header = |on| PageCommand::SetProp {
        id: t,
        prop: BlockProp::TableHeaderRow(on),
    };
    assert_eq!(page.apply(header(true)).unwrap(), PageOutcome::Changed);
    assert_eq!(page.kind(t).unwrap(), BlockKind::Table { header_row: true });
    assert_eq!(page.apply(header(true)).unwrap(), PageOutcome::Unchanged);
    assert_eq!(page.apply(header(false)).unwrap(), PageOutcome::Changed);

    let p = paragraph(&mut page, "text");
    let wrong = page.apply(PageCommand::SetProp {
        id: p,
        prop: BlockProp::TableHeaderRow(true),
    });
    assert!(matches!(wrong, Err(ModelError::PropNotForKind { .. })));
    let cell = page.table_cells(t).unwrap()[0][0];
    let wrong = page.apply(PageCommand::SetProp {
        id: cell,
        prop: BlockProp::Checked(true),
    });
    assert!(matches!(wrong, Err(ModelError::PropNotForKind { .. })));
}

#[test]
fn table_commands_need_a_table() {
    let mut page = PageDoc::new();
    let p = paragraph(&mut page, "not a table");
    for command in [
        PageCommand::InsertTableRow { table: p, index: 0 },
        PageCommand::DeleteTableRow { table: p, index: 0 },
        PageCommand::MoveTableRow {
            table: p,
            from: 0,
            to: 0,
        },
        PageCommand::InsertTableColumn { table: p, index: 0 },
        PageCommand::DeleteTableColumn { table: p, index: 0 },
        PageCommand::MoveTableColumn {
            table: p,
            from: 0,
            to: 0,
        },
    ] {
        assert!(
            matches!(page.apply(command.clone()), Err(ModelError::NotATable(id)) if id == p),
            "{command:?}"
        );
    }
    assert!(matches!(page.table_cells(p), Err(ModelError::NotATable(_))));
}

#[test]
fn rows_and_cells_only_change_through_the_table_commands() {
    let mut page = PageDoc::new();
    let above = paragraph(&mut page, "above");
    let t = table(&mut page, 2, 2);
    let row = rows_of(&page, t)[1];
    let cell = page.table_cells(t).unwrap()[0][0];
    for part in [row, cell] {
        for command in [
            PageCommand::DeleteBlocks { ids: vec![part] },
            PageCommand::MoveBlocks {
                ids: vec![part],
                new_parent: None,
                index: 0,
            },
            PageCommand::DuplicateBlocks { ids: vec![part] },
            PageCommand::Indent { ids: vec![part] },
            PageCommand::Outdent { ids: vec![part] },
            PageCommand::SetBlockKind {
                id: part,
                kind: BlockKind::Paragraph,
            },
        ] {
            let refused = page.apply(command.clone());
            assert!(
                matches!(refused, Err(ModelError::TablePart { block }) if block == part),
                "{command:?} gave {refused:?}"
            );
        }
    }
    for command in [
        PageCommand::SplitBlock { id: cell, pos: 0 },
        PageCommand::MergeIntoPrevious { id: cell },
    ] {
        assert!(matches!(page.apply(command), Err(ModelError::TablePart { .. })));
    }
    // A row or cell listed with its table goes with the table.
    let deleted = page
        .apply(PageCommand::DeleteBlocks { ids: vec![cell, t] })
        .unwrap();
    assert!(matches!(deleted, PageOutcome::BlocksDeleted(ids) if ids.len() == 7));
    assert_eq!(page.root_blocks(), [above]);
    page.check_invariants().unwrap();
}

#[test]
fn nothing_else_goes_into_a_table_and_a_table_stays_a_table() {
    let mut page = PageDoc::new();
    let t = table(&mut page, 1, 1);
    let row = rows_of(&page, t)[0];
    let p = paragraph(&mut page, "below");
    for parent in [t, row] {
        let inserted = page.apply(PageCommand::InsertBlock {
            parent: Some(parent),
            index: 0,
            kind: BlockKind::Paragraph,
            text: String::new(),
        });
        assert!(matches!(inserted, Err(ModelError::ChildrenNotAllowed { .. })));
        let moved = page.apply(PageCommand::MoveBlocks {
            ids: vec![p],
            new_parent: Some(parent),
            index: 0,
        });
        assert!(matches!(moved, Err(ModelError::ChildrenNotAllowed { .. })));
    }
    assert_eq!(
        page.apply(PageCommand::Indent { ids: vec![p] }).unwrap(),
        PageOutcome::Unchanged,
        "the block below a table doesn't indent into it"
    );
    for kind in [
        BlockKind::Table { header_row: false },
        BlockKind::TableRow,
        BlockKind::TableCell,
    ] {
        let made = page.apply(PageCommand::InsertBlock {
            parent: None,
            index: 0,
            kind: kind.clone(),
            text: String::new(),
        });
        assert!(matches!(made, Err(ModelError::InvalidKind(_))), "{kind:?}");
        let turned = page.apply(PageCommand::SetBlockKind { id: p, kind });
        assert!(matches!(turned, Err(ModelError::InvalidKind(_))));
    }
    let turned = page.apply(PageCommand::SetBlockKind {
        id: t,
        kind: BlockKind::Paragraph,
    });
    assert!(matches!(turned, Err(ModelError::InvalidKind(_))));
}

#[test]
fn backspace_below_a_table_selects_the_table() {
    let mut page = PageDoc::new();
    let t = table(&mut page, 2, 2);
    let below = paragraph(&mut page, "below");
    let merged = page.apply(PageCommand::MergeIntoPrevious { id: below });
    assert!(
        matches!(merged, Err(ModelError::MergeTargetNotText { target }) if target == t),
        "{merged:?}"
    );
}

#[test]
fn whole_tables_move_and_duplicate_like_any_block() {
    let mut page = PageDoc::new();
    let first = paragraph(&mut page, "first");
    let t = table(&mut page, 2, 2);
    label_cells(&mut page, t);
    page.apply(PageCommand::MoveBlocks {
        ids: vec![t],
        new_parent: None,
        index: 0,
    })
    .unwrap();
    assert_eq!(page.root_blocks(), [t, first]);

    let PageOutcome::BlocksCreated(copies) =
        page.apply(PageCommand::DuplicateBlocks { ids: vec![t] }).unwrap()
    else {
        panic!("expected a copy");
    };
    let copy = copies[0];
    assert_eq!(grid(&page, copy), grid(&page, t));
    let cells: Vec<BlockId> = page.table_cells(t).unwrap().concat();
    assert!(
        page.table_cells(copy)
            .unwrap()
            .concat()
            .iter()
            .all(|c| !cells.contains(c)),
        "the copy's cells have new IDs"
    );
    page.check_invariants().unwrap();
}

#[test]
fn a_merge_that_leaves_rows_uneven_is_evened_out_by_the_next_table_command() {
    let mut page = PageDoc::new();
    let t = table(&mut page, 2, 2);
    label_cells(&mut page, t);
    // As if another copy had added a row while this one removed a column: a short row.
    let short = page.table_cells(t).unwrap()[1][1];
    let node = page.node(short).unwrap();
    page.tree.delete(node).unwrap();
    page.index.remove(short);
    page.doc.commit();
    page.check_invariants().unwrap();
    assert_eq!(grid(&page, t), [vec!["r0c0", "r0c1"], vec!["r1c0"]]);

    page.apply(PageCommand::InsertTableRow { table: t, index: 2 })
        .unwrap();
    assert_eq!(grid(&page, t), [["r0c0", "r0c1"], ["r1c0", ""], ["", ""]]);
    page.check_invariants().unwrap();
}

#[test]
fn a_merge_that_puts_blocks_in_the_wrong_place_is_repaired_without_losing_them() {
    let mut page = PageDoc::new();
    let t = table(&mut page, 1, 1);
    let row = rows_of(&page, t)[0];
    let cell = page.table_cells(t).unwrap()[0][0];
    let p = paragraph(&mut page, "stray");
    // A paragraph inside a table, and a cell outside any row: what no command makes.
    page.tree
        .mov_to(page.node(p).unwrap(), Some(page.node(t).unwrap()), 1)
        .unwrap();
    page.tree.mov_to(page.node(cell).unwrap(), None, 0).unwrap();
    page.doc.commit();
    assert!(page.check_invariants().is_err());

    page.repair().unwrap();
    page.check_invariants().unwrap();
    assert!(page.contains(p) && page.contains(cell), "nothing lost");
    assert_eq!(page.parent(p).unwrap(), None, "the paragraph left the table");
    assert_eq!(
        page.kind(cell).unwrap(),
        BlockKind::Paragraph,
        "the stray cell is a paragraph now"
    );
    assert_eq!(page.children(t).unwrap(), [row]);
}

#[test]
fn data_from_a_newer_version_survives_edits_and_a_reload() {
    let mut page = PageDoc::new();
    let t = table(&mut page, 2, 2);
    let row = rows_of(&page, t)[0];
    let cell = page.table_cells(t).unwrap()[0][0];
    // A newer Leaf stores column widths on the table, a colour on a row and a cell, and a mark.
    let meta = |page: &PageDoc, id| page.tree.get_meta(page.node(id).unwrap()).unwrap();
    meta(&page, t).insert("column_widths", "[120, 80]").unwrap();
    meta(&page, row).insert("background", "yellow").unwrap();
    meta(&page, cell).insert("align", "right").unwrap();
    page.text_container(cell)
        .unwrap()
        .unwrap()
        .insert(0, "note")
        .unwrap();
    page.text_container(cell)
        .unwrap()
        .unwrap()
        .mark(0..4, "highlight", "green")
        .unwrap();
    page.doc.commit();

    page.apply(PageCommand::InsertTableColumn { table: t, index: 0 })
        .unwrap();
    page.apply(PageCommand::MoveTableRow {
        table: t,
        from: 0,
        to: 1,
    })
    .unwrap();
    page.apply(PageCommand::SetProp {
        id: t,
        prop: BlockProp::TableHeaderRow(true),
    })
    .unwrap();
    page.apply(PageCommand::InsertText {
        id: cell,
        pos: 4,
        text: "s".into(),
    })
    .unwrap();

    let loaded = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    let string = |page: &PageDoc, id, key| match value(&meta(page, id), key) {
        Some(LoroValue::String(s)) => s.to_string(),
        other => panic!("{key}: {other:?}"),
    };
    assert_eq!(string(&loaded, t, "column_widths"), "[120, 80]");
    assert_eq!(string(&loaded, row, "background"), "yellow");
    assert_eq!(string(&loaded, cell, "align"), "right");
    assert!(
        loaded
            .text_container(cell)
            .unwrap()
            .unwrap()
            .to_delta()
            .iter()
            .any(|d| matches!(
                d,
                loro::TextDelta::Insert { attributes: Some(a), .. } if a.contains_key("highlight")
            )),
        "the unknown mark is kept"
    );
    assert_eq!(loaded.kind(t).unwrap(), BlockKind::Table { header_row: true });
    assert_eq!(loaded.plain_text(cell).unwrap().as_deref(), Some("notes"));
    assert_eq!(
        value(&meta(&loaded, t), keys::HEADER_ROW),
        Some(LoroValue::Bool(true))
    );
    loaded.check_invariants().unwrap();
}
