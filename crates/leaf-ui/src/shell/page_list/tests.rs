//! Tests for telling the virtual list what changed between frames.

use leaf_core::BlockId;
use leaf_editor::{BlockRow, BlockStyle};

use super::{Changes, changes};

fn row(id: BlockId, text: &str) -> BlockRow {
    BlockRow {
        id,
        depth: 0,
        style: BlockStyle::Paragraph,
        text: Some(text.into()),
        spans: None,
    }
}

fn ids(n: usize) -> Vec<BlockId> {
    (0..n).map(|_| BlockId::new()).collect()
}

fn rows(ids: &[BlockId]) -> Vec<BlockRow> {
    ids.iter().map(|&id| row(id, "text")).collect()
}

#[test]
fn nothing_changed_means_nothing_to_do() {
    let ids = ids(5);
    assert_eq!(changes(&rows(&ids), &rows(&ids)), Changes::default());
    assert_eq!(changes(&[], &[]), Changes::default());
}

#[test]
fn typing_in_a_block_remeasures_just_that_row() {
    let ids = ids(4);
    let mut after = rows(&ids);
    after[2] = row(ids[2], "longer text that may wrap");
    assert_eq!(
        changes(&rows(&ids), &after),
        Changes {
            remeasure: vec![2],
            splice: None
        }
    );
}

#[test]
fn a_new_block_is_spliced_in_where_it_appeared() {
    let ids = ids(4);
    let new = BlockId::new();
    let mut after = rows(&ids);
    after.insert(2, row(new, ""));
    assert_eq!(
        changes(&rows(&ids), &after),
        Changes {
            remeasure: vec![],
            splice: Some((2..2, 1))
        }
    );
    // Enter at the end of the page: appended.
    let mut appended = rows(&ids);
    appended.push(row(new, ""));
    assert_eq!(changes(&rows(&ids), &appended).splice, Some((4..4, 1)));
}

#[test]
fn deleted_and_moved_blocks_replace_the_run_between_unchanged_ends() {
    let ids = ids(6);
    let mut deleted = rows(&ids);
    deleted.drain(1..3);
    assert_eq!(changes(&rows(&ids), &deleted).splice, Some((1..3, 0)));

    // Block 4 moved up to position 1: rows 1..=4 change places.
    let moved: Vec<BlockRow> = [0, 4, 1, 2, 3, 5].iter().map(|&i| row(ids[i], "text")).collect();
    assert_eq!(changes(&rows(&ids), &moved).splice, Some((1..5, 4)));
}

#[test]
fn edits_in_the_tail_remeasure_by_their_old_position() {
    // A block is inserted near the top while the last block's text changes: the last row is
    // re-measured at its old position, before the splice shifts it.
    let ids = ids(4);
    let mut after = rows(&ids);
    after[3] = row(ids[3], "changed");
    after.insert(1, row(BlockId::new(), ""));
    assert_eq!(
        changes(&rows(&ids), &after),
        Changes {
            remeasure: vec![3],
            splice: Some((1..1, 1))
        }
    );
}

#[test]
fn a_whole_new_set_of_rows_is_one_splice() {
    assert_eq!(changes(&rows(&ids(3)), &rows(&ids(2))).splice, Some((0..3, 2)));
}
