//! An action item keeps all its fields through Leaf's own clipboard (E19-S01), and through Markdown
//! for other apps: a to-do line ending in its fields (E19-S04).

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
    assert_eq!(
        back.to_markdown(),
        "- [x] Send invoice (due 2026-10-31 14:00, high)"
    );
}

#[test]
fn an_unreadable_task_field_makes_the_copy_unreadable() {
    // As for a kind Leaf doesn't know (ADR-030): the paste falls back to the copy's plain text.
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

fn text_of(editor: &crate::PageEditor, row: usize) -> String {
    outline(editor)[row].2.iter().map(|s| s.text.as_str()).collect()
}

#[test]
fn markdown_out_and_back_gives_the_same_action_items() {
    let plain = BlockKind::Task {
        done: false,
        due: None,
        time: None,
        priority: None,
    };
    let todo = BlockKind::Todo { checked: false };
    let (mut editor, ids) = page(&[
        (0, task(), "Send invoice"),
        (0, plain.clone(), "Call the plumber"),
        (0, todo.clone(), "Pick a level (high)"),
        (0, todo.clone(), "Call Sam (after lunch)"),
        p(0, ""),
    ]);
    editor.select_between(at(ids[0], 0), at(ids[3], 22)).unwrap();
    let markdown = editor.copy().unwrap().unwrap().to_markdown();
    insta::assert_snapshot!(markdown, @r"
    - [x] Send invoice (due 2026-10-31 14:00, high)
    - [ ] Call the plumber (action item)
    - [ ] Pick a level \(high)
    - [ ] Call Sam (after lunch)
    ");
    editor.set_caret(at(ids[4], 0)).unwrap();
    editor.paste_text(&markdown).unwrap();
    let rows = outline(&editor);
    assert_eq!(rows[4].1, task());
    assert_eq!(text_of(&editor, 4), "Send invoice");
    assert_eq!(rows[5].1, plain);
    assert_eq!(text_of(&editor, 5), "Call the plumber");
    assert_eq!(rows[6].1, todo);
    assert_eq!(text_of(&editor, 6), "Pick a level (high)");
    assert_eq!(rows[7].1, todo);
    assert_eq!(text_of(&editor, 7), "Call Sam (after lunch)");
}

#[test]
fn pasted_markdown_from_another_app_makes_action_items_and_to_dos() {
    let (mut editor, ids) = page(&[p(0, "")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor
        .paste_text("- [ ] Book flights (Medium)\n- [x] Renew passport (due 2026-10-28)\n- [ ] Buy milk\n")
        .unwrap();
    let rows = outline(&editor);
    assert!(matches!(
        rows[0].1,
        BlockKind::Task {
            done: false,
            due: None,
            priority: Some(Priority::Medium),
            ..
        }
    ));
    assert!(
        matches!(rows[1].1, BlockKind::Task { done: true, due: Some(d), time: None, .. } if d.to_string() == "2026-10-28")
    );
    assert_eq!(rows[2].1, BlockKind::Todo { checked: false });
    assert_eq!(text_of(&editor, 2), "Buy milk");
}
