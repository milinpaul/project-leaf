//! Tests for setting an action item's fields and the pickers' logic (E19-S03).

use leaf_model::{BlockKind, DueDate, PageCommand, PageDoc, PageOutcome, Priority};

use super::{DatePicker, parse_time, priority_choices};
use crate::PageEditor;

fn d(text: &str) -> DueDate {
    text.parse().unwrap()
}

#[test]
fn the_calendar_shows_the_month_in_monday_first_weeks() {
    let picker = DatePicker::new(None, d("2026-10-30"));
    assert_eq!(picker.title(), "October 2026");
    let weeks = picker.weeks();
    assert_eq!(weeks.len(), 5);
    assert_eq!(weeks[0][..3], [None, None, None], "1 October 2026 is a Thursday");
    assert_eq!(weeks[0][3], Some(d("2026-10-01")));
    assert_eq!(weeks[4][5], Some(d("2026-10-31")));
    assert_eq!(weeks[4][6], None);
    // February 2027 starts on a Monday and fills exactly four weeks.
    let mut picker = DatePicker::new(Some(d("2027-02-10")), d("2026-10-30"));
    assert_eq!(picker.title(), "February 2027", "opens on the task's month");
    assert_eq!(picker.weeks().len(), 4);
    picker.next_month();
    assert_eq!(picker.title(), "March 2027");
    for _ in 0..3 {
        picker.previous_month();
    }
    assert_eq!(picker.title(), "December 2026");
}

#[test]
fn quick_picks_count_from_today() {
    let picker = DatePicker::new(None, d("2026-12-30"));
    assert_eq!(
        picker.quick_picks(),
        vec![
            ("Today", d("2026-12-30")),
            ("Tomorrow", d("2026-12-31")),
            ("Next week", d("2027-01-06")),
        ]
    );
}

#[test]
fn typed_times_read_in_common_forms() {
    let t = |text: &str| parse_time(text).map(|time| time.to_string());
    for (typed, read) in [
        ("9", "09:00"),
        ("9:30", "09:30"),
        ("09.30", "09:30"),
        ("0930", "09:30"),
        ("1745", "17:45"),
        ("9pm", "21:00"),
        ("9:30 am", "09:30"),
        ("12am", "00:00"),
        ("12 PM", "12:00"),
        (" 23:59 ", "23:59"),
    ] {
        assert_eq!(t(typed).as_deref(), Some(read), "{typed:?}");
    }
    // "é1": cutting the last two bytes off used to split the "é" and crash (E08-S15).
    for bad in [
        "", "25", "9:5", "13pm", "0am", "noon", "9:60", "-1", "12345", "é1", "1é5",
    ] {
        assert_eq!(t(bad), None, "{bad:?}");
    }
}

#[test]
fn setting_fields_is_one_undo_step_each_and_only_for_action_items() {
    let mut doc = PageDoc::new();
    let PageOutcome::BlockCreated(task) = doc
        .apply(PageCommand::InsertBlock {
            parent: None,
            index: 0,
            kind: BlockKind::Task {
                done: false,
                due: None,
                time: None,
                priority: None,
            },
            text: "Pay rent".into(),
        })
        .unwrap()
    else {
        panic!("a block");
    };
    let PageOutcome::BlockCreated(para) = doc
        .apply(PageCommand::InsertBlock {
            parent: None,
            index: 1,
            kind: BlockKind::Paragraph,
            text: "x".into(),
        })
        .unwrap()
    else {
        panic!("a block");
    };
    let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    doc.set_undo_merge_interval(0);
    let mut editor = PageEditor::new(doc);
    editor.set_task_due(task, Some(d("2026-11-01"))).unwrap();
    editor.set_task_time(task, parse_time("9am")).unwrap();
    editor.set_task_priority(task, Some(Priority::High)).unwrap();
    assert_eq!(
        editor.doc().kind(task).unwrap(),
        BlockKind::Task {
            done: false,
            due: Some(d("2026-11-01")),
            time: parse_time("09:00"),
            priority: Some(Priority::High),
        }
    );
    editor.undo().unwrap();
    assert!(matches!(
        editor.doc().kind(task).unwrap(),
        BlockKind::Task {
            priority: None,
            time: Some(_),
            ..
        }
    ));
    assert!(
        editor.set_task_priority(para, Some(Priority::Low)).is_err(),
        "not an action item"
    );
    assert_eq!(
        priority_choices().map(|(_, label)| label),
        ["High", "Medium", "Low", "No priority"]
    );
}
