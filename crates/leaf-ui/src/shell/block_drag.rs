//! Dragging blocks by their ⋮⋮ handle (E07-S05, ADR-050): the preview under the pointer, the
//! line where the blocks will land, and scrolling while the pointer is near the page's top or
//! bottom edge. Where a drop lands, and the move itself, are `leaf_editor`'s
//! (`PageEditor::drop_target` and `drop_blocks`, ADR-049); this module reports where rows were
//! drawn and where the pointer is, and draws the answer.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, Bounds, Context, DragMoveEvent, InteractiveElement as _, IntoElement, ParentElement, Pixels,
    Point, Render, SharedString, Styled, Task, Window, canvas, div, px,
};
use gpui_component::ActiveTheme as _;
use leaf_core::BlockId;
use leaf_editor::{BlockDrop, BlockRow, RowBox};

use super::block_row::INDENT_PX;
use super::{Shell, report};

/// How close to the page's top or bottom edge the pointer must be to scroll, and the scroll at the
/// edge, in pixels per tick. Past the edge it keeps speeding up, to `PAST_EDGE_FACTOR` times as fast.
/// Shared with drag-selecting text (`select_scroll`).
pub(super) const EDGE_PX: f32 = 48.0;
pub(super) const MAX_SCROLL_PX: f32 = 18.0;
const PAST_EDGE_FACTOR: f32 = 3.0;
/// How often the page scrolls while the pointer rests near an edge.
pub(super) const SCROLL_TICK: Duration = Duration::from_millis(16);
/// How much of the first line of the first dragged block's text the preview shows.
const PREVIEW_CHARS: usize = 60;

/// The value GPUI carries while a handle is dragged: the block whose handle was grabbed.
#[derive(Debug, Clone, Copy)]
pub(super) struct DraggedBlock(pub(super) BlockId);

/// What follows the pointer: the first dragged block's text, and how many more come with it.
pub(super) struct DragPreview {
    text: SharedString,
    more: usize,
}

impl DragPreview {
    /// A preview with nothing in it, for a drag the window has gone away under.
    pub(super) fn empty() -> Self {
        Self {
            text: SharedString::default(),
            more: 0,
        }
    }
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let more = (self.more > 0).then(|| {
            div()
                .text_color(theme.muted_foreground)
                .child(format!("+{}", self.more))
        });
        gpui_component::h_flex()
            .gap_2()
            .max_w(px(320.0))
            .px_2()
            .py_1()
            .rounded(theme.radius)
            .bg(theme.popover)
            // Drawn on its own, outside the page, so it inherits no text colour.
            .text_color(theme.popover_foreground)
            .border_1()
            .border_color(theme.border)
            .shadow_md()
            .text_sm()
            .child(div().truncate().child(self.text.clone()))
            .children(more)
    }
}

/// Every row drawn in the last frame, with its bounds. Cheap to clone; clones share the rows.
#[derive(Clone, Default)]
pub(super) struct DrawnRows(Rc<RefCell<Vec<Drawn>>>);

/// One drawn row: its block and bounds.
type Drawn = (BlockId, Bounds<Pixels>);

