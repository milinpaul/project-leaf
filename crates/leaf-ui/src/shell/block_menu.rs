//! The ⋮⋮ handle beside each block and the menu it opens (E07-S04, ADR-041). The menu's state and
//! every decision live in `leaf_editor::block_menu`; this module draws it, gives it the keyboard while
//! it is open, and forwards keys, hovers and clicks.

use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Bounds, ClickEvent, Context, InteractiveElement, IntoElement,
    KeyBinding, MouseButton, MouseDownEvent, ParentElement, Pixels, Point, SharedString,
    StatefulInteractiveElement as _, Styled, Window, actions, anchored, deferred, div, point, px, size,
};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_core::BlockId;
use leaf_editor::{BlockMenu, HoverChange, MenuEntry, MenuItem, MenuKey};
use leaf_model::BlockKind;

use super::block_drag::{DragPreview, DraggedBlock};
use super::{Shell, menu_place, report};

/// The menu's key context, active while it has the keyboard.
const CONTEXT: &str = "LeafBlockMenu";

/// The group every block row belongs to, so its handle shows while the pointer is over the row.
pub(super) const ROW_GROUP: &str = "leaf-block-row";

/// How long the pointer rests on "Turn into" before its list opens: long enough that passing over it
/// on the way down the menu doesn't flash the list open.
const OPEN_DELAY: Duration = Duration::from_millis(150);

/// How long the pointer rests on another entry before an open "Turn into" list closes: long enough to
/// cross a row or two diagonally on the way into the list (E17-S03).
const CLOSE_DELAY: Duration = Duration::from_millis(300);

/// Both lists' width, and one entry's height plus the list's padding and border: the lists' sizes are
/// known before they are drawn, so they can be placed to fit (E17-S07).
const MENU_WIDTH_PX: f32 = 220.0;
const ROW_PX: f32 = 30.0;
const CHROME_PX: f32 = 10.0;

/// How tall a list of `rows` entries is.
fn list_height(rows: usize) -> Pixels {
    px(ROW_PX) * row_offset(rows) + px(CHROME_PX)
}

/// `rows` as a multiplier for pixel sizes.
fn row_offset(rows: usize) -> f32 {
    f32::from(u16::try_from(rows).unwrap_or(u16::MAX))
}

/// The handle's square, and how far left of the row's text it sits.
const HANDLE_PX: f32 = 20.0;
const HANDLE_GAP_PX: f32 = 4.0;

actions!(
    leaf_block_menu,
    [
        /// Open the block menu for the caret's block (in the page).
        OpenBlockMenu,
        /// Highlight the entry above.
        MenuUp,
        /// Highlight the entry below.
        MenuDown,
        /// Back from the "Turn into" list.
        MenuLeft,
        /// Into the "Turn into" list.
        MenuRight,
        /// Choose the highlighted entry.
        MenuEnter,
        /// Back, or close the menu.
        MenuEscape,
    ]
);

/// Binds the menu's keys, and `Cmd/Ctrl+/` in the page to open it. Called once at startup.
pub(super) fn bind(cx: &mut App, page_context: &'static str, open: &'static str) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new(open, OpenBlockMenu, Some(page_context)),
        KeyBinding::new("up", MenuUp, c),
        KeyBinding::new("down", MenuDown, c),
        KeyBinding::new("left", MenuLeft, c),
        KeyBinding::new("right", MenuRight, c),
        KeyBinding::new("enter", MenuEnter, c),
        KeyBinding::new("escape", MenuEscape, c),
    ]);
}

/// The icon drawn beside a menu entry.
pub(super) fn icon_of(entry: &MenuEntry) -> IconName {
    match entry {
        MenuEntry::TurnInto => IconName::Repeat,
        MenuEntry::Duplicate => IconName::Copy,
        MenuEntry::MoveUp => IconName::ArrowUp,
        MenuEntry::MoveDown => IconName::ArrowDown,
        MenuEntry::Delete => IconName::Trash,
        MenuEntry::Kind(kind) => kind_icon(kind),
        MenuEntry::ToLink => IconName::Link,
        MenuEntry::ToWebBookmark(_) => IconName::Bookmark,
    }
}

