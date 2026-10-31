//! The open page: the header (icon, cover, title), then the blocks, as a virtual list drawn by the
//! rich text view. A block's type is changed with the slash menu, the block handle's "Turn into" or
//! Markdown shortcuts (there is no toolbar, ADR-057). The page takes the keyboard itself (E07-S03): keys become editor actions
//! (`page_keys`), typed text and input-method drafts arrive through the platform's text input
//! (`ime`), and the mouse places the caret and selects (`rich_text`). Every decision is
//! `PageEditor`'s; this module only forwards and draws.

use std::collections::HashMap;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, ElementInputHandler, InteractiveElement as _, IntoElement, MouseButton,
    MouseUpEvent, ParentElement, Pixels, Point, Styled, Task, Window, canvas, div, list, px,
};
use gpui_component::v_flex;
use leaf_core::BlockId;
use leaf_editor::{PageEditor, Preedit};
use leaf_model::{BlockKind, Caret};

use super::block_drag::{BlockDrag, DrawnRows};
use super::column;
use super::image_row::{ImageResize, ImageRoom};
use super::page_list::{HEADER_ITEMS, PageList};
use super::rich_text::{Highlight, PaintedLayout, highlights};
use super::{Shell, report};

/// What the page needs between frames.
#[derive(Default)]
pub(super) struct PageState {
    /// The page's rows as a virtual list: only those on screen are laid out.
    pub(super) list: PageList,
    /// How the rows were laid out in the last frame, for Up/Down, clicks and input methods.
    pub(super) painted: PaintedLayout,
    /// The editor's selection, per row, as of this frame.
    pub(super) highlights: HashMap<BlockId, Highlight>,
    /// The editor's caret as of this frame, when the page has the keyboard.
    pub(super) caret: Option<Caret>,
    /// The input method's draft as of this frame, drawn underlined at the caret.
    pub(super) draft: Option<Preedit>,
    /// Whether the caret is in the visible half of its blink.
    pub(super) caret_on: bool,
    /// Toggles `caret_on` while a caret is drawn; dropped (stopped) otherwise.
    blink: Option<Task<()>>,
    /// Where a mouse drag started, while the button is down.
    pub(super) drag: Option<Caret>,
    /// The row whose link the pointer is over, drawn with a pointing-hand cursor (E10-S11).
    pub(super) link_hover: Option<BlockId>,
    /// What the block menu hangs from, set when it opens: the left edge and the top and bottom of the
    /// handle (or of the line, from the keyboard). The menu goes below it, or above when there's no room.
    pub(super) menu_at: Point<Pixels>,
    pub(super) menu_top: Pixels,
    /// Where the "Turn into" list was drawn, so a click in it doesn't count as outside the menu.
    pub(super) menu_list: Option<gpui::Bounds<Pixels>>,
    /// The block whose menu is open as of this frame, so its handle stays shown.
    pub(super) menu_block: Option<BlockId>,
    /// The pause before the block menu's "Turn into" list opens or closes under the pointer.
    pub(super) menu_hover: Option<Task<()>>,
    /// Each table's sideways scroll position (E16-S03), kept across frames.
    pub(super) table_scroll: HashMap<BlockId, gpui::ScrollHandle>,
    /// Where the page's list showed on screen, taken before the rows are drawn: a row can't ask the list while
    /// it is drawing (a wide table keeps its scrollbar inside it, E16-S05).
    pub(super) viewport: gpui::Bounds<Pixels>,
    /// A mouse-wheel glide in progress (E17-S01).
    pub(super) wheel: super::smooth_scroll::WheelGlide,
    /// Scrolling while a text selection is dragged near an edge (E17-S02).
    pub(super) select_scroll: super::select_scroll::SelectScroll,
    /// The block a search result opened at, tinted for a moment.
    pub(super) flash: super::found::Flash,
    /// Where each row was drawn in the last frame, for dropping dragged blocks.
    pub(super) drawn: DrawnRows,
    /// A drag of blocks by their handle, while it lasts.
    pub(super) dragging: Option<BlockDrag>,
    /// How much room each image row had in the last frame.
    pub(super) image_room: ImageRoom,
    /// Each picture drawn so far, checked once (ADR-078): its file, or why it isn't drawn. A stored
    /// picture never changes, since it is named by the hash of its bytes.
    pub(super) drawable: HashMap<leaf_core::AssetId, Result<std::path::PathBuf, leaf_store::SvgProblem>>,
    /// A resize of an image by its handle, while it lasts.
    pub(super) resizing: Option<ImageResize>,
    /// Where an image's resize handle was last pressed, for the resize it may start.
    pub(super) resize_press: Option<Pixels>,
    /// The link popover, while it is open.
    pub(super) link: Option<super::link_popover::OpenLink>,
    /// A callout's tone picker, while it is open (E21-S02).
    pub(super) tones: Option<super::callout::OpenTones>,
    /// The floating formatting bar over selected text (E07-S13).
    pub(super) format_bar: super::format_bar::FormatBarState,
    /// A code block's language picker, while it is open.
    pub(super) language: Option<super::language_picker::OpenLanguage>,
    /// An action item's date picker or priority menu, while open (E19-S03).
    pub(super) task_picker: Option<super::task_pickers::OpenTaskPicker>,
}

