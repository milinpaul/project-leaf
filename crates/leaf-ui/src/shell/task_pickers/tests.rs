//! An action item in a (test) window (E19-S03): the round check, the chips, the date picker and the
//! priority menu, by mouse and by keyboard.

use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext, VisualTestContext};
use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, DueDate, Priority};

use super::TaskPickerKind;
use crate::shell::Shell;
use crate::shell::task_row::local_now;

fn open(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext, BlockId) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let id = shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let id = page.ensure_block().unwrap();
        page.set_text(id, "Send the invoice").unwrap();
        page.turn_into(
            id,
            BlockKind::Task {
                done: false,
                due: None,
                time: None,
                priority: None,
            },
        )
        .unwrap();
        id
    });
    draw(cx);
    (shell, cx, id)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn click(cx: &mut VisualTestContext, selector: &str) {
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is drawn"))
        .center();
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    draw(cx);
}

fn task(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, id: BlockId) -> BlockKind {
    shell.update(cx, |shell, _| shell.page().unwrap().doc().kind(id).unwrap())
}

#[gpui::test]
fn the_round_check_ticks_the_item_off_and_on(cx: &mut TestAppContext) {
    let (shell, cx, id) = open(cx);
    click(cx, &format!("task-check-{id}"));
    assert!(matches!(task(&shell, cx, id), BlockKind::Task { done: true, .. }));
    click(cx, &format!("task-check-{id}"));
    assert!(matches!(
        task(&shell, cx, id),
        BlockKind::Task { done: false, .. }
    ));
}

#[gpui::test]
fn the_date_picker_sets_a_day_a_time_and_clears_them(cx: &mut TestAppContext) {
    let (shell, cx, id) = open(cx);
    let (today, _) = local_now().unwrap();
    let tomorrow = leaf_editor::add_days(today, 1).unwrap();

    // "Add date" opens the picker; a quick pick sets the date and closes it.
    click(cx, &format!("task-date-{id}"));
    assert_eq!(
        shell.read_with(cx, |s, _| s.task_picker_open()),
        Some(TaskPickerKind::Date)
    );
    click(cx, "quick-tomorrow");
    assert_eq!(shell.read_with(cx, |s, _| s.task_picker_open()), None);
    assert!(
        matches!(task(&shell, cx, id), BlockKind::Task { due: Some(d), time: None, .. } if d == tomorrow)
    );

    // A day on the calendar. It reopens on tomorrow's month (not today's, on a month's last day), so
    // pick another day in that month.
    let other = if tomorrow.day() == 15 { 16 } else { 15 };
    let picked = DueDate::new(tomorrow.year(), tomorrow.month(), other).unwrap();
    click(cx, &format!("task-date-{id}"));
    click(cx, &format!("day-{picked}"));
    assert!(matches!(task(&shell, cx, id), BlockKind::Task { due: Some(d), .. } if d == picked));

    // A typed time, with Enter.
    click(cx, &format!("task-date-{id}"));
    cx.simulate_input("9:30pm");
    cx.simulate_keystrokes("enter");
    draw(cx);
    let time = leaf_editor::parse_time("21:30");
    assert!(matches!(task(&shell, cx, id), BlockKind::Task { time: t, .. } if t == time));

    // Reopened, the old time is selected, so typing replaces it.
    click(cx, &format!("task-date-{id}"));
    cx.simulate_input("8am");
    cx.simulate_keystrokes("enter");
    draw(cx);
    let time = leaf_editor::parse_time("8:00");
    assert!(matches!(task(&shell, cx, id), BlockKind::Task { time: t, .. } if t == time));

    // The month arrows don't pick anything; Clear takes the date and time away.
    click(cx, &format!("task-date-{id}"));
    click(cx, "month-next");
    assert_eq!(
        shell.read_with(cx, |s, _| s.task_picker_open()),
        Some(TaskPickerKind::Date)
    );
    click(cx, "clear-date");
    assert!(matches!(
        task(&shell, cx, id),
        BlockKind::Task {
            due: None,
            time: None,
            ..
        }
    ));
}

#[gpui::test]
fn the_priority_menu_works_with_the_mouse_and_the_keys(cx: &mut TestAppContext) {
    let (shell, cx, id) = open(cx);
    click(cx, &format!("task-priority-{id}"));
    assert_eq!(
        shell.read_with(cx, |s, _| s.task_picker_open()),
        Some(TaskPickerKind::Priority)
    );
    click(cx, "priority-0");
    assert!(matches!(
        task(&shell, cx, id),
        BlockKind::Task {
            priority: Some(Priority::High),
            ..
        }
    ));

    // From the keyboard: the shortcut opens it on the current priority; Down twice, Enter.
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.focus_page_at(Caret { block: id, at: 0 }, window, cx);
        });
    });
    draw(cx);
    let shortcut = if cfg!(target_os = "macos") {
        "cmd-shift-p"
    } else {
        "ctrl-shift-p"
    };
    cx.simulate_keystrokes(shortcut);
    draw(cx);
    assert_eq!(
        shell.read_with(cx, |s, _| s.task_picker_open()),
        Some(TaskPickerKind::Priority)
    );
    cx.simulate_keystrokes("down down enter");
    draw(cx);
    assert!(matches!(
        task(&shell, cx, id),
        BlockKind::Task {
            priority: Some(Priority::Low),
            ..
        }
    ));
    // Escape closes without a change.
    cx.simulate_keystrokes(shortcut);
    cx.simulate_keystrokes("down escape");
    draw(cx);
    assert_eq!(shell.read_with(cx, |s, _| s.task_picker_open()), None);
    assert!(matches!(
        task(&shell, cx, id),
        BlockKind::Task {
            priority: Some(Priority::Low),
            ..
        }
    ));
}

#[gpui::test]
fn the_date_shortcut_opens_the_picker_for_the_caret_item(cx: &mut TestAppContext) {
    let (shell, cx, id) = open(cx);
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.focus_page_at(Caret { block: id, at: 2 }, window, cx);
        });
    });
    draw(cx);
    let shortcut = if cfg!(target_os = "macos") {
        "cmd-shift-d"
    } else {
        "ctrl-shift-d"
    };
    cx.simulate_keystrokes(shortcut);
    draw(cx);
    assert_eq!(
        shell.read_with(cx, |s, _| s.task_picker_open()),
        Some(TaskPickerKind::Date)
    );
    // A date chip once a date is set; a far past one is overdue (drawn in red, checked in the editor).
    click(cx, "quick-today");
    shell.update(cx, |shell, _| {
        let page = shell.page().unwrap();
        page.set_task_due(id, DueDate::new(2001, 1, 1).ok()).unwrap();
    });
    draw(cx);
    assert!(
        cx.debug_bounds(Box::leak(format!("task-date-{id}").into_boxed_str()))
            .is_some()
    );
}