/// The icon for a block kind, in the block menu's "Turn into" list and the slash menu.
pub(super) fn kind_icon(kind: &BlockKind) -> IconName {
    match kind {
        BlockKind::Heading { level: 1 } => IconName::Heading1,
        BlockKind::Heading { level: 2 } => IconName::Heading2,
        BlockKind::Heading { .. } => IconName::Heading3,
        BlockKind::BulletedList => IconName::List,
        BlockKind::NumberedList => IconName::ListOrdered,
        BlockKind::Todo { .. } => IconName::SquareCheck,
        BlockKind::Task { .. } => IconName::CircleCheck,
        BlockKind::Quote => IconName::TextQuote,
        BlockKind::Callout { tone } => super::callout::icon_of(tone),
        BlockKind::Code { .. } => IconName::SquareCode,
        BlockKind::Divider => IconName::Minus,
        BlockKind::WebBookmark { .. } => IconName::Bookmark,
        _ => IconName::Type,
    }
}

impl Shell {
    /// The open block menu, as the editor has it.
    pub(super) fn block_menu(&mut self) -> Option<BlockMenu> {
        self.page().and_then(|page| page.block_menu().ok().flatten())
    }

    /// Opens the menu for `block` with its top-left corner at `at`, and gives it the keyboard.
    pub fn open_block_menu(
        &mut self,
        block: BlockId,
        at: Point<Pixels>,
        top: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(Err(err)) = self.page().map(|page| page.open_block_menu(block)) {
            report("could not open the block menu", err);
        }
        self.blocks.menu_at = at;
        self.blocks.menu_top = top;
        window.focus(&self.menu_focus, cx);
        cx.notify();
    }

    /// `Cmd/Ctrl+/`: the menu for the caret's block.
    fn on_open_block_menu(&mut self, _: &OpenBlockMenu, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(block) = self.caret_block() {
            self.open_block_menu_beside(block, window, cx);
        }
    }

    /// Opens the menu for `block` under its handle, as drawn in the last frame. Also used by the
    /// screenshot tool.
    pub fn open_block_menu_beside(&mut self, block: BlockId, window: &mut Window, cx: &mut Context<Self>) {
        let (at, top) = self.blocks.painted.caret_bounds(block, 0).map_or_else(
            || (point(px(0.0), px(0.0)), px(0.0)),
            |(origin, height)| {
                (
                    point(origin.x - px(HANDLE_PX + HANDLE_GAP_PX), origin.y + height),
                    origin.y,
                )
            },
        );
        self.open_block_menu(block, at, top, window, cx);
    }

    /// A key while the menu is open; the page gets the keyboard back once the menu closes.
    fn menu_key(&mut self, key: MenuKey, window: &mut Window, cx: &mut Context<Self>) {
        let after = self.page().map(|page| page.block_menu_key(key));
        self.menu_done(after, window, cx);
    }

    /// The pointer over an entry: highlights it, and opens or closes the "Turn into" list once the
    /// pointer has rested for a moment (the editor decides which, from where it is by then).
    fn menu_hover(&mut self, entry: &MenuEntry, cx: &mut Context<Self>) {
        let change = match self.page().map(|page| page.highlight(entry)) {
            Some(Ok(change)) => change,
            Some(Err(err)) => return report("could not highlight the block menu entry", err),
            None => None,
        };
        if let Some(change) = change {
            let delay = match change {
                HoverChange::OpenKinds => OPEN_DELAY,
                HoverChange::CloseKinds => CLOSE_DELAY,
            };
            self.blocks.menu_hover = Some(cx.spawn(async move |shell, cx| {
                cx.background_executor().timer(delay).await;
                let _ = shell.update(cx, |shell, cx| {
                    if let Some(Err(err)) = shell.page().map(leaf_editor::PageEditor::settle_hover) {
                        report("could not update the block menu", err);
                    }
                    cx.notify();
                });
            }));
        }
        cx.notify();
    }