impl Shell {
    /// The open page's editor, reporting (and hiding) errors so the window stays usable.
    pub(super) fn page(&mut self) -> Option<&mut PageEditor> {
        self.editor.open_page().unwrap_or_else(|err| {
            report("could not open the page's blocks", err);
            None
        })
    }

    /// The block holding the caret, if the page has one.
    pub fn caret_block(&mut self) -> Option<BlockId> {
        self.page()
            .and_then(|page| page.caret().ok().flatten())
            .map(|c| c.block)
    }

    /// How many blocks the open page has, as last drawn.
    #[must_use]
    pub fn page_block_count(&self) -> usize {
        self.blocks.list.rows.len()
    }

    /// Scrolls the open page so block number `index` (in reading order, from 0) is at the top. Used
    /// to show a search result in place, and by the screenshot tool.
    pub fn scroll_page_to(&mut self, index: usize, cx: &mut Context<Self>) {
        self.blocks.list.scroll_to_block(index);
        cx.notify();
    }

    /// Gives the page the keyboard with the caret at `caret`, scrolled into view.
    pub fn focus_page_at(&mut self, caret: Caret, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Err(err)) = self.page().map(|page| page.set_caret(caret)) {
            report("could not place the caret", err);
        }
        self.focus_page(window, cx);
    }

    /// Gives the page the keyboard, keeping the caret and selection where they are.
    pub fn focus_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.page_focus, cx);
        self.reveal_caret();
        cx.notify();
    }

    /// Scrolls so the caret's block is on screen.
    ///
    /// A table's cells are drawn by the table's row; their own rows, listed after it, are empty. So for a
    /// caret in a cell this keeps the cell itself on screen, where it was drawn last frame: up and down in the
    /// page, and sideways in the table's box (E16-S05). Before the table has been drawn, its row is revealed.
    pub(super) fn reveal_caret(&mut self) {
        let Some(caret) = self.page().and_then(|page| page.caret().ok().flatten()) else {
            return;
        };
        let Some(table) = self.blocks.list.table_of(caret.block) else {
            return self.blocks.list.reveal(caret.block);
        };
        match self.blocks.painted.caret_bounds(caret.block, caret.at) {
            Some((at, line)) => {
                // The cell's padding and border around its line, so the whole cell comes into view.
                let room = px(super::table_row::CELL_ROOM);
                self.blocks.list.reveal_span(at.y - room, at.y + line + room);
                self.reveal_table_column(table, at.x);
            }
            None => self.blocks.list.reveal(table),
        }
    }

    /// Puts the caret at the start of the open page's first block, so a fresh page is ready for typing.
    pub(super) fn focus_first_block(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let first = self.page().and_then(|page| page.ensure_block().ok());
        if let Some(block) = first {
            self.focus_page_at(Caret { block, at: 0 }, window, cx);
        }
    }

    /// Applies a toolbar kind to the caret's block, or every block a selection touches.
    pub fn turn_focused_into(&mut self, kind: BlockKind, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(block) = self.caret_block() {
            let action = leaf_editor::BlockAction::TurnInto(kind);
            if let Some(Err(err)) = self.page().map(|page| page.block_action(block, action)) {
                report("could not change the block", err);
            }
        }
        self.focus_page(window, cx);
    }

    /// Ticks or unticks a to-do.
    pub fn toggle_todo(&mut self, id: BlockId, cx: &mut Context<Self>) {
        if let Some(Err(err)) = self.page().map(|page| page.toggle_checked(id)) {
            report("could not tick the to-do", err);
        }
        cx.notify();
    }

    /// The open page: the "Turn into" toolbar, then the title and blocks as a virtual list that
    /// scrolls. Only the rows on screen are built.
    pub(super) fn render_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.finish_image_resize(cx);
        // Only blocks that changed since the last frame are re-read (E04-S01).
        let rows = match self.page().map(|page| page.view().map(<[_]>::to_vec)) {
            Some(Ok(rows)) => rows,
            Some(Err(err)) => {
                report("could not read the page's blocks", err);
                Vec::new()
            }
            None => Vec::new(),
        };
        // The caret, selection and draft are drawn only while the page has the keyboard.
        let focused = self.page_focus.is_focused(window);
        let (selection, caret, draft) = match self.page() {
            Some(page) if focused => (
                page.selection().ok().flatten(),
                page.caret().ok().flatten(),
                page.preedit().ok().flatten(),
            ),
            Some(page) => (page.selection().ok().flatten(), None, None),
            None => (None, None, None),
        };
        self.blocks.highlights = highlights(&rows, selection.as_ref());
        self.blocks.caret = caret;
        self.blocks.draft = draft;
        self.blink_while(caret.is_some(), cx);
        // Placed by where the `/` was drawn last frame: this frame's layout is only known once painted.
        let slash = self.render_slash_popover(window, cx);
        let link_menu = self.render_link_menu_popover(window, cx);
        let format_bar = self.render_format_bar(window, cx);
        self.blocks.painted.clear();
        self.blocks.drawn.clear();
        self.end_stale_drag(cx);
        self.blocks.list.sync(self.editor.selected(), rows);
        self.blocks.viewport = self.blocks.list.state.viewport_bounds();

        let rows = list(self.blocks.list.state.clone(), cx.processor(Self::render_item)).size_full();
        // The platform's text input (typing, input methods, the emoji picker) reaches the page
        // through this handler while the page has the keyboard.
        let shell = cx.entity();
        let focus = self.page_focus.clone();
        let input = canvas(
            |_, _, _| {},
            move |bounds, (), window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(bounds, shell), cx);
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        let full_width = self.full_width();
        let menu = self.render_block_menu(window, cx);
        let link = self.render_link_popover(cx);
        let language = self.render_language_picker(cx);
        let tones = self.render_tone_picker(cx);
        let task_picker = self.render_task_picker(cx);
        let page = v_flex()
            .size_full()
            // A page opened from the Trash says so above everything, and stays in view while scrolling.
            .children(
                self.render_trash_banner(cx)
                    .map(|banner| column(div().pt_4().child(banner), full_width)),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .size_full()
                    .track_focus(&self.page_focus)
                    .key_context(super::page_keys::CONTEXT)
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|shell, _: &MouseUpEvent, _, _| shell.blocks.drag = None),
                    )
                    .on_drag_move(cx.listener(Self::on_block_drag_move))
                    .on_drop(cx.listener(Self::on_block_drop))
                    .on_drag_move(cx.listener(Self::on_image_resize_move))
                    .on_drop(cx.listener(Self::on_image_resize_drop))
                    .child(input)
                    .child(Self::wheel_glide_layer(cx))
                    .child(Self::select_scroll_layer(cx))
                    .child(rows)
                    // Where you are in the page, at its right edge (E07-S14).
                    .child(crate::scrollbar::slim(&self.blocks.list.state, cx).id("page-scrollbar")),
            )
            // Outside the page's key context, so keys reach the menu and not the page while it is open.
            .child(menu)
            .children(link)
            .children(language)
            .children(tones)
            .children(task_picker)
            .children(slash)
            .children(link_menu)
            .children(format_bar);
        let page = Self::with_block_menu_action(page, cx);
        let page = Self::with_task_picker_actions(page, cx);
        Self::with_page_actions(page, cx).into_any_element()
    }

    /// Keeps the caret blinking while `showing`; stops the timer otherwise. The caret starts visible.
    fn blink_while(&mut self, showing: bool, cx: &mut Context<Self>) {
        if !showing {
            self.blocks.blink = None;
            self.blocks.caret_on = true;
            return;
        }
        if self.blocks.blink.is_none() {
            self.blocks.caret_on = true;
            self.blocks.blink = Some(cx.spawn(async move |shell, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(530))
                        .await;
                    let toggled = shell.update(cx, |shell, cx| {
                        shell.blocks.caret_on = !shell.blocks.caret_on;
                        cx.notify();
                    });
                    if toggled.is_err() {
                        break;
                    }
                }
            }));
        }
    }

    /// Shows the caret solidly for a moment after it moves or text changes, as text editors do.
    pub(super) fn restart_blink(&mut self) {
        self.blocks.blink = None;
        self.blocks.caret_on = true;
    }

    /// List item `item`: the page header (cover, icon and title) first, then one block each. The header
    /// is part of the list so it scrolls away with the page.
    fn render_item(&mut self, item: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if item < HEADER_ITEMS {
            // The cover spans the whole page area; the icon and title stay in the reading column.
            let cover = self.page_cover();
            let band = cover.as_ref().map(|cover| self.render_cover(cover, cx));
            let header = self.render_page_header(band.is_some(), cx);
            let top = if band.is_some() { px(0.0) } else { px(64.0) };
            return v_flex()
                .w_full()
                .children(band)
                .child(column(div().pt(top).pb_3().child(header), self.full_width()))
                .into_any_element();
        }
        // A table's cells are drawn in its grid, by the table's row (E16-S03): their own rows are empty.
        if self
            .blocks
            .list
            .row(item)
            .is_some_and(|row| matches!(row.style, leaf_editor::BlockStyle::TableCell { .. }))
        {
            return div().into_any_element();
        }
        let content = if let Some(row) = self.blocks.list.row(item).cloned() {
            let (left, top) = super::block_row::handle_place(&row);
            let handle = self.render_handle(row.id, left, top, cx);
            let body = self.render_row(&row, window, cx);
            let faded = self
                .blocks
                .dragging
                .as_ref()
                .is_some_and(|drag| drag.faded.contains(&row.id));
            let line = self.render_drop_line(&row, item == HEADER_ITEMS, cx);
            div()
                .relative()
                .group(super::block_menu::ROW_GROUP)
                .when(faded, |row| row.opacity(0.4))
                .child(self.blocks.drawn.recorder(row.id))
                .child(self.with_flash(row.id, body, cx))
                .child(handle)
                .children(line)
                .into_any_element()
        } else {
            div().into_any_element()
        };
        column(div().py(px(1.0)).child(content), self.full_width()).into_any_element()
    }
}
