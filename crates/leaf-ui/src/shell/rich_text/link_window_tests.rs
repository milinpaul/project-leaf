//! Page links in a (test) window (E10-S04): drawn as the target's icon and title, positions that still
//! count a link as one character, a plain click that opens the page, and redrawing after a rename.

use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext, VisualTestContext};
use leaf_core::{BlockId, PageId};
use leaf_editor::{Fragment, LineLayout};
use leaf_model::{Caret, Marks, PAGE_LINK, TextSpan};

use crate::shell::Shell;

const NOW: i64 = 1_700_000_000_000;

/// A target page "Trip to Japan" (🗾) and, open, a page whose first line is "see " + link + " now".
fn open(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext, BlockId, PageId) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    let target = shell.update(cx, |shell, _| {
        let target = shell.editor.new_page(NOW).unwrap();
        shell.editor.rename(target, "Trip to Japan").unwrap();
        shell.editor.set_icon(target, Some("🗾")).unwrap();
        target
    });
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let block = shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let first = page.ensure_block().unwrap();
        page.set_text(first, "see  now").unwrap();
        page.set_caret(Caret { block: first, at: 4 }).unwrap();
        let link = TextSpan {
            text: PAGE_LINK.to_string(),
            marks: Marks {
                page: Some(target),
                ..Marks::default()
            },
        };
        page.paste(&Fragment::Text(vec![link])).unwrap();
        first
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    (shell, cx, block, target)
}

fn click(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>) {
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
}

/// The middle of the link (between page positions 4 and 5), at mid-line height.
fn middle_of_link(
    shell: &gpui::Entity<Shell>,
    cx: &mut VisualTestContext,
    block: BlockId,
) -> gpui::Point<gpui::Pixels> {
    shell.read_with(cx, |shell, _| {
        let (start, height) = shell.blocks.painted.caret_bounds(block, 4).unwrap();
        let (end, _) = shell.blocks.painted.caret_bounds(block, 5).unwrap();
        gpui::point((start.x + end.x) / 2.0, start.y + height / 2.0)
    })
}

#[gpui::test]
fn a_link_draws_as_its_label_and_still_counts_as_one_character(cx: &mut TestAppContext) {
    let (shell, cx, block, _) = open(cx);
    let painted = shell.read_with(cx, |shell, _| shell.blocks.painted.clone());
    assert_eq!(painted.text(block).as_deref(), Some("see 🗾 Trip to Japan now"));
    // The label is drawn wide, yet stepping over it is one position.
    let before = painted.offset_to_x(block, 4);
    let after = painted.offset_to_x(block, 5);
    assert!(after - before > 60.0, "the label takes room: {before} -> {after}");
    for offset in 0..=9 {
        let x = painted.offset_to_x(block, offset);
        assert_eq!(
            painted.x_to_offset(block, 0, x),
            offset,
            "offset {offset} round trips"
        );
    }
    // A point inside the label lands the caret at the link's nearer end, never inside it.
    let near_start = painted.x_to_offset(block, 0, before + 5.0);
    let near_end = painted.x_to_offset(block, 0, after - 5.0);
    assert_eq!((near_start, near_end), (4, 5));
}

#[gpui::test]
fn a_plain_click_on_a_link_opens_the_page(cx: &mut TestAppContext) {
    let (shell, cx, block, target) = open(cx);
    let at = middle_of_link(&shell, cx, block);
    click(cx, at);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.editor.selected()),
        Some(target)
    );
}

#[gpui::test]
fn a_click_beside_a_link_places_the_caret(cx: &mut TestAppContext) {
    let (shell, cx, block, target) = open(cx);
    let at = shell.read_with(cx, |shell, _| {
        let (top, height) = shell.blocks.painted.caret_bounds(block, 1).unwrap();
        gpui::point(top.x + gpui::px(0.5), top.y + height / 2.0)
    });
    click(cx, at);
    let (selected, caret) = shell.update(cx, |shell, _| {
        let selected = shell.editor.selected();
        (
            selected,
            shell.editor.open_page().unwrap().unwrap().caret().unwrap(),
        )
    });
    assert_ne!(selected, Some(target));
    assert_eq!(caret, Some(Caret { block, at: 1 }));
}

