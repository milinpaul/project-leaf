//! Callouts in a (test) window (E21-S02): the box, and changing the tone from its icon.

use gpui::{Modifiers, TestAppContext, VisualTestContext};
use leaf_core::BlockId;
use leaf_model::{BlockKind, CalloutTone};

use crate::shell::Shell;

fn open(
    cx: &mut TestAppContext,
    tone: CalloutTone,
) -> (gpui::Entity<Shell>, &mut VisualTestContext, BlockId) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let block = shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let block = page.ensure_block().unwrap();
        page.set_text(block, "Pack light: one 7 kg bag only").unwrap();
        page.turn_into(block, BlockKind::Callout { tone }).unwrap();
        block
    });
    draw(cx);
    (shell, cx, block)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not on screen"))
        .center();
    cx.simulate_click(at, Modifiers::none());
    draw(cx);
}

/// Selectors must be `'static`; leaking a few short strings in a test is fine.
fn selector(text: String) -> &'static str {
    text.leak()
}

fn tone(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) -> BlockKind {
    shell.update(cx, |shell, _| shell.page().unwrap().doc().kind(block).unwrap())
}

#[gpui::test]
fn a_callout_is_a_box_with_its_icon_before_the_text(cx: &mut TestAppContext) {
    let (_shell, cx, block) = open(cx, CalloutTone::Warning);
    let callout = cx
        .debug_bounds(selector(format!("callout-{block}")))
        .expect("the box is drawn");
    let icon = cx
        .debug_bounds(selector(format!("callout-icon-{block}")))
        .expect("with its icon");
    assert!(callout.contains(&icon.center()), "the icon sits inside the box");
    assert!(
        icon.left() > callout.left() && icon.top() > callout.top(),
        "with room around it"
    );
}

#[gpui::test]
fn the_icon_opens_the_tone_picker_and_a_tone_changes_the_callout(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx, CalloutTone::Info);
    assert!(cx.debug_bounds("callout-tones").is_none());
    click(cx, selector(format!("callout-icon-{block}")));
    assert!(cx.debug_bounds("callout-tones").is_some(), "the picker opens");
    for name in ["info", "success", "warning", "error", "note"] {
        assert!(
            cx.debug_bounds(selector(format!("callout-tone-{name}")))
                .is_some(),
            "{name} is offered"
        );
    }
    click(cx, "callout-tone-error");
    assert_eq!(
        tone(&shell, cx, block),
        BlockKind::Callout {
            tone: CalloutTone::Error
        }
    );
    assert!(cx.debug_bounds("callout-tones").is_none(), "choosing closes it");
}

#[gpui::test]
fn a_tone_from_a_newer_version_is_drawn_and_can_be_changed(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx, CalloutTone::Other("celebrate".into()));
    assert!(cx.debug_bounds(selector(format!("callout-{block}"))).is_some());
    click(cx, selector(format!("callout-icon-{block}")));
    click(cx, "callout-tone-success");
    assert_eq!(
        tone(&shell, cx, block),
        BlockKind::Callout {
            tone: CalloutTone::Success
        }
    );
}

#[gpui::test]
fn a_click_elsewhere_closes_the_picker_without_changing_anything(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx, CalloutTone::Note);
    click(cx, selector(format!("callout-icon-{block}")));
    let far = cx.debug_bounds(selector(format!("callout-{block}"))).unwrap();
    cx.simulate_click(
        gpui::point(far.right() - gpui::px(4.0), far.bottom() + gpui::px(200.0)),
        Modifiers::none(),
    );
    draw(cx);
    assert!(cx.debug_bounds("callout-tones").is_none());
    assert_eq!(
        tone(&shell, cx, block),
        BlockKind::Callout {
            tone: CalloutTone::Note
        }
    );
}

#[gpui::test]
fn callouts_in_a_row_are_apart_and_the_icon_sits_on_the_first_line(cx: &mut TestAppContext) {
    let (shell, cx, first) = open(cx, CalloutTone::Info);
    let second = shell.update(cx, |shell, _| {
        let page = shell.page().unwrap();
        let second = page.enter(first).unwrap();
        page.set_text(second, "Second").unwrap();
        page.turn_into(
            second,
            BlockKind::Callout {
                tone: CalloutTone::Error,
            },
        )
        .unwrap();
        second
    });
    draw(cx);
    let a = cx.debug_bounds(selector(format!("callout-{first}"))).unwrap();
    let b = cx.debug_bounds(selector(format!("callout-{second}"))).unwrap();
    let gap = f32::from(b.top() - a.bottom());
    assert!(gap >= 7.0, "a clear gap between the boxes: {gap}");

    let icon = cx
        .debug_bounds(selector(format!("callout-icon-{first}")))
        .unwrap();
    let from_top = f32::from(icon.top() - a.top());
    // The box's padding, then the icon dropped onto the middle of the first 28 px line.
    assert!(
        (13.0..=15.0).contains(&from_top),
        "icon {from_top} px below the box's top"
    );
}
