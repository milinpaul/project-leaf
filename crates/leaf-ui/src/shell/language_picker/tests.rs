//! The code block's language picker, and Tab inside code, in a (test) window (E07-S09).

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};
use leaf_core::BlockId;
use leaf_editor::BlockAction;
use leaf_model::{BlockKind, Caret};

use crate::shell::Shell;

fn open(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext, BlockId) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let block = shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let block = page.ensure_block().unwrap();
        page.set_text(block, "let x = 1;").unwrap();
        let code = BlockKind::Code {
            language: String::new(),
        };
        page.block_action(block, BlockAction::TurnInto(code)).unwrap();
        block
    });
    draw(cx);
    (shell, cx, block)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn click(cx: &mut VisualTestContext, selector: &str) {
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    let at = cx.debug_bounds(selector).unwrap().center();
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    draw(cx);
}

fn language(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) -> String {
    shell.update(cx, |shell, _| {
        match shell.page().unwrap().doc().kind(block).unwrap() {
            BlockKind::Code { language } => language,
            other => panic!("not code: {other:?}"),
        }
    })
}

fn picker_open(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.language_picker_open())
}

#[gpui::test]
fn typing_in_the_picker_and_pressing_enter_sets_the_language(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    click(cx, &format!("code-language-{block}"));
    assert!(picker_open(&shell, cx));
    cx.simulate_input("rus");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert!(!picker_open(&shell, cx));
    assert_eq!(language(&shell, cx, block), "rust");
    let focused = cx.update(|window, cx| shell.read(cx).page_focus.is_focused(window));
    assert!(focused, "the page has the keyboard back");
}

#[gpui::test]
fn clicking_a_language_picks_it_and_escape_or_a_click_away_changes_nothing(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    click(cx, &format!("code-language-{block}"));
    // Python is below the list's fold; typing narrows the list to it.
    cx.simulate_input("p");
    draw(cx);
    click(cx, "language-python");
    assert_eq!(language(&shell, cx, block), "python");

    click(cx, &format!("code-language-{block}"));
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(!picker_open(&shell, cx));
    assert_eq!(language(&shell, cx, block), "python");

    click(cx, &format!("code-language-{block}"));
    let away = cx.debug_bounds("language-picker").unwrap().bottom_right() + point(px(120.0), px(120.0));
    cx.simulate_mouse_down(away, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(away, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert!(!picker_open(&shell, cx));
    assert_eq!(language(&shell, cx, block), "python");
}

#[gpui::test]
fn the_tab_key_indents_code_instead_of_nesting_it(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.focus_page_at(Caret { block, at: 0 }, window, cx);
        });
    });
    draw(cx);
    cx.simulate_keystrokes("tab");
    draw(cx);
    let text = shell.update(cx, |shell, _| {
        shell.page().unwrap().doc().plain_text(block).unwrap()
    });
    assert_eq!(text.as_deref(), Some("    let x = 1;"));
    cx.simulate_keystrokes("shift-tab");
    draw(cx);
    let text = shell.update(cx, |shell, _| {
        shell.page().unwrap().doc().plain_text(block).unwrap()
    });
    assert_eq!(text.as_deref(), Some("let x = 1;"));
}

#[test]
fn the_current_language_comes_first_until_something_is_typed() {
    let listed = super::listed("", "rust");
    assert_eq!(listed[0].id, "rust");
    assert_eq!(listed.len(), leaf_editor::CODE_LANGUAGES.len());
    assert_eq!(super::listed("", "")[0].label, "Plain text");
    assert_eq!(
        super::listed("", "elixir")[0].label,
        "Plain text",
        "an unlisted language isn't added"
    );
    assert_eq!(super::listed("py", "rust")[0].id, "python");
}
