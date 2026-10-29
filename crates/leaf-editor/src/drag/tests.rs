//! Tests for dragging blocks (E05-S04): drop places from a fake layout, and the move.

use leaf_core::BlockId;
use leaf_model::BlockKind;
use proptest::prelude::*;

use crate::block_actions::tests::{outline, p, page};
use crate::{BlockDrop, PageEditor, RowBox};

const ROW: f32 = 24.0;
const INDENT: f32 = 24.0;

/// Every row drawn 24 px tall, one under the other, as the UI would lay them out.
fn layout(editor: &PageEditor) -> Vec<RowBox> {
    let mut top = 0.0;
    editor
        .rows()
        .unwrap()
        .into_iter()
        .map(|row| {
            let b = RowBox {
                block: row.id,
                top,
                bottom: top + ROW,
            };
            top += ROW;
            b
        })
        .collect()
}

/// The pointer `levels` indents in, over the top (`upper`) or bottom half of row `row`.
fn target(editor: &mut PageEditor, dragged: &[BlockId], row: usize, upper: bool, levels: f32) -> BlockDrop {
    let rows = layout(editor);
    let y = rows[row].top + if upper { 4.0 } else { ROW - 4.0 };
    editor
        .drop_target(dragged, &rows, levels * INDENT + 2.0, y, INDENT)
        .unwrap()
        .unwrap()
}

fn texts(editor: &PageEditor) -> Vec<(usize, String)> {
    outline(editor).into_iter().map(|(d, _, t)| (d, t)).collect()
}

fn t(depth: usize, text: &str) -> (usize, String) {
    (depth, text.into())
}

#[test]
fn the_pointer_height_picks_the_gap_and_its_distance_from_the_left_the_depth() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b"), p(0, "c")]);
    let (a, c) = (ids[0], ids[2]);
    let before_a = target(&mut editor, &[c], 0, true, 3.0);
    assert_eq!(
        before_a,
        BlockDrop {
            below: None,
            depth: 0,
            parent: None,
            after: None
        },
        "nothing to nest in above the first row"
    );
    let after_a = target(&mut editor, &[c], 0, false, 0.0);
    assert_eq!(
        (after_a.below, after_a.depth, after_a.parent, after_a.after),
        (Some(a), 0, None, Some(a))
    );
    assert_eq!(
        target(&mut editor, &[c], 1, true, 0.0),
        after_a,
        "the same gap from below"
    );
    let into_a = target(&mut editor, &[c], 0, false, 1.0);
    assert_eq!((into_a.depth, into_a.parent, into_a.after), (1, Some(a), None));
    assert_eq!(
        target(&mut editor, &[c], 0, false, 9.0),
        into_a,
        "one level deeper at most"
    );

    assert!(editor.drop_blocks(&[c], &into_a).unwrap());
    assert_eq!(texts(&editor), vec![t(0, "a"), t(1, "c"), t(0, "b")]);
    assert!(editor.drop_blocks(&[c], &before_a).unwrap());
    assert_eq!(texts(&editor), vec![t(0, "c"), t(0, "a"), t(0, "b")]);
}

#[test]
fn below_the_last_child_the_depth_can_climb_back_out() {
    let (mut editor, ids) = page(&[p(0, "a"), p(1, "a1"), p(0, "b"), p(0, "c")]);
    let (a, a1, c) = (ids[0], ids[1], ids[3]);
    let place = |editor: &mut PageEditor, levels| {
        let d = target(editor, &[c], 1, false, levels);
        (d.depth, d.parent, d.after)
    };
    assert_eq!(place(&mut editor, 0.0), (0, None, Some(a)), "after a, before b");
    assert_eq!(
        place(&mut editor, 1.0),
        (1, Some(a), Some(a1)),
        "after a1, inside a"
    );
    assert_eq!(place(&mut editor, 2.0), (2, Some(a1), None), "into a1");
    assert_eq!(place(&mut editor, 7.0), (2, Some(a1), None));
    let into_a1 = target(&mut editor, &[c], 1, false, 2.0);
    editor.drop_blocks(&[c], &into_a1).unwrap();
    assert_eq!(texts(&editor), vec![t(0, "a"), t(1, "a1"), t(2, "c"), t(0, "b")]);
}

