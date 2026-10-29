//! The link popover and following links in a (test) window (E07-S07).

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};
use leaf_core::BlockId;
use leaf_model::{Caret, TextSpan};

use crate::menus::SearchOrLink;
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

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
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

fn spans(
    shell: &gpui::Entity<Shell>,
    cx: &mut VisualTestContext,
    block: BlockId,
) -> Vec<(String, Option<String>)> {
    shell.update(cx, |shell, _| {
        shell
            .page()
            .unwrap()
            .doc()
            .text(block)
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|TextSpan { text, marks }| (text, marks.link))
            .collect()
    })
}

fn link(
    shell: &gpui::Entity<Shell>,
    cx: &mut VisualTestContext,
    block: BlockId,
    range: std::ops::Range<usize>,
    href: &str,
) {
    shell.update(cx, |shell, _| {
        let page = shell.page().unwrap();
        page.select_between(
            Caret {
                block,
                at: range.start,
            },
            Caret { block, at: range.end },
        )
        .unwrap();
        page.set_link(href).unwrap();
    });
    draw(cx);
}

fn popover_open(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.link_popover_open())
}

fn page_focused(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| shell.read(cx).page_focus.is_focused(window))
}

fn s(text: &str, link: Option<&str>) -> (String, Option<String>) {
    (text.into(), link.map(Into::into))
}

#[gpui::test]
fn cmd_k_over_selected_text_links_it_with_the_typed_address(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx, "read the docs today");
    select(&shell, cx, block, 9, 13);
    cx.dispatch_action(SearchOrLink);
    draw(cx);
    assert!(popover_open(&shell, cx));
    assert!(cx.debug_bounds("link-popover").is_some());
    cx.simulate_input("example.com/docs");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert!(!popover_open(&shell, cx));
    assert_eq!(
        spans(&shell, cx, block),
        vec![
            s("read the ", None),
            s("docs", Some("example.com/docs")),
            s(" today", None)
        ]
    );
    assert!(page_focused(&shell, cx), "the page has the keyboard back");
}

#[gpui::test]
fn escape_and_clicks_elsewhere_close_it_without_changes(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx, "plain words");
    select(&shell, cx, block, 0, 5);
    cx.dispatch_action(SearchOrLink);
    draw(cx);
    cx.simulate_input("https://example.com");
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(!popover_open(&shell, cx));
    assert_eq!(spans(&shell, cx, block), vec![s("plain words", None)]);
    assert!(page_focused(&shell, cx));

    select(&shell, cx, block, 0, 5);
    cx.dispatch_action(SearchOrLink);
    draw(cx);
    let far = cx.debug_bounds("link-popover").unwrap().bottom_right() + point(px(150.0), px(150.0));
    cx.simulate_mouse_down(far, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(far, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert!(!popover_open(&shell, cx));
    assert_eq!(spans(&shell, cx, block), vec![s("plain words", None)]);
}

#[gpui::test]
fn an_existing_link_is_filled_in_and_can_be_changed_or_removed(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx, "see docs");
    link(&shell, cx, block, 4..8, "https://old.example");
    select(&shell, cx, block, 4, 8);
    cx.dispatch_action(SearchOrLink);
    draw(cx);
    // The old address is selected, so typing replaces it.
    cx.simulate_input("https://new.example");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(
        spans(&shell, cx, block),
        vec![s("see ", None), s("docs", Some("https://new.example"))]
    );

    select(&shell, cx, block, 4, 8);
    cx.dispatch_action(SearchOrLink);
    draw(cx);
    let remove = cx
        .debug_bounds("link-remove")
        .expect("Remove shows for a link")
        .center();
    cx.simulate_mouse_down(remove, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(remove, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert!(!popover_open(&shell, cx));
    assert_eq!(spans(&shell, cx, block), vec![s("see docs", None)]);
}

#[gpui::test]
fn cmd_click_on_a_link_opens_it_and_leaves_the_caret(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx, "go to docs now");
    link(&shell, cx, block, 6..10, "example.com");
    shell.update(cx, |shell, _| {
        shell.page().unwrap().set_caret(Caret { block, at: 0 }).unwrap();
    });
    draw(cx);
    let (origin, height) = shell
        .read_with(cx, |shell, _| shell.blocks.painted.caret_bounds(block, 8))
        .unwrap();
    let on_link = point(origin.x, origin.y + height / 2.0);
    let secondary = if cfg!(target_os = "macos") {
        Modifiers::command()
    } else {
        Modifiers::control()
    };
    cx.simulate_mouse_down(on_link, MouseButton::Left, secondary);
    cx.simulate_mouse_up(on_link, MouseButton::Left, secondary);
    draw(cx);
    assert_eq!(cx.opened_url().as_deref(), Some("https://example.com"));
    let caret = shell.update(cx, |shell, _| shell.page().unwrap().caret().unwrap());
    assert_eq!(caret, Some(Caret { block, at: 0 }), "the caret stays");

    // A plain click places the caret instead.
    cx.simulate_mouse_down(on_link, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(on_link, MouseButton::Left, Modifiers::none());
    draw(cx);
    let caret = shell.update(cx, |shell, _| shell.page().unwrap().caret().unwrap());
    assert_eq!(caret, Some(Caret { block, at: 8 }));
}
