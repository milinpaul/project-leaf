//! The page list in a real (test) window: a 10,000-block page builds only the rows on screen
//! (E07-S01).

use gpui::{ListOffset, TestAppContext, VisualTestContext, px};
use leaf_editor::{BlockStyle, PageEditor};

use crate::shell::Shell;

const BLOCKS: usize = 10_000;

fn open_shell(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| window.draw(cx).clear(cx));
    (shell, cx)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// A new page with `count` blocks: numbered items, every tenth a nested bullet, the rest paragraphs.
fn big_page(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, count: usize) {
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    shell.update(cx, |shell, _| {
        let page: &mut PageEditor = shell.editor.open_page().unwrap().unwrap();
        let mut block = page.ensure_block().unwrap();
        page.set_text(block, "block 0").unwrap();
        for i in 1..count {
            block = page.enter(block).unwrap();
            page.set_text(block, &format!("block {i}")).unwrap();
            if i % 10 == 0 {
                page.turn_into(block, leaf_model::BlockKind::BulletedList)
                    .unwrap();
            }
        }
    });
}

#[gpui::test]
fn a_ten_thousand_block_page_builds_only_the_rows_on_screen(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    big_page(&shell, cx, BLOCKS);
    draw(cx);
    let (items, inputs) = shell.read_with(cx, |shell, _| {
        (shell.blocks.list.state.item_count(), shell.blocks.inputs.len())
    });
    assert_eq!(items, BLOCKS + 1, "the title plus every block is in the list");
    assert!(
        inputs > 0 && inputs < 150,
        "only the rows on screen (plus a little overdraw) were built: {inputs}"
    );

    // Scroll to the middle: rows there get built, and still only a screenful.
    shell.update(cx, |shell, _| {
        shell.blocks.list.state.scroll_to(ListOffset {
            item_ix: 5_000,
            offset_in_item: px(0.0),
        });
    });
    draw(cx);
    let (built_middle, inputs) = shell.read_with(cx, |shell, _| {
        let middle = shell.blocks.list.rows[5_000].id;
        (
            shell.blocks.inputs.contains_key(&middle),
            shell.blocks.inputs.len(),
        )
    });
    assert!(built_middle, "the row scrolled to is on screen");
    assert!(inputs < 300, "rows built so far stay few: {inputs}");
}

#[gpui::test]
fn editing_keeps_the_list_in_step_with_the_page(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    big_page(&shell, cx, 30);
    draw(cx);
    // Delete a block and add one: the list follows without being rebuilt from scratch.
    shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let rows = page.rows().unwrap();
        page.delete_block(rows[3].id).unwrap();
        page.enter(rows[10].id).unwrap();
    });
    draw(cx);
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.blocks.list.state.item_count(), 31);
        assert_eq!(shell.blocks.list.rows.len(), 30);
        assert!(
            matches!(shell.blocks.list.rows[9].style, BlockStyle::Bulleted),
            "row 10 of the page is a bullet"
        );
    });
}

#[gpui::test]
fn a_scroll_asked_for_right_after_opening_a_page_survives_the_reset(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    big_page(&shell, cx, 2_000);
    draw(cx);
    // Open another page and come back, asking for block 1,500 before the next frame.
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    draw(cx);
    let first = shell.read_with(cx, |shell, _| shell.editor().sidebar().unwrap()[0].page);
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.open(first, window, cx);
            shell.scroll_page_to(1_500, cx);
        });
    });
    draw(cx);
    let top = shell.read_with(cx, |shell, _| {
        shell.blocks.list.state.logical_scroll_top().item_ix
    });
    assert_eq!(top, 1 + 1_500, "the title is item 0");
}
