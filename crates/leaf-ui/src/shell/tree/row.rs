//! One row of the sidebar's page tree: indent, the expand chevron, icon and title (or the rename
//! field), its context menu, and the line or ring that shows where a dragged page would land.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, Context, DragMoveEvent, Hsla, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, SharedString,
    StatefulInteractiveElement as _, Styled, WeakEntity, div, px,
};
use gpui_component::input::Input;
use gpui_component::menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem};
use gpui_component::tooltip::Tooltip;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit_assets::IconName;
use leaf_core::PageId;
use leaf_editor::{DropPlace, SidebarItem};

use super::DraggedPage;
use crate::shell::Shell;

/// Height of a row, in pixels. Fixed, so the list can draw only the rows on screen.
const ROW_HEIGHT: f32 = 30.0;

/// Extra indent per nesting level, in pixels.
const INDENT_PX: f32 = 14.0;

/// Width of the chevron (or the space it would take), in pixels.
const CHEVRON_PX: f32 = 18.0;

/// Left padding of a row at `depth`.
fn indent(depth: usize) -> Pixels {
    px(4.0 + INDENT_PX * f32::from(u16::try_from(depth).unwrap_or(u16::MAX)))
}

/// How a row shows where a dragged page would land.
enum Mark {
    /// A line above it, indented to its title.
    Above,
    /// A line below it, indented to its title.
    Below,
    /// A line below it across the whole tree: the end of the top level.
    BelowAll,
    /// A ring around it: inside its page.
    Ring,
}

impl Shell {
    /// Draws row `ix` of `count` for `item`.
    pub(super) fn render_tree_row(
        &mut self,
        ix: usize,
        item: &SidebarItem,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page = item.page;
        let theme = cx.theme();
        let (accent, hover, active, radius) = (
            theme.ring,
            theme.sidebar_accent.opacity(0.6),
            theme.sidebar_accent,
            theme.radius,
        );
        let renaming = self
            .tree
            .renaming
            .as_ref()
            .is_some_and(|rename| rename.page == page);

        let mut row = h_flex()
            .id(("page-row", ix))
            .group("page-row")
            .debug_selector(|| format!("page-row-{ix}"))
            .relative()
            .w_full()
            .h(px(ROW_HEIGHT))
            .pl(indent(item.depth))
            .pr_1()
            .gap_1()
            .rounded(radius)
            .text_sm()
            .hover(|style| style.bg(hover))
            .when(item.selected, |row| row.bg(active))
            .child(Self::chevron(ix, item, cx))
            .child(Self::page_icon(item, cx))
            .child(self.label(item, cx))
            .when(!renaming, |row| row.child(Self::add_inside(ix, page, cx)));
        let title_left = indent(item.depth) + px(CHEVRON_PX);
        row = match self.mark(page, ix + 1 == count) {
            Some(Mark::Above) => row.child(line(accent).top_0().left(title_left)),
            Some(Mark::Below) => row.child(line(accent).bottom_0().left(title_left)),
            Some(Mark::BelowAll) => row.child(line(accent).bottom_0().left_0()),
            Some(Mark::Ring) => row.border_1().border_color(accent).bg(accent.opacity(0.08)),
            None => row,
        };
        if !renaming {
            // While renaming, clicks belong to the text field.
            row = row.on_click(cx.listener(move |shell, _, window, cx| shell.open(page, window, cx)));
        }

        let dragged = DraggedPage {
            page,
            label: SharedString::from(item.label.clone()),
        };
        let shell = cx.entity().downgrade();
        row.on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
            .on_drag_move(
                cx.listener(move |shell, event: &DragMoveEvent<DraggedPage>, _, cx| {
                    let bounds = event.bounds;
                    if bounds.contains(&event.event.position) {
                        let offset = f32::from(event.event.position.y - bounds.top());
                        let place = DropPlace::at(page, offset, f32::from(bounds.size.height));
                        shell.hint_drop(event.drag(cx).page, place, cx);
                    }
                }),
            )
            .on_drop(cx.listener(|shell, dragged: &DraggedPage, _, cx| {
                if let Some(place) = shell.tree.drop_hint {
                    shell.drop_page(dragged.page, place, cx);
                }
                cx.stop_propagation(); // not also a drop on the space below the tree
            }))
            .context_menu(move |menu, _, _| page_menu(menu, &shell, page))
            .into_any_element()
    }