    /// A click on an entry.
    fn menu_choose(&mut self, entry: MenuEntry, window: &mut Window, cx: &mut Context<Self>) {
        let after = self.page().map(|page| page.choose(entry));
        self.menu_done(after, window, cx);
    }

    fn menu_done(
        &mut self,
        after: Option<Result<Option<BlockMenu>, leaf_editor::EditorError>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match after {
            Some(Ok(Some(_))) => cx.notify(),
            Some(Err(err)) => {
                report("could not apply the block menu", err);
                self.focus_page(window, cx);
            }
            _ => self.focus_page(window, cx),
        }
    }

    /// A click outside the menu closes it, leaving the keyboard wherever the click put it.
    fn menu_dismiss(&mut self, cx: &mut Context<Self>) {
        if let Some(page) = self.page() {
            page.close_block_menu();
        }
        cx.notify();
    }

    /// Attaches `Cmd/Ctrl+/` to the page element.
    pub(super) fn with_block_menu_action<E: InteractiveElement>(element: E, cx: &mut Context<Self>) -> E {
        element.on_action(cx.listener(Self::on_open_block_menu))
    }

    /// The ⋮⋮ handle for `block`, `top` px below the row's top and just left of its text at `left`.
    /// It shows while the pointer is over the row, or while this block's menu is open.
    pub(super) fn render_handle(
        &mut self,
        block: BlockId,
        left: Pixels,
        top: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.blocks.menu_block == Some(block);
        let shell = cx.entity().downgrade();
        let theme = cx.theme();
        let (muted, hover) = (theme.muted_foreground, theme.muted);
        div()
            .id(SharedString::from(format!("block-handle-{block}")))
            .debug_selector(move || format!("block-handle-{block}"))
            .absolute()
            .left(left - px(HANDLE_PX + HANDLE_GAP_PX))
            .top(top)
            .size(px(HANDLE_PX))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .cursor_pointer()
            .opacity(if open { 1.0 } else { 0.0 })
            .group_hover(ROW_GROUP, |s| s.opacity(1.0))
            .hover(|s| s.bg(hover))
            .when(open, |s| s.bg(hover))
            .child(Icon::new(IconName::GripVertical).small().text_color(muted))
            // A click opens the menu; pressing and moving drags the block instead (E07-S05).
            .on_click(cx.listener(move |shell, event: &ClickEvent, window, cx| {
                let at = event.position();
                let top = at.y - px(HANDLE_PX / 2.0);
                let at = point(at.x - px(HANDLE_PX / 2.0), at.y + px(HANDLE_PX / 2.0));
                shell.open_block_menu(block, at, top, window, cx);
            }))
            .on_drag(DraggedBlock(block), move |dragged, _, _, cx| {
                let preview = shell.update(cx, |shell, cx| shell.start_block_drag(dragged.0, cx));
                cx.new(|_| preview.unwrap_or_else(|_| DragPreview::empty()))
            })
            .into_any_element()
    }

