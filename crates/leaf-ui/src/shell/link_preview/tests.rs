//! Window tests for the link preview card (E10-S12): it waits on a link before showing, goes when the
//! pointer leaves (after a grace pause), on a key or Esc, never shows while a button is down or when
//! turned off, and a click on it opens the page.

use std::time::Duration;

use gpui::{Modifiers, MouseButton, MouseMoveEvent, TestAppContext, VisualTestContext};
use leaf_core::{BlockId, PageId};
use leaf_editor::Fragment;
use leaf_model::{BlockKind, Caret, Marks, PAGE_LINK, PageCommand, PageDoc, TextSpan};

use super::{CLOSE_DELAY, OPEN_DELAY};
use crate::shell::Shell;

const NOW: i64 = 1_700_000_000_000;

/// "Trips" › "Japan" (🗾), whose page has a heading and a line, and, open, a page whose first line is
/// "see " + a link to Japan + " now".
fn open(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext, BlockId, PageId) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    let target = shell.update(cx, |shell, _| {
        let trips = shell.editor.new_page(NOW).unwrap();
        shell.editor.rename(trips, "Trips").unwrap();
        let target = shell.editor.new_page_in(Some(trips), NOW).unwrap();
        shell.editor.rename(target, "Japan").unwrap();
        shell.editor.set_icon(target, Some("🗾")).unwrap();
        let mut doc = PageDoc::new();
        for (index, (kind, text)) in [
            (BlockKind::Heading { level: 1 }, "Plan"),
            (BlockKind::Paragraph, "Book the ryokan"),
        ]
        .into_iter()
        .enumerate()
        {
            doc.apply(PageCommand::InsertBlock {
                parent: None,
                index,
                kind,
                text: text.into(),
            })
            .unwrap();
        }
        shell.editor.insert_page(target, doc).unwrap();
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
    draw(cx);
    (shell, cx, block, target)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn wait(cx: &mut VisualTestContext, delay: Duration) {
    cx.executor().advance_clock(delay);
    cx.run_until_parked();
    draw(cx);
}

fn move_to(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, pressed: Option<MouseButton>) {
    cx.simulate_event(MouseMoveEvent {
        position: at,
        pressed_button: pressed,
        modifiers: Modifiers::none(),
    });
}

/// The middle of the link, and a point over the "e" of "see" beside it, at mid-line height.
fn points(
    shell: &gpui::Entity<Shell>,
    cx: &mut VisualTestContext,
    block: BlockId,
) -> (gpui::Point<gpui::Pixels>, gpui::Point<gpui::Pixels>) {
    shell.read_with(cx, |shell, _| {
        let (start, height) = shell.blocks.painted.caret_bounds(block, 4).unwrap();
        let (end, _) = shell.blocks.painted.caret_bounds(block, 5).unwrap();
        let (beside, _) = shell.blocks.painted.caret_bounds(block, 1).unwrap();
        let mid = start.y + height / 2.0;
        (
            gpui::point((start.x + end.x) / 2.0, mid),
            gpui::point(beside.x + gpui::px(2.0), mid),
        )
    })
}

fn shown(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> Option<PageId> {
    shell.read_with(cx, |shell, _| shell.link_preview_page())
}

#[gpui::test]
fn the_card_shows_after_a_pause_under_the_link(cx: &mut TestAppContext) {
    let (shell, cx, block, target) = open(cx);
    let (link, _) = points(&shell, cx, block);
    move_to(cx, link, None);
    wait(cx, OPEN_DELAY / 2);
    assert_eq!(shown(&shell, cx), None, "not yet");
    assert!(cx.debug_bounds("link-preview").is_none());
    wait(cx, OPEN_DELAY);
    assert_eq!(shown(&shell, cx), Some(target));
    let card = cx.debug_bounds("link-preview").expect("the card is drawn");
    assert!(card.origin.y > link.y, "below the link's line, not over it");
    let preview = shell.read_with(cx, |shell, _| {
        shell.blocks.preview.shown.as_ref().unwrap().preview.clone()
    });
    assert_eq!(preview.path, vec!["Trips".to_owned()]);
    assert_eq!(preview.lines.len(), 2);
}

#[gpui::test]
fn leaving_the_link_hides_it_after_a_grace_pause(cx: &mut TestAppContext) {
    let (shell, cx, block, target) = open(cx);
    let (link, beside) = points(&shell, cx, block);
    move_to(cx, link, None);
    wait(cx, OPEN_DELAY * 2);
    move_to(cx, beside, None);
    assert_eq!(shown(&shell, cx), Some(target), "time to reach the card");
    wait(cx, CLOSE_DELAY / 2);
    move_to(cx, link, None);
    wait(cx, CLOSE_DELAY * 2);
    assert_eq!(shown(&shell, cx), Some(target), "back on the link: it stays");
    move_to(cx, beside, None);
    wait(cx, CLOSE_DELAY * 2);
    assert_eq!(shown(&shell, cx), None);

    // Resting beside the link never opens one.
    wait(cx, OPEN_DELAY * 2);
    assert_eq!(shown(&shell, cx), None);
}

#[gpui::test]
fn a_key_or_escape_hides_it(cx: &mut TestAppContext) {
    let (shell, cx, block, target) = open(cx);
    let (link, _) = points(&shell, cx, block);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.focus_page(window, cx)));
    move_to(cx, link, None);
    wait(cx, OPEN_DELAY * 2);
    assert_eq!(shown(&shell, cx), Some(target));
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert_eq!(shown(&shell, cx), None);

    // Typing closes it too, and still types.
    move_to(cx, gpui::point(link.x + gpui::px(1.0), link.y), None);
    wait(cx, OPEN_DELAY * 2);
    assert_eq!(shown(&shell, cx), Some(target));
    cx.simulate_keystrokes("x");
    draw(cx);
    assert_eq!(shown(&shell, cx), None);
    let text = shell.update(cx, |shell, _| {
        shell
            .editor
            .open_page()
            .unwrap()
            .unwrap()
            .doc()
            .plain_text(block)
            .unwrap()
            .unwrap()
    });
    assert!(text.contains('x'), "the key was typed: {text:?}");
}

#[gpui::test]
fn no_card_while_a_button_is_down_or_when_turned_off(cx: &mut TestAppContext) {
    let (shell, cx, block, _) = open(cx);
    let (link, _) = points(&shell, cx, block);
    move_to(cx, link, Some(MouseButton::Left));
    wait(cx, OPEN_DELAY * 2);
    assert_eq!(shown(&shell, cx), None, "selecting, not resting");

    cx.update(|_, cx| crate::settings::update(cx, |s| s.set_link_previews(false)));
    move_to(cx, gpui::point(link.x + gpui::px(1.0), link.y), None);
    wait(cx, OPEN_DELAY * 2);
    assert_eq!(shown(&shell, cx), None, "turned off in Settings");
}

#[gpui::test]
fn a_deleted_page_has_no_card(cx: &mut TestAppContext) {
    let (shell, cx, block, target) = open(cx);
    shell.update(cx, |shell, _| {
        shell.editor.trash(target, NOW).unwrap();
        shell.editor.empty_trash().unwrap();
    });
    draw(cx);
    let (link, _) = points(&shell, cx, block);
    move_to(cx, link, None);
    wait(cx, OPEN_DELAY * 2);
    assert_eq!(shown(&shell, cx), None);
}

#[gpui::test]
fn a_click_on_the_card_opens_the_page(cx: &mut TestAppContext) {
    let (shell, cx, block, target) = open(cx);
    let (link, _) = points(&shell, cx, block);
    move_to(cx, link, None);
    wait(cx, OPEN_DELAY * 2);
    let card = cx.debug_bounds("link-preview").unwrap().center();
    move_to(cx, card, None);
    wait(cx, CLOSE_DELAY * 2);
    assert_eq!(shown(&shell, cx), Some(target), "on the card: it stays");
    cx.simulate_click(card, Modifiers::none());
    draw(cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.editor.selected()),
        Some(target)
    );
    assert_eq!(shown(&shell, cx), None);
}
