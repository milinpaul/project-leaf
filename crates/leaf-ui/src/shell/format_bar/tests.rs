//! The floating formatting bar in a (test) window (E07-S13).

use gpui::{Modifiers, TestAppContext, VisualTestContext};
use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, Marks};

use crate::shell::Shell;

fn open<'a>(
    cx: &'a mut TestAppContext,
    text: &str,
) -> (gpui::Entity<Shell>, &'a mut VisualTestContext, BlockId) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let block = shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let block = page.ensure_block().unwrap();
        page.set_text(block, text).unwrap();
        block
    });
    draw(cx);
    (shell, cx, block)
}

/// Draws twice: the bar is placed by where the selection was drawn the frame before.
fn draw(cx: &mut VisualTestContext) {
    for _ in 0..2 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}

/// Selects characters `from..to` of `block` and gives the page the keyboard.
fn select(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId, from: usize, to: usize) {
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell
                .page()
                .unwrap()
                .select_between(Caret { block, at: from }, Caret { block, at: to })
                .unwrap();
            shell.focus_page(window, cx);
        });
    });
    draw(cx);
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not on screen"))
        .center();
    cx.simulate_click(at, Modifiers::none());
    draw(cx);
}

fn shown(cx: &mut VisualTestContext) -> bool {
    cx.debug_bounds("format-bar").is_some()
}

/// The block's text as (text, marks) runs.
fn runs(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) -> Vec<(String, Marks)> {
    shell.update(cx, |shell, _| {
        shell
            .page()
            .unwrap()
            .doc()
            .text(block)
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|span| (span.text, span.marks))
            .collect()
    })
}

fn kind(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) -> BlockKind {
    shell.update(cx, |shell, _| shell.page().unwrap().doc().kind(block).unwrap())
}

fn selection(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> Option<leaf_editor::Selection> {
    shell.update(cx, |shell, _| shell.page().unwrap().selection().unwrap())
}

#[gpui::test]
fn the_bar_shows_above_selected_text_only(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx, "pack light for the trip");
    assert!(!shown(cx), "not with just a caret");
    select(&shell, cx, block, 5, 10);
    assert!(shown(cx), "selected text shows the bar");
    for selector in [
        "format-turn-into",
        "format-bold",
        "format-italic",
        "format-strikethrough",
        "format-code",
        "format-link",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector} is offered");
    }

    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(!shown(cx), "Escape collapses the selection and the bar goes");
}

#[gpui::test]
fn a_mark_button_formats_the_selection_and_keeps_it(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx, "pack light");
    select(&shell, cx, block, 0, 4);
    let before = selection(&shell, cx);
    click(cx, "format-bold");
    let runs = runs(&shell, cx, block);
    assert_eq!(runs[0].0, "pack");
    assert!(runs[0].1.bold, "bold, as Cmd/Ctrl+B would");
    assert!(!runs[1].1.bold);
    assert_eq!(selection(&shell, cx), before, "the selection stays");
    assert!(shown(cx), "and so does the bar");

    click(cx, "format-bold");
    assert!(!runs_first_bold(&shell, cx, block), "a second click takes it off");
}

fn runs_first_bold(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) -> bool {
    runs(shell, cx, block)[0].1.bold
}

#[gpui::test]
fn turn_into_lists_the_kinds_and_changes_the_block(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx, "pack light");
    select(&shell, cx, block, 0, 4);
    assert!(cx.debug_bounds("format-kinds").is_none());
    click(cx, "format-turn-into");
    assert!(cx.debug_bounds("format-kinds").is_some(), "the list opens");
    assert!(shown(cx), "under the bar, which stays");
    click(cx, "format-kind-2");
    assert_eq!(kind(&shell, cx, block), BlockKind::Heading { level: 2 });
    assert!(
        cx.debug_bounds("format-kinds").is_none(),
        "choosing closes the list"
    );
    assert!(selection(&shell, cx).is_some(), "the text stays selected");
}

#[gpui::test]
fn link_opens_the_link_popover_in_place_of_the_bar(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx, "read the docs");
    select(&shell, cx, block, 9, 13);
    click(cx, "format-link");
    assert!(shell.read_with(cx, |shell, _| shell.link_popover_open()));
    assert!(!shown(cx), "the popover takes its place");
}

#[gpui::test]
fn in_a_code_block_only_turn_into_is_offered(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx, "let x = 1;");
    shell.update(cx, |shell, _| {
        let page = shell.page().unwrap();
        page.select_between(Caret { block, at: 0 }, Caret { block, at: 0 })
            .unwrap();
        page.block_action(
            block,
            leaf_editor::BlockAction::TurnInto(BlockKind::Code {
                language: String::new(),
            }),
        )
        .unwrap();
    });
    select(&shell, cx, block, 0, 3);
    assert!(shown(cx));
    assert!(cx.debug_bounds("format-turn-into").is_some());
    assert!(
        cx.debug_bounds("format-bold").is_none(),
        "code blocks take no marks"
    );
}