    /// The open menu, floating over the page, and the "Turn into" list beside it while that is open.
    /// Both are placed before drawing (`menu_place`), so the menu stays where it opened whatever the
    /// list does (E17-S07).
    pub(super) fn render_block_menu(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let menu = self.block_menu();
        self.blocks.menu_block = menu.as_ref().map(|m| m.block);
        self.blocks.menu_list = None;
        let Some(menu) = menu else {
            return div().into_any_element();
        };
        let viewport = window.viewport_size();
        let main_size = size(px(MENU_WIDTH_PX), list_height(menu.items.len()));
        let origin = menu_place::menu_origin(
            self.blocks.menu_at.x,
            self.blocks.menu_top,
            self.blocks.menu_at.y,
            main_size,
            viewport,
        );
        let main = Self::render_menu_list("block-menu", &menu.items, menu.selected, true, None, cx);
        let panel = div()
            .w(px(MENU_WIDTH_PX))
            .track_focus(&self.menu_focus)
            .key_context(CONTEXT)
            .on_action(cx.listener(|s, _: &MenuUp, w, cx| s.menu_key(MenuKey::Up, w, cx)))
            .on_action(cx.listener(|s, _: &MenuDown, w, cx| s.menu_key(MenuKey::Down, w, cx)))
            .on_action(cx.listener(|s, _: &MenuLeft, w, cx| s.menu_key(MenuKey::Left, w, cx)))
            .on_action(cx.listener(|s, _: &MenuRight, w, cx| s.menu_key(MenuKey::Right, w, cx)))
            .on_action(cx.listener(|s, _: &MenuEnter, w, cx| s.menu_key(MenuKey::Enter, w, cx)))
            .on_action(cx.listener(|s, _: &MenuEscape, w, cx| s.menu_key(MenuKey::Escape, w, cx)))
            .on_mouse_down_out(cx.listener(|shell, event: &MouseDownEvent, _, cx| {
                // The list floats apart from the menu; a click in it isn't a click outside.
                if !shell
                    .blocks
                    .menu_list
                    .is_some_and(|list| list.contains(&event.position))
                {
                    shell.menu_dismiss(cx);
                }
            }))
            .child(main);
        let kinds = menu.kinds.as_ref().map(|(kinds, at)| {
            let row = menu
                .items
                .iter()
                .position(|item| item.entry == MenuEntry::TurnInto)
                .unwrap_or(0);
            let row_top = origin.y + px(CHROME_PX / 2.0) + px(ROW_PX) * row_offset(row);
            let menu_bounds = Bounds::new(origin, main_size);
            let wanted = size(px(MENU_WIDTH_PX), list_height(kinds.len()));
            let bounds = menu_place::list_bounds(menu_bounds, row_top, wanted, viewport);
            self.blocks.menu_list = Some(bounds);
            let list = Self::render_menu_list(
                "block-menu-kinds",
                kinds,
                *at,
                false,
                Some(bounds.size.height),
                cx,
            );
            deferred(anchored().position(bounds.origin).child(list)).with_priority(2)
        });
        div()
            .child(deferred(anchored().position(origin).child(panel)).with_priority(1))
            .children(kinds)
            .into_any_element()
    }

    /// One list of entries with icons; the highlighted one shaded, greyed-out ones dimmed.
    fn render_menu_list(
        id: &'static str,
        items: &[MenuItem],
        selected: usize,
        main: bool,
        max_height: Option<Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let (bg, border, accent, muted) = (theme.popover, theme.border, theme.accent, theme.muted_foreground);
        let danger = theme.danger;
        let radius = theme.radius;
        v_flex()
            .id(id)
            .debug_selector(move || id.into())
            .w(px(MENU_WIDTH_PX))
            .when_some(max_height, |list, height| list.max_h(height).overflow_y_scroll())
            .p_1()
            .bg(bg)
            .border_1()
            .border_color(border)
            .rounded(radius)
            .shadow_lg()
            .text_sm()
            .children(items.iter().enumerate().map(|(ix, item)| {
                let entry = item.entry.clone();
                let hovered = entry.clone();
                let colour = if !item.enabled {
                    muted.opacity(0.5)
                } else if entry == MenuEntry::Delete {
                    danger
                } else {
                    theme.foreground
                };
                h_flex()
                    .id((id, ix))
                    .debug_selector(move || format!("{id}-{ix}"))
                    .h(px(30.0))
                    .px_2()
                    .gap_2()
                    .rounded(radius)
                    .text_color(colour)
                    .when(ix == selected && item.enabled, |row| row.bg(accent))
                    .when(item.enabled, gpui::Styled::cursor_pointer)
                    .child(Icon::new(icon_of(&entry)).small().text_color(if item.enabled {
                        colour
                    } else {
                        muted.opacity(0.5)
                    }))
                    .child(div().flex_1().child(entry.label()))
                    .when(main && entry == MenuEntry::TurnInto, |row| {
                        row.child(Icon::new(IconName::ChevronRight).small().text_color(muted))
                    })
                    .when(item.current, |row| {
                        row.child(Icon::new(IconName::Check).small().text_color(muted))
                    })
                    .on_mouse_move(cx.listener(move |shell, _, _, cx| shell.menu_hover(&hovered, cx)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |shell, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            shell.menu_choose(entry.clone(), window, cx);
                        }),
                    )
                    .into_any_element()
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
