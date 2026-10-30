//! Getting tables in and out (E16-S04): `/table`, pasting Markdown tables, copying tables as Markdown
//! and as Leaf's own fragment.

use leaf_core::BlockId;
use leaf_model::{BlockKind, PageCommand, PageDoc, PageOutcome};

use super::{Page, at};
use crate::{Fragment, PageEditor};

/// A page of paragraphs with these texts, loaded from bytes so building it can't be undone.
fn paragraphs(texts: &[&str]) -> (PageEditor, Vec<BlockId>) {
    let mut doc = PageDoc::new();
    let ids = texts
        .iter()
        .enumerate()
        .map(|(index, text)| {
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

/// The page's top-level blocks, one line each: a table as `table(header)` and its rows as
/// `[a|b]`, anything else as `kind: text`.
fn picture(editor: &PageEditor) -> Vec<String> {
    let doc = editor.doc();
    let text = |b| doc.plain_text(b).unwrap().unwrap_or_default();
    let mut out = Vec::new();
    for block in doc.root_blocks() {
        match doc.kind(block).unwrap() {
            BlockKind::Table { header_row } => {
                out.push(format!("table({})", if header_row { "header" } else { "plain" }));
                for row in doc.table_cells(block).unwrap() {
                    let cells: Vec<String> = row.into_iter().map(text).collect();
                    out.push(format!("[{}]", cells.join("|")));
                }
            }
            kind => out.push(format!("{}: {}", kind.type_name(), text(block))),
        }
    }
    out
}

fn tables(editor: &PageEditor) -> Vec<BlockId> {
    let doc = editor.doc();
    doc.root_blocks()
        .into_iter()
        .filter(|&b| matches!(doc.kind(b).unwrap(), BlockKind::Table { .. }))
        .collect()
}

#[test]
fn slash_table_on_an_empty_line_puts_a_three_by_three_table_there_with_the_caret_in_it() {
    let (mut editor, ids) = paragraphs(&["intro", "", "outro"]);
    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.insert_text("/").unwrap();
    editor.insert_text("table").unwrap();
    let menu = editor.slash_menu().unwrap().unwrap();
    assert_eq!(menu.items[0].label, "Table");
    let caret = editor.slash_apply(None).unwrap().unwrap();
    assert_eq!(
        picture(&editor),
        [
            "paragraph: intro",
            "table(header)",
            "[||]",
            "[||]",
            "[||]",
            "paragraph: outro"
        ]
    );
    let table = tables(&editor)[0];
    assert_eq!(caret, at(editor.doc().table_cells(table).unwrap()[0][0], 0));
    assert_eq!(editor.caret().unwrap(), Some(caret));
    // Undo gives back the typed `/table`.
    editor.undo().unwrap();
    assert_eq!(
        picture(&editor),
        ["paragraph: intro", "paragraph: /table", "paragraph: outro"]
    );
}

#[test]
fn slash_table_after_text_adds_the_table_below_the_line() {
    let (mut editor, ids) = paragraphs(&["Plants: "]);
    editor.set_caret(at(ids[0], 8)).unwrap();
    editor.insert_text("/").unwrap();
    editor.insert_text("tab").unwrap();
    editor.slash_apply(None).unwrap();
    assert_eq!(
        picture(&editor),
        ["paragraph: Plants: ", "table(header)", "[||]", "[||]", "[||]"]
    );
    let first = editor.doc().table_cells(tables(&editor)[0]).unwrap()[0][0];
    assert_eq!(editor.caret().unwrap(), Some(at(first, 0)));
}

#[test]
fn a_markdown_table_pastes_as_a_table_with_or_without_outer_pipes_and_with_inline_marks() {
    let (mut editor, ids) = paragraphs(&[""]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor
        .paste_text(
            "| Plant | Bed |\n|:---|---:|\n| **Basil** | `pots` |\n| a \\| b | line<br>two |\nPeas | fence\n",
        )
        .unwrap();
    assert_eq!(
        picture(&editor),
        [
            "table(header)",
            "[Plant|Bed]",
            "[Basil|pots]",
            "[a | b|line\ntwo]",
            "[Peas|fence]",
            "paragraph: "
        ]
    );
    let grid = editor.doc().table_cells(tables(&editor)[0]).unwrap();
    let basil = editor.doc().text(grid[1][0]).unwrap().unwrap();
    assert!(basil[0].marks.bold, "inline marks survive: {basil:?}");
    let pots = editor.doc().text(grid[1][1]).unwrap().unwrap();
    assert!(pots[0].marks.code);

    // Without outer pipes, short rows padded and long rows cut to the header's width.
    let (mut editor, ids) = paragraphs(&[""]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor
        .paste_text("Day | Job\n--- | ---\nMon\nTue | dig | extra")
        .unwrap();
    // "Mon" has no pipe, so the table ends before it.
    assert_eq!(
        picture(&editor),
        [
            "table(header)",
            "[Day|Job]",
            "paragraph: Mon",
            "paragraph: Tue | dig | extra"
        ]
    );
    let (mut editor, ids) = paragraphs(&[""]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor
        .paste_text("Day | Job\n--- | ---\nMon |\nTue | dig | extra")
        .unwrap();
    assert_eq!(
        picture(&editor),
        ["table(header)", "[Day|Job]", "[Mon|]", "[Tue|dig]", "paragraph: "]
    );
}

#[test]
fn rows_separated_by_blank_lines_still_paste_as_one_table() {
    let (mut editor, ids) = paragraphs(&[""]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    let copied = "| Crate | Job |\n\n|---|---|\n\n| `leaf-core` | Shared IDs. |\n\n| `leaf-model` | The documents. |\n\nAfter the table.";
    editor.paste_text(copied).unwrap();
    assert_eq!(
        picture(&editor),
        [
            "table(header)",
            "[Crate|Job]",
            "[leaf-core|Shared IDs.]",
            "[leaf-model|The documents.]",
            "paragraph: After the table."
        ]
    );
}

#[test]
fn two_tables_and_text_that_only_looks_like_one_stay_apart() {
    let (mut editor, ids) = paragraphs(&[""]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor
        .paste_text("a | b\n-|-\n1 | 2\n\nc | d\n-|-\n3 | 4\n\nchoose this | or that\nno delimiter here")
        .unwrap();
    assert_eq!(
        picture(&editor),
        [
            "table(header)",
            "[a|b]",
            "[1|2]",
            "table(header)",
            "[c|d]",
            "[3|4]",
            "paragraph: choose this | or that",
            "paragraph: no delimiter here"
        ]
    );
}

#[test]
fn a_table_pasted_mid_text_goes_between_the_two_halves_of_the_line() {
    let (mut editor, ids) = paragraphs(&["before|after"]);
    editor.set_caret(at(ids[0], 6)).unwrap();
    editor.paste_text("x | y\n--|--\n1 | 2").unwrap();
    assert_eq!(
        picture(&editor),
        [
            "paragraph: before",
            "table(header)",
            "[x|y]",
            "[1|2]",
            "paragraph: |after"
        ]
    );
    let after = editor.doc().root_blocks()[2];
    assert_eq!(editor.caret().unwrap(), Some(at(after, 0)));
    // One undo step.
    editor.undo().unwrap();
    assert_eq!(picture(&editor), ["paragraph: before|after"]);
}

#[test]
fn copying_a_table_writes_markdown_and_keeps_the_table_for_leaf() {
    let mut p = Page::new(2, 2);
    let cell = p.cell(1, 1);
    p.put(cell, 4);
    p.editor.insert_text("|\nmore").unwrap();
    p.editor.select_blocks(&[p.table]).unwrap();
    let copied = p.editor.copy().unwrap().unwrap();
    assert_eq!(
        copied.to_markdown(),
        "| r0c0 | r0c1 |\n| --- | --- |\n| r1c0 | r1c1\\|<br>more |"
    );
    assert_eq!(copied.to_plain_text(), "r0c0\tr0c1\nr1c0\tr1c1| more");

    // Leaf's own clipboard keeps it a table, and so does its Markdown read back.
    let through = Fragment::from_json(&copied.to_json()).unwrap();
    assert_eq!(through, copied);
    for fragment in [through, Fragment::Text(Vec::new())] {
        let (mut editor, ids) = paragraphs(&[""]);
        editor.set_caret(at(ids[0], 0)).unwrap();
        if let Fragment::Text(_) = fragment {
            editor.paste_text(&copied.to_markdown()).unwrap();
        } else {
            editor.paste(&fragment).unwrap();
        }
        assert_eq!(
            picture(&editor),
            [
                "table(header)",
                "[r0c0|r0c1]",
                "[r1c0|r1c1|\nmore]",
                "paragraph: "
            ]
        );
    }
}

#[test]
fn copying_selected_cells_copies_that_part_of_the_table() {
    let mut p = Page::new(3, 3);
    p.editor
        .select_between(at(p.cell(1, 1), 0), at(p.cell(2, 2), 0))
        .unwrap();
    let copied = p.editor.copy().unwrap().unwrap();
    assert_eq!(
        copied.to_markdown(),
        "| r1c1 | r1c2 |\n| --- | --- |\n| r2c1 | r2c2 |"
    );
    let Fragment::Blocks(blocks) = &copied else {
        panic!("expected blocks");
    };
    assert_eq!(
        blocks[0].kind,
        BlockKind::Table { header_row: false },
        "the header row wasn't copied"
    );
    p.editor
        .select_between(at(p.cell(0, 0), 0), at(p.cell(1, 0), 0))
        .unwrap();
    let Some(Fragment::Blocks(blocks)) = p.editor.copy().unwrap() else {
        panic!("expected blocks");
    };
    assert_eq!(blocks[0].kind, BlockKind::Table { header_row: true });
    // Cut still empties the cells.
    p.editor.cut().unwrap();
    assert_eq!(p.grid()[0][0], "");
    assert_eq!(p.grid()[1][0], "");
}

#[test]
fn a_table_pasted_into_a_cell_goes_in_as_lines_of_text() {
    let mut p = Page::new(1, 1);
    p.editor.select_blocks(&[p.table]).unwrap();
    let copied = p.editor.copy().unwrap().unwrap();
    let cell = p.cell(0, 0);
    p.put(cell, 4);
    p.editor.paste(&copied).unwrap();
    assert_eq!(p.text(cell), "r0c0r0c0");
    p.editor.paste_text("a | b\n-|-\n1 | 2").unwrap();
    assert_eq!(p.text(cell), "r0c0r0c0a\nb\n1\n2", "a line per cell");
    assert_eq!(tables(&p.editor).len(), 1, "no table inside a table");
}

#[test]
fn a_table_pasted_over_selected_blocks_goes_after_them() {
    let mut p = Page::new(1, 2);
    p.editor.select_blocks(&[p.table]).unwrap();
    let copied = p.editor.copy().unwrap().unwrap();
    p.put(p.above, 0);
    p.editor.select_blocks(&[p.above]).unwrap();
    p.editor.paste(&copied).unwrap();
    assert_eq!(
        picture(&p.editor),
        [
            "paragraph: above",
            "table(header)",
            "[r0c0|r0c1]",
            "paragraph: ",
            "table(header)",
            "[r0c0|r0c1]",
            "paragraph: below"
        ]
    );
}
