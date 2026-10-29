//! The ⋮⋮ handle beside each block and the menu it opens (E07-S04, ADR 0041). The menu's state and
//! every decision live in `leaf_editor::block_menu`; this module draws it, gives it the keyboard while
//! it is open, and forwards keys, hovers and clicks.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement, IntoElement, KeyBinding, MouseButton, MouseDownEvent,
    ParentElement, Pixels, Point, SharedString, Styled, Window, actions, anchored, deferred, div, point, px,
};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_core::BlockId;
use leaf_editor::{BlockMenu, MenuEntry, MenuItem, MenuKey};
use leaf_model::BlockKind;

use super::{Shell, report};

/// The menu's key context, active while it has the keyboard.
const CONTEXT: &str = "LeafBlockMenu";

/// The group every block row belongs to, so its handle shows while the pointer is over the row.
pub(super) const ROW_GROUP: &str = "leaf-block-row";

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
        BlockKind::Quote => IconName::TextQuote,
        BlockKind::Code { .. } => IconName::SquareCode,
        BlockKind::Divider => IconName::Minus,
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(Err(err)) = self.page().map(|page| page.open_block_menu(block)) {
            report("could not open the block menu", err);
        }
        self.blocks.menu_at = at;
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
        let at = self.blocks.painted.caret_bounds(block, 0).map_or_else(
            || point(px(0.0), px(0.0)),
            |(origin, height)| point(origin.x - px(HANDLE_PX + HANDLE_GAP_PX), origin.y + height),
        );
        self.open_block_menu(block, at, window, cx);
    }

    /// A key while the menu is open; the page gets the keyboard back once the menu closes.
    fn menu_key(&mut self, key: MenuKey, window: &mut Window, cx: &mut Context<Self>) {
        let after = self.page().map(|page| page.block_menu_key(key));
        self.menu_done(after, window, cx);
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
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    let at = point(
                        event.position.x - px(HANDLE_PX / 2.0),
                        event.position.y + px(HANDLE_PX / 2.0),
                    );
                    shell.open_block_menu(block, at, window, cx);
                }),
            )
            .into_any_element()
    }

    /// The open menu, floating over the page: the main entries, and the "Turn into" list beside them
    /// while it is open.
    pub(super) fn render_block_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.block_menu();
        self.blocks.menu_block = menu.as_ref().map(|m| m.block);
        let menu = menu?;
        let main = Self::render_menu_list("block-menu", &menu.items, menu.selected, true, cx);
        let kinds = menu
            .kinds
            .as_ref()
            .map(|(kinds, at)| Self::render_menu_list("block-menu-kinds", kinds, *at, false, cx));
        let panel = h_flex()
            .items_start()
            .gap_1()
            .track_focus(&self.menu_focus)
            .key_context(CONTEXT)
            .on_action(cx.listener(|s, _: &MenuUp, w, cx| s.menu_key(MenuKey::Up, w, cx)))
            .on_action(cx.listener(|s, _: &MenuDown, w, cx| s.menu_key(MenuKey::Down, w, cx)))
            .on_action(cx.listener(|s, _: &MenuLeft, w, cx| s.menu_key(MenuKey::Left, w, cx)))
            .on_action(cx.listener(|s, _: &MenuRight, w, cx| s.menu_key(MenuKey::Right, w, cx)))
            .on_action(cx.listener(|s, _: &MenuEnter, w, cx| s.menu_key(MenuKey::Enter, w, cx)))
            .on_action(cx.listener(|s, _: &MenuEscape, w, cx| s.menu_key(MenuKey::Escape, w, cx)))
            .on_mouse_down_out(cx.listener(|shell, _: &MouseDownEvent, _, cx| shell.menu_dismiss(cx)))
            .child(main)
            .children(kinds);
        Some(
            deferred(
                anchored()
                    .position(self.blocks.menu_at)
                    .snap_to_window_with_margin(px(8.0))
                    .child(panel),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    /// One list of entries with icons; the highlighted one shaded, greyed-out ones dimmed.
    fn render_menu_list(
        id: &'static str,
        items: &[MenuItem],
        selected: usize,
        main: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let (bg, border, accent, muted) = (theme.popover, theme.border, theme.accent, theme.muted_foreground);
        let danger = theme.danger;
        let radius = theme.radius;
        v_flex()
            .id(id)
            .debug_selector(move || id.into())
            .min_w(px(200.0))
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
                    .on_mouse_move(cx.listener(move |shell, _, _, cx| {
                        if let Some(page) = shell.page() {
                            let _ = page.highlight(&hovered);
                        }
                        cx.notify();
                    }))
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