#[gpui::test]
fn renaming_or_trashing_the_target_redraws_the_link(cx: &mut TestAppContext) {
    let (shell, cx, block, target) = open(cx);
    shell.update(cx, |shell, _| shell.editor.rename(target, "Japan").unwrap());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let painted = shell.read_with(cx, |shell, _| shell.blocks.painted.clone());
    assert_eq!(painted.text(block).as_deref(), Some("see 🗾 Japan now"));

    shell.update(cx, |shell, _| shell.editor.trash(target, NOW).unwrap());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(painted.text(block).as_deref(), Some("see Japan (in trash) now"));
}

#[gpui::test]
fn a_link_to_a_deleted_page_says_so_and_a_click_places_the_caret(cx: &mut TestAppContext) {
    let (shell, cx, block, target) = open(cx);
    shell.update(cx, |shell, _| {
        shell.editor.trash(target, NOW).unwrap();
        shell.editor.empty_trash().unwrap();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let painted = shell.read_with(cx, |shell, _| shell.blocks.painted.clone());
    assert_eq!(painted.text(block).as_deref(), Some("see Deleted page now"));
    let at = middle_of_link(&shell, cx, block);
    let opened = shell.read_with(cx, |shell, _| shell.editor.selected());
    click(cx, at);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.editor.selected()),
        opened,
        "nothing to open"
    );
}

fn hover(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>) {
    cx.simulate_event(gpui::MouseMoveEvent {
        position: at,
        pressed_button: None,
        modifiers: Modifiers::none(),
    });
}

/// Where the caret at `at` in `block` is drawn, nudged right and to mid-line: over the character after it.
fn over_char(
    shell: &gpui::Entity<Shell>,
    cx: &mut VisualTestContext,
    block: BlockId,
    at: usize,
) -> gpui::Point<gpui::Pixels> {
    shell.read_with(cx, |shell, _| {
        let (top, height) = shell.blocks.painted.caret_bounds(block, at).unwrap();
        gpui::point(top.x + gpui::px(2.0), top.y + height / 2.0)
    })
}

#[gpui::test]
fn the_pointer_is_a_hand_over_a_link_and_an_i_beam_elsewhere(cx: &mut TestAppContext) {
    let (shell, cx, block, target) = open(cx);
    let hovered = |shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.blocks.link_hover)
    };
    let at = middle_of_link(&shell, cx, block);
    hover(cx, at);
    assert_eq!(hovered(&shell, cx), Some(block));
    let beside = over_char(&shell, cx, block, 1);
    hover(cx, beside);
    assert_eq!(hovered(&shell, cx), None, "back to the I-beam beside it");

    // A link to a deleted page has nothing to open.
    shell.update(cx, |shell, _| {
        shell.editor.trash(target, NOW).unwrap();
        shell.editor.empty_trash().unwrap();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let at = middle_of_link(&shell, cx, block);
    hover(cx, at);
    assert_eq!(hovered(&shell, cx), None);
}

#[gpui::test]
fn the_pointer_is_a_hand_over_a_web_link(cx: &mut TestAppContext) {
    let (shell, cx, block, _) = open(cx);
    shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        page.set_caret(Caret { block, at: 0 }).unwrap();
        let web = TextSpan {
            text: "site".into(),
            marks: Marks {
                link: Some("https://example.com".into()),
                ..Marks::default()
            },
        };
        page.paste(&Fragment::Text(vec![web])).unwrap();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let at = over_char(&shell, cx, block, 1);
    hover(cx, at);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.blocks.link_hover),
        Some(block)
    );
}