    /// The expand/collapse chevron, or the space it would take so titles line up.
    fn chevron(ix: usize, item: &SidebarItem, cx: &mut Context<Self>) -> AnyElement {
        let slot = div().flex_none().size(px(CHEVRON_PX));
        if !item.has_children {
            return slot.into_any_element();
        }
        let page = item.page;
        let icon = if item.expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        let (muted, active, radius) = (
            cx.theme().muted_foreground,
            cx.theme().sidebar_accent,
            cx.theme().radius,
        );
        slot.id(("chevron", ix))
            .debug_selector(|| format!("chevron-{ix}"))
            .flex()
            .items_center()
            .justify_center()
            .rounded(radius)
            .text_color(muted)
            .hover(|style| style.bg(active))
            .child(Icon::new(icon).xsmall())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation(); // a chevron click doesn't open the page or start a drag
                    shell.toggle_expanded(page, cx);
                }),
            )
            .into_any_element()
    }

    /// The page's emoji, or a quiet page icon when it has none, so every title starts at the same place.
    fn page_icon(item: &SidebarItem, cx: &mut Context<Self>) -> AnyElement {
        let slot = div().flex_none().w(px(18.0)).flex().justify_center();
        match &item.icon {
            Some(emoji) => slot.child(emoji.clone()).into_any_element(),
            None => slot
                .text_color(cx.theme().muted_foreground)
                .child(Icon::new(IconName::FileText).small())
                .into_any_element(),
        }
    }

    /// A "+" at the row's right end, shown while the pointer is over the row, that adds a page inside.
    fn add_inside(ix: usize, page: PageId, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (muted, active, radius) = (theme.muted_foreground, theme.sidebar_accent, theme.radius);
        div()
            .id(("add-inside", ix))
            .debug_selector(|| format!("add-inside-{ix}"))
            .flex_none()
            .size(px(20.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(radius)
            .text_color(muted)
            .opacity(0.0)
            .group_hover("page-row", |style| style.opacity(1.0))
            .hover(|style| style.bg(active))
            .tooltip(|window, cx| Tooltip::new("Add a page inside").build(window, cx))
            .child(Icon::new(IconName::Plus).xsmall())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation(); // not also a click that opens this row's page
                    shell.new_page_inside(page, window, cx);
                }),
            )
            .into_any_element()
    }

    /// The row's title, or the rename field while it is being renamed.
    fn label(&self, item: &SidebarItem, cx: &mut Context<Self>) -> AnyElement {
        let cell = div().flex_1().min_w_0();
        let input = self
            .tree
            .renaming
            .as_ref()
            .filter(|rename| rename.page == item.page)
            .map(|rename| rename.input.clone());
        match input {
            Some(input) => cell
                // Escape keeps the old title; the field would otherwise only clear its selection.
                .capture_key_down(cx.listener(|shell, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        shell.finish_rename(false, window, cx);
                    }
                }))
                .child(Input::new(&input).small())
                .into_any_element(),
            None => cell.truncate().child(item.label.clone()).into_any_element(),
        }
    }

    /// How `page`'s row shows the current drop hint, if it concerns this row. `last` says whether
    /// this is the bottom row, which shows drops on the space below the tree.
    fn mark(&self, page: PageId, last: bool) -> Option<Mark> {
        match self.tree.drop_hint? {
            DropPlace::Before(target) if target == page => Some(Mark::Above),
            DropPlace::After(target) if target == page => Some(Mark::Below),
            DropPlace::Inside(target) if target == page => Some(Mark::Ring),
            DropPlace::End if last => Some(Mark::BelowAll),
            _ => None,
        }
    }
}

/// The row's right-click menu.
fn page_menu(menu: PopupMenu, shell: &WeakEntity<Shell>, page: PageId) -> PopupMenu {
    let (inside, rename, trash) = (shell.clone(), shell.clone(), shell.clone());
    // The shell outlives its rows; if it is gone the window is closing anyway.
    menu.item(
        PopupMenuItem::new("Add page inside")
            .icon(IconName::Plus)
            .on_click(move |_, window, cx| {
                let _ = inside.update(cx, |shell, cx| shell.new_page_inside(page, window, cx));
            }),
    )
    .item(
        PopupMenuItem::new("Rename")
            .icon(IconName::PencilLine)
            .on_click(move |_, window, cx| {
                let _ = rename.update(cx, |shell, cx| shell.start_rename(page, window, cx));
            }),
    )
    .separator()
    .item(
        PopupMenuItem::new("Move to Trash")
            .icon(IconName::Trash)
            .on_click(move |_, _, cx| {
                let _ = trash.update(cx, |shell, cx| shell.trash_page(page, cx));
            }),
    )
}

/// A 2 px accent line across a row, placed by the caller.
fn line(color: Hsla) -> gpui::Div {
    div().absolute().right_0().h(px(2.0)).rounded_full().bg(color)
}