impl DrawnRows {
    /// Starts a new frame.
    pub(super) fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    /// An invisible element filling a row that records where the row was drawn.
    pub(super) fn recorder(&self, block: BlockId) -> AnyElement {
        let rows = self.clone();
        canvas(
            move |bounds, _, _| rows.0.borrow_mut().push((block, bounds)),
            |_, (), _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element()
    }

    /// Where `block`'s row was drawn in the last frame, if it was.
    pub(super) fn bounds_of(&self, block: BlockId) -> Option<Bounds<Pixels>> {
        self.0.borrow().iter().find(|(b, _)| *b == block).map(|(_, b)| *b)
    }

    /// The last row drawn starting at or above `y` that `accept`s, or else the first that does: the
    /// row a pointer at that height is over, skipping rows without text.
    pub(super) fn row_at(&self, y: Pixels, accept: impl Fn(BlockId) -> bool) -> Option<BlockId> {
        let mut rows = self.0.borrow().clone();
        rows.sort_by(|a, b| f32::from(a.1.top()).total_cmp(&f32::from(b.1.top())));
        let mut usable = rows.into_iter().filter(|(block, _)| accept(*block));
        let first = usable.next()?;
        let mut found = first.0;
        for (block, bounds) in std::iter::once(first).chain(usable) {
            if bounds.top() > y {
                break;
            }
            found = block;
        }
        Some(found)
    }

    /// The rows top to bottom as the editor wants them, and where top-level text starts.
    fn boxes(&self) -> (Vec<RowBox>, f32) {
        let mut rows = self.0.borrow().clone();
        rows.sort_by(|a, b| f32::from(a.1.top()).total_cmp(&f32::from(b.1.top())));
        let left = rows.first().map_or(0.0, |(_, b)| f32::from(b.left()));
        let boxes = rows
            .into_iter()
            .map(|(block, b)| RowBox {
                block,
                top: f32::from(b.top()),
                bottom: f32::from(b.bottom()),
            })
            .collect();
        (boxes, left)
    }
}

/// A drag in progress.
#[derive(Default)]
pub(super) struct BlockDrag {
    /// The blocks being dragged (the selection, or the grabbed block), in reading order.
    blocks: Vec<BlockId>,
    /// Those blocks and everything inside them, drawn faded.
    pub(super) faded: HashSet<BlockId>,
    /// Where they would land now.
    pub(super) drop: Option<BlockDrop>,
    /// The pointer, in window coordinates.
    pointer: Option<Point<Pixels>>,
    /// Pixels per tick to scroll: negative up, positive down, 0 when away from the edges.
    speed: f32,
    /// Scrolls while `speed` isn't 0.
    scroll: Option<Task<()>>,
}

/// How fast to scroll for a pointer at `y` over a page from `top` to `bottom`: faster the closer it
/// is to the edge and further still past it (E17-S02), nothing away from them.
pub(super) fn scroll_speed(y: f32, top: f32, bottom: f32) -> f32 {
    let into = |depth: f32| {
        if depth >= 0.0 {
            (EDGE_PX - depth).max(0.0) / EDGE_PX * MAX_SCROLL_PX
        } else {
            let past = (-depth / (2.0 * EDGE_PX)).min(1.0);
            MAX_SCROLL_PX * (1.0 + past * (PAST_EDGE_FACTOR - 1.0))
        }
    };
    if y < top + EDGE_PX {
        -into(y - top)
    } else if y > bottom - EDGE_PX {
        into(bottom - y)
    } else {
        0.0
    }
}

/// `blocks` and every row nested inside one of them, from the rows in reading order.
fn with_insides(rows: &[BlockRow], blocks: &[BlockId]) -> HashSet<BlockId> {
    let mut faded = HashSet::new();
    let mut inside: Option<usize> = None;
    for row in rows {
        if inside.is_some_and(|depth| row.depth > depth) {
            faded.insert(row.id);
            continue;
        }
        inside = None;
        if blocks.contains(&row.id) {
            faded.insert(row.id);
            inside = Some(row.depth);
        }
    }
    faded
}

impl Shell {
    /// A handle was dragged far enough to start: drag the selection when the block is in it, else
    /// the block. Returns the preview to show under the pointer.
    pub(super) fn start_block_drag(&mut self, block: BlockId, cx: &mut Context<Self>) -> DragPreview {
        let Some(page) = self.page() else {
            return DragPreview::empty();
        };
        page.close_block_menu();
        let blocks = page.action_targets(block).unwrap_or_else(|err| {
            report("could not start dragging", err);
            vec![block]
        });
        let first = blocks.first().copied().unwrap_or(block);
        let text: String = page
            .doc()
            .plain_text(first)
            .ok()
            .flatten()
            .unwrap_or_default()
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(PREVIEW_CHARS)
            .collect();
        let faded = with_insides(&self.blocks.list.rows, &blocks);
        let more = blocks.len().saturating_sub(1);
        self.blocks.dragging = Some(BlockDrag {
            blocks,
            faded,
            ..BlockDrag::default()
        });
        cx.notify();
        DragPreview {
            text: if text.is_empty() { " ".into() } else { text.into() },
            more,
        }
    }

