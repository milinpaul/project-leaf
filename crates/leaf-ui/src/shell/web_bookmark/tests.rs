//! The web bookmark offer and address box in a (test) window (E15-S03).

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};
use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, WebPreview};

use crate::shell::Shell;

const LEAF: &str = "https://leaf.app/blog";

fn open(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext, BlockId) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let block = shell.update(cx, |shell, _| {
        shell.editor.open_page().unwrap().unwrap().ensure_block().unwrap()
    });
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.page().unwrap().set_caret(Caret { block, at: 0 }).unwrap();
            shell.focus_page(window, cx);
        });
    });
    draw(cx);
    (shell, cx, block)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn paste_address(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.page().unwrap().paste_text(LEAF).unwrap();
            shell.changed(window, cx);
        });
    });
    draw(cx);
}

fn kind(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) -> BlockKind {
    shell.update(cx, |shell, _| shell.page().unwrap().doc().kind(block).unwrap())
}

fn bookmark(url: &str) -> BlockKind {
    BlockKind::WebBookmark {
        url: url.into(),
        preview: WebPreview::default(),
    }
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).unwrap().center();
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    draw(cx);
}

#[gpui::test]
fn a_pasted_address_shows_the_offer_and_down_enter_makes_a_bookmark(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    paste_address(&shell, cx);
    assert!(
        cx.debug_bounds("bookmark-offer").is_some(),
        "the offer shows under the link"
    );
    cx.simulate_keystrokes("down enter");
    draw(cx);
    assert_eq!(kind(&shell, cx, block), bookmark(LEAF));
    assert!(cx.debug_bounds("bookmark-offer").is_none(), "and goes");
}

#[gpui::test]
fn escape_or_typing_closes_the_offer_and_keeps_the_link(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    paste_address(&shell, cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(cx.debug_bounds("bookmark-offer").is_none());
    assert_eq!(kind(&shell, cx, block), BlockKind::Paragraph);

    paste_address(&shell, cx);
    assert!(cx.debug_bounds("bookmark-offer").is_some());
    cx.simulate_input(" ");
    draw(cx);
    assert!(
        cx.debug_bounds("bookmark-offer").is_none(),
        "typing goes on as usual"
    );
    assert_eq!(kind(&shell, cx, block), BlockKind::Paragraph);
}

#[gpui::test]
fn clicking_create_web_bookmark_makes_one(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    paste_address(&shell, cx);
    click(cx, "bookmark-offer-1");
    assert_eq!(kind(&shell, cx, block), bookmark(LEAF));
}

#[gpui::test]
fn the_slash_menus_web_bookmark_asks_for_an_address_and_adds_it(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    cx.simulate_input("/bookmark");
    draw(cx);
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert!(cx.debug_bounds("web-address").is_some(), "the address box opens");
    cx.simulate_input("not an address");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert!(
        cx.debug_bounds("web-address").is_some(),
        "an address it can't take keeps it open"
    );
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            let open = shell.blocks.address.as_ref().unwrap();
            open.view.update(cx, |prompt, cx| {
                prompt
                    .input
                    .update(cx, |input, cx| input.set_value("leaf.app/blog", window, cx));
            });
        });
    });
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert!(cx.debug_bounds("web-address").is_none());
    assert_eq!(kind(&shell, cx, block), bookmark(LEAF), "https:// added");
}

#[gpui::test]
fn escape_or_a_click_away_closes_the_address_box_and_adds_nothing(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    cx.simulate_input("/bookmark");
    draw(cx);
    cx.simulate_keystrokes("enter");
    draw(cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(cx.debug_bounds("web-address").is_none());
    assert_eq!(kind(&shell, cx, block), BlockKind::Paragraph);

    cx.simulate_input("/bookmark");
    draw(cx);
    cx.simulate_keystrokes("enter");
    draw(cx);
    let far = cx.debug_bounds("web-address").unwrap().bottom_right() + point(px(150.0), px(150.0));
    cx.simulate_mouse_down(far, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(far, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert!(cx.debug_bounds("web-address").is_none());
    assert_eq!(kind(&shell, cx, block), BlockKind::Paragraph);
}
