//! The ⋮⋮ handle and block menu in a (test) window (E07-S04, E17-S03).

use std::time::Duration;

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, TestAppContext, VisualTestContext,
};
use leaf_core::BlockId;
use leaf_model::Caret;

use crate::shell::Shell;

fn open<'a>(
    cx: &'a mut TestAppContext,
    texts: &[&str],
) -> (gpui::Entity<Shell>, &'a mut VisualTestContext, Vec<BlockId>) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let ids = shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let mut ids = vec![page.ensure_block().unwrap()];
        page.set_text(ids[0], texts[0]).unwrap();
        for text in &texts[1..] {
            let next = page.enter(*ids.last().unwrap()).unwrap();
            page.set_text(next, text).unwrap();
            ids.push(next);
        }
        ids
    });
    draw(cx);
    (shell, cx, ids)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn texts(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.update(cx, |shell, _| {
        shell
            .page()
            .unwrap()
            .rows()
            .unwrap()
            .into_iter()
            .map(|row| row.text.unwrap_or_default())
            .collect()
    })
}

fn menu_open(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.update(cx, |shell, _| shell.block_menu().is_some())
}

fn click(cx: &mut VisualTestContext, selector: &str) {
    // Debug selectors are looked up by `&'static str`; a few leaked test strings are fine.
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    let at = cx.debug_bounds(selector).unwrap().center();
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

fn open_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd-/"
    } else {
        "ctrl-/"
    }
}

#[gpui::test]
fn the_keyboard_opens_the_menu_moves_through_it_and_gets_the_page_back(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &["a", "b", "c"]);
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.focus_page_at(Caret { block: ids[1], at: 1 }, window, cx);
        });
    });
    draw(cx);
    cx.simulate_keystrokes(open_shortcut());
    draw(cx);
    assert!(menu_open(&shell, cx));
    assert!(cx.debug_bounds("block-menu").is_some(), "drawn");
    // Turn into, Duplicate, Move up: Enter runs it.
    cx.simulate_keystrokes("down down enter");
    draw(cx);
    assert!(!menu_open(&shell, cx));
    assert_eq!(texts(&shell, cx), vec!["b", "a", "c"]);
    // The page has the keyboard again, caret where it was.
    cx.simulate_input("!");
    draw(cx);
    assert_eq!(texts(&shell, cx), vec!["b!", "a", "c"]);

    // Turn into a quote through the list of kinds; Escape closes without doing anything.
    cx.simulate_keystrokes(&format!("{} right", open_shortcut()));
    draw(cx);
    assert!(cx.debug_bounds("block-menu-kinds").is_some());
    cx.simulate_keystrokes("escape escape");
    draw(cx);
    assert!(!menu_open(&shell, cx));
    assert_eq!(texts(&shell, cx), vec!["b!", "a", "c"]);
}

#[gpui::test]
fn the_handle_opens_the_menu_and_clicks_choose(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &["a", "b", "c"]);
    click(cx, &format!("block-handle-{}", ids[2]));
    assert!(menu_open(&shell, cx));
    // Move down is greyed out for the last block: clicking it does nothing.
    click(cx, "block-menu-3");
    assert!(menu_open(&shell, cx));
    click(cx, "block-menu-2");
    assert_eq!(texts(&shell, cx), vec!["a", "c", "b"]);
    assert!(!menu_open(&shell, cx));

    click(cx, &format!("block-handle-{}", ids[0]));
    click(cx, "block-menu-4");
    assert_eq!(texts(&shell, cx), vec!["c", "b"], "Delete");

    // A click elsewhere closes the menu.
    click(cx, &format!("block-handle-{}", ids[1]));
    assert!(menu_open(&shell, cx));
    let far =
        cx.debug_bounds("block-menu").unwrap().bottom_right() + gpui::point(gpui::px(200.0), gpui::px(200.0));
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: far,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    draw(cx);
    assert!(!menu_open(&shell, cx));
}

/// Moves the pointer onto the middle of `selector`, then lets `ms` milliseconds pass.
fn hover(cx: &mut VisualTestContext, selector: &str, ms: u64) {
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    let at = cx.debug_bounds(selector).unwrap().center();
    cx.simulate_event(MouseMoveEvent {
        position: at,
        pressed_button: None,
        modifiers: Modifiers::none(),
    });
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
    draw(cx);
}

fn kinds_open(cx: &mut VisualTestContext) -> bool {
    cx.debug_bounds("block-menu-kinds").is_some()
}

#[gpui::test]
fn hovering_turn_into_opens_the_list_and_a_diagonal_path_into_it_keeps_it(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &["a", "b", "c"]);
    click(cx, &format!("block-handle-{}", ids[1]));
    assert!(menu_open(&shell, cx));

    // Passing over "Turn into" doesn't open it; resting on it does.
    hover(cx, "block-menu-0", 50);
    hover(cx, "block-menu-1", 200);
    assert!(!kinds_open(cx), "passed over, not opened");
    hover(cx, "block-menu-0", 200);
    assert!(kinds_open(cx), "opened by resting on Turn into, without a click");

    // Down and across to "Heading 2" (the third kind), over Duplicate and Move up on the way.
    hover(cx, "block-menu-1", 60);
    hover(cx, "block-menu-2", 60);
    hover(cx, "block-menu-kinds-2", 400);
    assert!(kinds_open(cx), "still open after crossing the other entries");
    click(cx, "block-menu-kinds-2");
    assert!(!menu_open(&shell, cx));
    let kind = shell.update(cx, |shell, _| shell.page().unwrap().doc().kind(ids[1]).unwrap());
    assert_eq!(kind, leaf_model::BlockKind::Heading { level: 2 });

    // Resting on another entry closes the list; a click on Turn into opens it again and keeps it.
    click(cx, &format!("block-handle-{}", ids[0]));
    hover(cx, "block-menu-0", 200);
    assert!(kinds_open(cx));
    hover(cx, "block-menu-1", 400);
    assert!(!kinds_open(cx), "closed after resting on Duplicate");
    click(cx, "block-menu-0");
    assert!(kinds_open(cx), "a click opens it");
    click(cx, "block-menu-0");
    assert!(kinds_open(cx), "a second click keeps it open");
}

#[test]
fn every_entry_has_an_icon_that_loads() {
    use gpui::AssetSource as _;
    use leaf_editor::{MenuEntry, turn_into_kinds};
    let assets = crate::assets();
    let entries = [
        MenuEntry::TurnInto,
        MenuEntry::Duplicate,
        MenuEntry::MoveUp,
        MenuEntry::MoveDown,
        MenuEntry::Delete,
    ]
    .into_iter()
    .chain(turn_into_kinds().into_iter().map(MenuEntry::Kind));
    for entry in entries {
        let icon = super::icon_of(&entry);
        let svg = assets.load(icon.path().as_ref()).unwrap();
        assert!(svg.is_some_and(|svg| svg.starts_with(b"<svg")), "{entry:?}");
    }
}
