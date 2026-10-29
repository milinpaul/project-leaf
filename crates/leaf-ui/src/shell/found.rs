//! Showing a search result in place: the page opens with the matching block at the top, the caret at
//! its start, and the block briefly tinted so the eye finds it (E11-S06).

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, Context, ElementId, IntoElement, ParentElement as _, Styled,
    Window, div, ease_out_quint,
};
use gpui_component::ActiveTheme as _;
use leaf_core::BlockId;
use leaf_editor::Destination;
use leaf_model::Caret;

use super::{Shell, report};

/// How long the tint on a found block takes to fade.
const FLASH: Duration = Duration::from_millis(1600);

/// How strong the tint starts, as the accent colour's opacity.
const FLASH_STRENGTH: f32 = 0.22;

/// The block being flashed, and a count that makes each flash a new animation.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Flash {
    block: Option<BlockId>,
    count: u64,
}

impl Shell {
    /// Opens a search result: the page, and if a block matched, that block at the top of the view with
    /// the caret at its start and a fading tint on it.
    pub fn open_at(&mut self, destination: Destination, window: &mut Window, cx: &mut Context<Self>) {
        self.open(destination.page, window, cx);
        let Some(block) = destination.block else {
            return;
        };
        let index = match self.page().map(|page| page.rows()) {
            Some(Ok(rows)) => rows.iter().position(|row| row.id == block),
            Some(Err(err)) => return report("could not read the page's blocks", err),
            None => None,
        };
        // The block may have been deleted since the index last saw it: the page is open, that's all.
        let Some(index) = index else {
            return;
        };
        if let Some(Err(err)) = self.page().map(|page| page.set_caret(Caret { block, at: 0 })) {
            report("could not place the caret", err);
        }
        window.focus(&self.page_focus, cx);
        self.scroll_page_to(index, cx);
        self.blocks.flash = Flash {
            block: Some(block),
            count: self.blocks.flash.count + 1,
        };
        cx.notify();
    }

    /// The block flashed last, if any: for tests.
    #[must_use]
    pub fn flashed_block(&self) -> Option<BlockId> {
        self.blocks.flash.block
    }

    /// Wraps a block row in the fading tint if it is the one just found.
    pub(super) fn with_flash(&self, block: BlockId, row: AnyElement, cx: &Context<Self>) -> AnyElement {
        let Flash {
            block: Some(found),
            count,
        } = self.blocks.flash
        else {
            return row;
        };
        if found != block {
            return row;
        }
        let tint = cx.theme().primary;
        let radius = cx.theme().radius;
        div()
            .rounded(radius)
            .child(row)
            .with_animation(
                ElementId::NamedInteger("found-flash".into(), count),
                Animation::new(FLASH).with_easing(ease_out_quint()),
                move |row, done| row.bg(tint.opacity(FLASH_STRENGTH * (1.0 - done))),
            )
            .into_any_element()
    }
}
