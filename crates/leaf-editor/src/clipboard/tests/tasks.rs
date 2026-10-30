//! E19-S01: an action item keeps all its fields through Leaf's own clipboard; Markdown writes a to-do
//! line for it until E19-S04.

use leaf_model::{BlockKind, DueDate, DueTime, Priority};

use super::{at, outline, p, page};
use crate::clipboard::Fragment;

fn task() -> BlockKind {
    BlockKind::Task {
        done: true,
        due: DueDate::new(2026, 10, 31).ok(),
        time: DueTime::new(14, 0).ok(),
        priority: Some(Priority::High),
    }
}

#[test]
fn an_action_item_round_trips_through_the_clipboard() {
    let (mut editor, ids) = page(&[(0, task(), "Send invoice"), p(0, "")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    let json = editor.copy().unwrap().unwrap().to_json();
    assert!(
        json.contains(r#""type":"task""#) && json.contains(r#""due":"2026-10-31""#),
        "{json}"
    );
    let back = Fragment::from_json(&json).expect("Leaf reads its own copy");
    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.paste(&back).unwrap();
    assert_eq!(outline(&editor)[1].1, task());
    assert_eq!(back.to_markdown(), "- [x] Send invoice");
}

#[test]
fn an_unreadable_task_field_makes_the_copy_unreadable() {
    // As for a kind Leaf doesn't know (ADR 0030): the paste falls back to the copy's plain text.
    let json = r#"{"leaf_fragment":1,"blocks":[{"type":"task","due":"2026-02-30","text":[{"insert":"x"}]}]}"#;
    assert_eq!(Fragment::from_json(json), None);
    let good = r#"{"leaf_fragment":1,"blocks":[{"type":"task","priority":"low","text":[{"insert":"x"}]}]}"#;
    let Some(Fragment::Blocks(blocks)) = Fragment::from_json(good) else {
        panic!("a task with readable fields reads");
    };
    assert!(matches!(
        blocks[0].kind,
        BlockKind::Task {
            done: false,
            due: None,
            priority: Some(Priority::Low),
            ..
        }
    ));
}
