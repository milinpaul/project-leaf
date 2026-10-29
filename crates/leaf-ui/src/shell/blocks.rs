//! The open page: a "Turn into" toolbar, then the title and blocks as a virtual list drawn by the rich
//! text view. The page takes the keyboard itself (E07-S03): keys become editor actions
//! (`page_keys`), typed text and input-method drafts arrive through the platform's text input
//! (`ime`), and the mouse places the caret and selects (`rich_text`). Every decision is
//! `PageEditor`'s; this module only forwards and draws.

use std::collections::HashMap;

use gpui::{
    AnyElement, ClickEvent, Context, ElementInputHandler, InteractiveElement as _, IntoElement, MouseButton,
    MouseUpEvent, ParentElement, Pixels, Point, SharedString, Styled, Task, Window, canvas, div, list, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{h_flex, v_flex};
use leaf_core::BlockId;
use leaf_editor::{PageEditor, Preedit};
use leaf_model::{BlockKind, Caret};

use super::column;
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
    /// Where the block menu's top-left corner goes, set when it opens.
    pub(super) menu_at: Point<Pixels>,
    /// The block whose menu is open as of this frame, so its handle stays shown.
    pub(super) menu_block: Option<BlockId>,
}

/// The "Turn into" toolbar: label and the kind each button applies.
fn toolbar_kinds() -> [(&'static str, BlockKind); 9] {
    [
        ("Text", BlockKind::Paragraph),
        ("H1", BlockKind::Heading { level: 1 }),
        ("H2", BlockKind::Heading { level: 2 }),
        ("H3", BlockKind::Heading { level: 3 }),
        ("• List", BlockKind::BulletedList),
        ("1. List", BlockKind::NumberedList),
        ("To-do", BlockKind::Todo { checked: false }),
        ("Quote", BlockKind::Quote),
        (
            "Code",
            BlockKind::Code {
                language: String::new(),
            },
        ),
    ]
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
    pub(super) fn reveal_caret(&mut self) {
        if let Some(block) = self.caret_block() {
            self.blocks.list.reveal(block);
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
        self.blocks.painted.clear();
        self.blocks.list.sync(self.editor.selected(), rows);

        let toolbar = h_flex()
            .gap_1()
            .children(toolbar_kinds().into_iter().map(|(label, kind)| {
                Button::new(SharedString::from(format!("turn-into-{label}")))
                    .label(label)
                    .ghost()
                    .on_click(cx.listener(move |shell, _: &ClickEvent, window, cx| {
                        shell.turn_focused_into(kind.clone(), window, cx);
                    }))
            }));
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

        let menu = self.render_block_menu(cx);
        let page = v_flex()
            .size_full()
            // A page opened from the Trash says so above everything, and stays in view while scrolling.
            .children(
                self.render_trash_banner(cx)
                    .map(|banner| column(div().pt_4().child(banner))),
            )
            .child(column(div().pt_2().pb_1().child(toolbar)))
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
                    .child(input)
                    .child(rows),
            )
            // Outside the page's key context, so keys reach the menu and not the page while it is open.
            .children(menu)
            .children(slash);
        let page = Self::with_block_menu_action(page, cx);
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
                .child(column(div().pt(top).pb_3().child(header)))
                .into_any_element();
        }
        let content = if let Some(row) = self.blocks.list.row(item).cloned() {
            let (left, top) = super::block_row::handle_place(&row);
            let handle = self.render_handle(row.id, left, top, cx);
            div()
                .relative()
                .group(super::block_menu::ROW_GROUP)
                .child(self.render_row(&row, window, cx))
                .child(handle)
                .into_any_element()
        } else {
            div().into_any_element()
        };
        column(div().py(px(1.0)).child(content)).into_any_element()
    }
}