#[test]
fn under_a_block_with_children_the_drop_becomes_its_first_child() {
    let (mut editor, ids) = page(&[p(0, "a"), p(1, "a1"), p(0, "b")]);
    let (a, b) = (ids[0], ids[2]);
    let d = target(&mut editor, &[b], 0, false, 0.0);
    assert_eq!(
        (d.depth, d.parent, d.after),
        (1, Some(a), None),
        "where the line is drawn"
    );
    editor.drop_blocks(&[b], &d).unwrap();
    assert_eq!(texts(&editor), vec![t(0, "a"), t(1, "b"), t(1, "a1")]);
}

#[test]
fn blocks_that_cannot_hold_children_are_not_nested_into() {
    let (mut editor, ids) = page(&[
        (0, BlockKind::Heading { level: 1 }, "title"),
        (
            0,
            BlockKind::Code {
                language: String::new(),
            },
            "code",
        ),
        p(0, "x"),
    ]);
    let x = ids[2];
    for row in [0, 1] {
        let d = target(&mut editor, &[x], row, false, 3.0);
        assert_eq!((d.depth, d.parent), (0, None));
    }
}

#[test]
fn a_block_can_never_be_dropped_inside_itself() {
    let (mut editor, ids) = page(&[p(0, "a"), p(1, "a1"), p(2, "a11"), p(0, "b")]);
    let (a, a1, b) = (ids[0], ids[1], ids[3]);
    // Over its own rows, the only gap is the one it is in: dropping there changes nothing.
    for row in [0, 1, 2] {
        for upper in [true, false] {
            let d = target(&mut editor, &[a], row, upper, 5.0);
            assert_eq!((d.below, d.depth, d.parent), (None, 0, None));
            assert!(!editor.drop_blocks(&[a], &d).unwrap());
        }
    }
    // A place inside it, made up by hand, is refused as well.
    let inside = BlockDrop {
        below: Some(a1),
        depth: 2,
        parent: Some(a1),
        after: None,
    };
    assert!(!editor.drop_blocks(&[a], &inside).unwrap());
    let before = texts(&editor);
    assert_eq!(before, vec![t(0, "a"), t(1, "a1"), t(2, "a11"), t(0, "b")]);
    assert!(!editor.undo().unwrap(), "nothing to undo");

    // Into the block below works, and takes the children along.
    let into_b = target(&mut editor, &[a], 3, false, 1.0);
    assert_eq!(into_b.parent, Some(b));
    assert!(editor.drop_blocks(&[a], &into_b).unwrap());
    assert_eq!(
        texts(&editor),
        vec![t(0, "b"), t(1, "a"), t(2, "a1"), t(3, "a11")]
    );
}

#[test]
fn several_blocks_move_together_in_order_as_one_undo_step() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b"), p(0, "c"), p(0, "d")]);
    let (b, c) = (ids[1], ids[2]);
    editor.select_blocks(&[b, c]).unwrap();
    let dragged = editor.action_targets(c).unwrap();
    assert_eq!(dragged, vec![b, c]);
    let end = target(&mut editor, &dragged, 3, false, 0.0);
    assert!(editor.drop_blocks(&[c, b], &end).unwrap(), "listed backwards");
    assert_eq!(texts(&editor), vec![t(0, "a"), t(0, "d"), t(0, "b"), t(0, "c")]);
    assert!(editor.undo().unwrap());
    assert_eq!(texts(&editor), vec![t(0, "a"), t(0, "b"), t(0, "c"), t(0, "d")]);
    assert!(!editor.undo().unwrap(), "one step");
}

#[test]
fn a_selected_child_travels_with_its_selected_parent() {
    let (mut editor, ids) = page(&[p(0, "a"), p(1, "a1"), p(0, "b")]);
    let (a, a1) = (ids[0], ids[1]);
    let end = target(&mut editor, &[a1, a], 2, false, 0.0);
    assert_eq!(end.after, Some(ids[2]));
    editor.drop_blocks(&[a1, a], &end).unwrap();
    assert_eq!(texts(&editor), vec![t(0, "b"), t(0, "a"), t(1, "a1")]);
}