    /// The pointer moved during a drag: work out the drop, and scroll near the page's edges.
    pub(super) fn on_block_drag_move(
        &mut self,
        event: &DragMoveEvent<DraggedBlock>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.blocks.dragging.as_mut() else {
            return;
        };
        let y = f32::from(event.event.position.y);
        drag.pointer = Some(event.event.position);
        drag.speed = scroll_speed(y, f32::from(event.bounds.top()), f32::from(event.bounds.bottom()));
        if drag.speed != 0.0 && drag.scroll.is_none() {
            drag.scroll = Some(cx.spawn(async move |shell, cx| {
                loop {
                    cx.background_executor().timer(SCROLL_TICK).await;
                    let going = shell.update(cx, Self::scroll_tick);
                    if !matches!(going, Ok(true)) {
                        break;
                    }
                }
            }));
        }
        self.update_drop(cx);
    }

    /// One step of scrolling during a drag. Returns whether to keep going.
    fn scroll_tick(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.blocks.dragging.as_mut() else {
            return false;
        };
        if drag.speed == 0.0 || !cx.has_active_drag() {
            drag.scroll = None;
            return false;
        }
        self.blocks.list.state.scroll_by(px(drag.speed));
        self.update_drop(cx);
        true
    }

    /// Works out where the dragged blocks would land for the pointer and the rows as last drawn.
    fn update_drop(&mut self, cx: &mut Context<Self>) {
        let Some((blocks, pointer)) = self
            .blocks
            .dragging
            .as_ref()
            .and_then(|drag| Some((drag.blocks.clone(), drag.pointer?)))
        else {
            return;
        };
        let (rows, left) = self.blocks.drawn.boxes();
        let x = f32::from(pointer.x) - left;
        let y = f32::from(pointer.y);
        let drop = self
            .page()
            .and_then(|page| page.drop_target(&blocks, &rows, x, y, INDENT_PX).ok().flatten());
        if let Some(drag) = self.blocks.dragging.as_mut()
            && drag.drop != drop
        {
            drag.drop = drop;
            cx.notify();
        }
    }

    /// The handle was let go over the page: move the blocks to the line.
    pub(super) fn on_block_drop(&mut self, _: &DraggedBlock, window: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.blocks.dragging.take() else {
            return;
        };
        if let Some(drop) = drag.drop
            && let Some(Err(err)) = self.page().map(|page| page.drop_blocks(&drag.blocks, &drop))
        {
            report("could not move the blocks", err);
        }
        self.focus_page(window, cx);
    }

    /// Where to press to drag block `from` by its handle, and where to let go to drop it on the lower
    /// half of block `to`'s row, `levels` indents in; from the rows drawn in the last frame. Used by
    /// the screenshot tool to show a drag in progress.
    pub fn block_drag_points(
        &self,
        from: BlockId,
        to: BlockId,
        levels: f32,
    ) -> Option<(Point<Pixels>, Point<Pixels>)> {
        let rows = self.blocks.drawn.0.borrow();
        let bounds = |block: BlockId| rows.iter().find(|(b, _)| *b == block).map(|(_, b)| *b);
        let (from, to) = (bounds(from)?, bounds(to)?);
        let left = rows.first().map_or(to.left(), |(_, b)| b.left());
        Some((
            gpui::point(from.left() - px(14.0), from.top() + px(14.0)),
            gpui::point(left + px(INDENT_PX * levels + 8.0), to.bottom() - px(4.0)),
        ))
    }

    /// Forgets a drag that ended without a drop on the page (let go elsewhere).
    pub(super) fn end_stale_drag(&mut self, cx: &Context<Self>) {
        if self.blocks.dragging.is_some() && !cx.has_active_drag() {
            self.blocks.dragging = None;
        }
    }

    /// The drop line for `row`, when the drop goes under it (or above it, for the first row), at
    /// the depth the blocks will have.
    pub(super) fn render_drop_line(
        &self,
        row: &BlockRow,
        first: bool,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let drop = self.blocks.dragging.as_ref()?.drop?;
        let under = drop.below == Some(row.id);
        if !under && (!first || drop.below.is_some()) {
            return None;
        }
        let depth = f32::from(u16::try_from(drop.depth).unwrap_or(u16::MAX));
        let line = div()
            .debug_selector(|| "block-drop-line".into())
            .absolute()
            .left(px(INDENT_PX * depth))
            .right_0()
            .h(px(3.0))
            .rounded_full()
            .bg(cx.theme().ring);
        Some(
            if under {
                line.bottom(px(-2.0))
            } else {
                line.top(px(-2.0))
            }
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests;
