//! E19-S02: editing rules for action items, one test per "Editor behaviour" row.

use leaf_model::{BlockKind, BlockProp, DueDate, PageCommand, Priority};

use super::{at, outline, page, press, press_and_undo, row};
use crate::{BlockStyle, Key, KeyOutcome};

fn task(priority: Option<Priority>) -> BlockKind {
    BlockKind::Task {
        done: false,
        due: None,
        time: None,
        priority,
    }
}

#[test]
fn row_enter_at_the_end_of_an_action_item_starts_the_next_one() {
    let (mut editor, ids) = page(&[(0, task(Some(Priority::High)), "Book flights")]);
    editor
        .doc
        .apply(PageCommand::SetProp {
            id: ids[0],
            prop: BlockProp::TaskDue(DueDate::new(2026, 11, 20).ok()),
        })
        .unwrap();
    let KeyOutcome::Caret(next) = press_and_undo(&mut editor, at(ids[0], 12), Key::Enter) else {
        panic!("a caret");
    };
    let rows = outline(&editor);
    assert_eq!(
        rows[1],
        row(0, task(Some(Priority::High)), ""),
        "no date, same priority"
    );
    assert_eq!(next.at, 0);
    assert!(
        matches!(rows[0].1, BlockKind::Task { due: Some(_), .. }),
        "the first keeps its date"
    );
}

#[test]
fn row_enter_on_an_empty_action_item_makes_a_paragraph() {
    let (mut editor, ids) = page(&[(0, task(None), "a"), (0, task(None), "")]);
    press_and_undo(&mut editor, at(ids[1], 0), Key::Enter);
    assert_eq!(outline(&editor)[1], row(0, BlockKind::Paragraph, ""));
}

#[test]
fn row_backspace_at_the_start_of_an_action_item_makes_a_paragraph() {
    let (mut editor, ids) = page(&[(0, task(Some(Priority::Low)), "Call Sam")]);
    press_and_undo(&mut editor, at(ids[0], 0), Key::Backspace);
    assert_eq!(outline(&editor), vec![row(0, BlockKind::Paragraph, "Call Sam")]);
}

#[test]
fn row_mod_enter_ticks_an_action_item_or_to_do_off_and_on() {
    let (mut editor, ids) = page(&[
        (0, task(None), "Pay rent"),
        (0, BlockKind::Todo { checked: false }, "milk"),
    ]);
    let outcome = press_and_undo(&mut editor, at(ids[0], 3), Key::ModEnter);
    assert_eq!(outcome, KeyOutcome::Caret(at(ids[0], 3)), "the caret stays");
    assert!(matches!(
        outline(&editor)[0].1,
        BlockKind::Task { done: true, .. }
    ));
    press(&mut editor, Key::ModEnter);
    assert!(matches!(
        outline(&editor)[0].1,
        BlockKind::Task { done: false, .. }
    ));
    press_and_undo(&mut editor, at(ids[1], 0), Key::ModEnter);
    assert_eq!(outline(&editor)[1].1, BlockKind::Todo { checked: true });
}

#[test]
fn row_typing_the_action_item_shortcut_makes_one() {
    let (mut editor, ids) = page(&[(0, BlockKind::Paragraph, "")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    for c in "[!] Water plants".chars() {
        editor.insert_text(&c.to_string()).unwrap();
    }
    assert_eq!(outline(&editor), vec![row(0, task(None), "Water plants")]);
    // Undo the typed words, then the conversion: the typed characters come back as plain text.
    while outline(&editor)[0].1 != BlockKind::Paragraph {
        editor.undo().unwrap();
    }
    assert_eq!(outline(&editor)[0].2, "[!] ");
}

#[test]
fn action_items_are_in_the_slash_menu_and_turn_into() {
    assert!(crate::turn_into_kinds().contains(&task(None)));
    let (mut editor, ids) = page(&[(0, BlockKind::Paragraph, "")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    for c in "/task".chars() {
        editor.insert_text(&c.to_string()).unwrap();
    }
    let menu = editor.slash_menu().unwrap().expect("the slash menu is open");
    assert_eq!(menu.items[0].label, "Action item", "best match for \"task\"");
    press(&mut editor, Key::Enter);
    assert_eq!(outline(&editor), vec![row(0, task(None), "")]);
}

#[test]
fn the_view_carries_an_action_items_fields() {
    let (mut editor, ids) = page(&[(0, task(Some(Priority::Medium)), "Renew passport")]);
    editor
        .doc
        .apply(PageCommand::SetProp {
            id: ids[0],
            prop: BlockProp::TaskDue(DueDate::new(2026, 12, 1).ok()),
        })
        .unwrap();
    let rows = editor.view().unwrap().to_vec();
    insta::assert_debug_snapshot!(rows[0].style, @r"
    Task {
        done: false,
        due: Some(
            DueDate {
                year: 2026,
                month: 12,
                day: 1,
            },
        ),
        time: None,
        priority: Some(
            Medium,
        ),
    }
    ");
    assert!(matches!(
        rows[0].style,
        BlockStyle::Task {
            done: false,
            priority: Some(Priority::Medium),
            ..
        }
    ));
}