#[test]
fn pointers_off_the_rows_use_the_nearest_row_and_no_rows_means_no_drop() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b")]);
    let (a, b) = (ids[0], ids[1]);
    let rows = layout(&editor);
    let below_all = editor
        .drop_target(&[a], &rows, 0.0, 500.0, INDENT)
        .unwrap()
        .unwrap();
    assert_eq!((below_all.after, below_all.depth), (Some(b), 0));
    let above_all = editor
        .drop_target(&[b], &rows, -40.0, -50.0, INDENT)
        .unwrap()
        .unwrap();
    assert_eq!((above_all.below, above_all.after), (None, None));
    assert_eq!(editor.drop_target(&[a], &[], 0.0, 0.0, INDENT).unwrap(), None);
    assert_eq!(editor.drop_target(&[], &rows, 0.0, 0.0, INDENT).unwrap(), None);
    let zero = editor.drop_target(&[b], &rows, 90.0, 20.0, 0.0).unwrap().unwrap();
    assert_eq!(zero.depth, 0, "no indent width: no nesting");
    assert!(!editor.drop_blocks(&[], &zero).unwrap());
}

#[test]
fn a_drop_after_a_block_that_has_gone_does_nothing() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b"), p(0, "c")]);
    let stale = target(&mut editor, &[ids[2]], 0, false, 0.0);
    editor.block_action(ids[0], crate::BlockAction::Delete).unwrap();
    assert!(!editor.drop_blocks(&[ids[2]], &stale).unwrap());
    assert_eq!(texts(&editor), vec![t(0, "b"), t(0, "c")]);
}

/// A random page: each block at most one level deeper than the one before.
fn shape() -> impl Strategy<Value = Vec<usize>> {
    prop::collection::vec(0..3_usize, 1..12).prop_map(|steps| {
        let mut depth = 0;
        steps
            .into_iter()
            .enumerate()
            .map(|(i, step)| {
                depth = if i == 0 { 0 } else { (depth + 1).min(step) };
                depth
            })
            .collect()
    })
}

proptest! {
    /// Wherever the pointer is, the drop lands exactly where the line was drawn: the dragged blocks
    /// end up right under the row the line is under, at the line's depth, in order, with nothing lost.
    #[test]
    fn every_drop_lands_where_the_line_is(
        depths in shape(),
        pick in any::<prop::sample::Index>(),
        count in 1..3_usize,
        y in -30.0_f32..320.0,
        x in -10.0_f32..120.0,
    ) {
        let texts_in: Vec<String> = (0..depths.len()).map(|i| format!("b{i}")).collect();
        let blocks: Vec<_> = depths.iter().zip(&texts_in).map(|(&d, t)| p(d, t)).collect();
        let (mut editor, ids) = page(&blocks);
        let first = pick.index(ids.len());
        let dragged: Vec<BlockId> = ids[first..(first + count).min(ids.len())].to_vec();
        let rows = layout(&editor);
        let drop = editor.drop_target(&dragged, &rows, x, y, INDENT).unwrap().unwrap();
        let before = editor.rows().unwrap().len();
        editor.drop_blocks(&dragged, &drop).unwrap();
        let after = editor.rows().unwrap();
        prop_assert_eq!(after.len(), before, "nothing lost");
        prop_assert_eq!(editor.doc().check_invariants(), Ok(()));
        // The outermost dragged blocks, in reading order, right under the line.
        let roots: Vec<BlockId> = after
            .iter()
            .filter(|r| dragged.contains(&r.id))
            .filter(|r| {
                editor.doc().parent(r.id).unwrap().is_none_or(|parent| {
                    let mut up = Some(parent);
                    while let Some(b) = up {
                        if dragged.contains(&b) {
                            return false;
                        }
                        up = editor.doc().parent(b).unwrap();
                    }
                    true
                })
            })
            .map(|r| r.id)
            .collect();
        let at = after.iter().position(|r| r.id == roots[0]).unwrap();
        let line = match drop.below {
            Some(below) => after.iter().position(|r| r.id == below).map(|i| i + 1),
            None => Some(0),
        };
        // The first root sits right under the line's row, unless that row's own subtree is in between.
        if drop.after.is_none() {
            prop_assert_eq!(Some(at), line);
        }
        prop_assert_eq!(after[at].depth, drop.depth);
        prop_assert_eq!(editor.doc().parent(roots[0]).unwrap(), drop.parent);
        let siblings = match drop.parent {
            Some(parent) => editor.doc().children(parent).unwrap(),
            None => editor.doc().root_blocks(),
        };
        let start = siblings.iter().position(|b| *b == roots[0]).unwrap();
        prop_assert_eq!(&siblings[start..start + roots.len()], roots.as_slice(), "together, in order");
        let previous = start.checked_sub(1).map(|i| siblings[i]);
        prop_assert_eq!(previous, drop.after);
    }
}
