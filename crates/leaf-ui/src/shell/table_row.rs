//! A table drawn in the page (E16-S03): a bordered grid of editable cells with a bold header row,
//! controls to add a row or column, and a menu on each row and column.
//!
//! The editor lists a table's cells right after it (ADR 0063). The table's own row draws the whole
//! grid; the cell rows draw nothing of their own (see `blocks.rs`), so the virtual list stays in step
//! with the editor's rows. Each cell's text is drawn by the usual rich-text renderer, so the caret,
//! selection, marks, clicks, drags and input methods work in cells as in any block. Every decision is
//! `PageEditor`'s; the controls here only forward to [`leaf_editor::PageEditor::edit_table`].
//!
//! Columns have a fixed width, and the grid sits in a box that scrolls sideways, so a wide table never
//! makes the page wider than its column.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    ScrollEvent, SharedString, StatefulInteractiveElement as _, Styled, Window, div, point, px,
};
use gpui_component::button::ButtonVariants as _;
use gpui_component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_core::BlockId;
use leaf_editor::{BlockRow, BlockStyle, TableEdit};

use super::rich_text::{Highlight, Look};
use super::{Shell, report};

/// Width of every column: room for a few words, and a predictable grid to scroll sideways.
pub(super) const CELL_WIDTH: f32 = 160.0;
/// The slim strip before each row and above each column that holds its menu handle.
const HANDLE: f32 = 18.0;
/// Thickness of the "+" bars that add a row (below) or a column (right).
const ADD_BAR: f32 = 18.0;

impl Shell {
    /// The grid of `table` (`row` is its view row), with its controls.
    pub(super) fn render_table(
        &mut self,
        row: &BlockRow,
        header_row: bool,
        rows: usize,
        columns: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let table = row.id;
        let cells = self.cells_of(table);
        let border = cx.theme().border;
        let group = SharedString::from(format!("table-{table}"));

        // A row of column handles, then each row: its handle and its cells.
        let column_handles = h_flex()
            .child(div().w(px(HANDLE)).flex_none())
            .children((0..columns).map(|column| {
                div()
                    .w(px(CELL_WIDTH))
                    .flex_none()
                    .flex()
                    .justify_center()
                    .child(Self::column_handle(table, column, columns, &group, cx))
            }));
        let mut grid = v_flex().child(column_handles);
        for r in 0..rows {
            let row_group = SharedString::from(format!("table-row-{table}-{r}"));
            let line = h_flex()
                .items_stretch()
                .group(row_group.clone())
                .child(
                    div()
                        .w(px(HANDLE))
                        .flex_none()
                        .flex()
                        .items_center()
                        .child(Self::row_handle(table, r, rows, header_row, &row_group, cx)),
                )
                .children(
                    cells
                        .iter()
                        .filter(|cell| matches!(cell.style, BlockStyle::TableCell { row, .. } if row == r))
                        .map(|cell| self.render_cell(cell, header_row && r == 0, r == 0, window, cx)),
                );
            grid = grid.child(line);
        }

        let add_row = div()
            .id(SharedString::from(format!("table-add-row-{table}")))
            .debug_selector(move || format!("table-add-row-{table}"))
            .ml(px(HANDLE))
            .w(px(
                CELL_WIDTH * f32::from(u16::try_from(columns).unwrap_or(u16::MAX))
            ))
            .h(px(ADD_BAR))
            .flex()
            .justify_center()
            .items_center()
            .rounded_b_md()
            .cursor_pointer()
            .opacity(0.0)
            .group_hover(group.clone(), |style| style.opacity(1.0))
            .hover(|style| style.bg(cx.theme().muted))
            .child(
                Icon::new(IconName::Plus)
                    .xsmall()
                    .text_color(cx.theme().muted_foreground),
            )
            .on_click(cx.listener(move |shell, _: &ClickEvent, window, cx| {
                shell.table_edit(table, TableEdit::InsertRow(rows), window, cx);
            }));
        let add_column = div()
            .id(SharedString::from(format!("table-add-column-{table}")))
            .debug_selector(move || format!("table-add-column-{table}"))
            .mt(px(HANDLE))
            .w(px(ADD_BAR))
            .flex()
            .justify_center()
            .items_center()
            .rounded_r_md()
            .cursor_pointer()
            .opacity(0.0)
            .group_hover(group.clone(), |style| style.opacity(1.0))
            .hover(|style| style.bg(cx.theme().muted))
            .child(
                Icon::new(IconName::Plus)
                    .xsmall()
                    .text_color(cx.theme().muted_foreground),
            )
            .on_click(cx.listener(move |shell, _: &ClickEvent, window, cx| {
                shell.table_edit(table, TableEdit::InsertColumn(columns), window, cx);
            }));

        // Never squeezed to the box's width: the grid keeps its full width, which is what scrolls.
        let body = h_flex()
            .flex_none()
            .items_stretch()
            .child(v_flex().child(grid).child(add_row))
            .child(add_column);
        // The page's column is the limit: anything wider scrolls inside this box. Only sideways
        // movement scrolls it (a horizontal swipe, or Shift+wheel); scrolling up and down over it
        // moves the page.
        let scroll = self.blocks.table_scroll.entry(table).or_default().clone();
        div()
            .id(SharedString::from(format!("table-scroll-{table}")))
            .debug_selector(move || format!("table-{table}"))
            .group(group)
            .w_full()
            .flex()
            .ml(super::block_row::indent_of(row) - px(HANDLE))
            .track_scroll(&scroll)
            .overflow_x_scroll()
            .restrict_scroll_to_axis()
            .on_scroll(move |event: &ScrollEvent, window, cx| {
                if !event.modifiers.shift {
                    return;
                }
                let delta = event.delta.pixel_delta(window.line_height());
                let sideways = if delta.x == px(0.0) { delta.y } else { delta.x };
                let offset = scroll.offset();
                let x = (offset.x + sideways).clamp(-scroll.max_offset().x, px(0.0));
                if x != offset.x {
                    scroll.set_offset(point(x, offset.y));
                    cx.stop_propagation();
                    window.refresh();
                }
            })
            .border_color(border)
            .child(body)
            .into_any_element()
    }

    /// The view rows of `table`'s cells, as the editor listed them right after it.
    fn cells_of(&self, table: BlockId) -> Vec<BlockRow> {
        let rows = &self.blocks.list.rows;
        let Some(at) = rows.iter().position(|row| row.id == table) else {
            return Vec::new();
        };
        rows[at + 1..]
            .iter()
            .take_while(|row| matches!(row.style, BlockStyle::TableCell { .. }))
            .cloned()
            .collect()
    }

    /// One cell: its text, editable like any block's, in a bordered box.
    fn render_cell(
        &mut self,
        cell: &BlockRow,
        header: bool,
        first_row: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = cell.id;
        let look = Look {
            size: None,
            line_height: px(24.0),
            pad: px(2.0),
            weight: if header {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            },
            mono: false,
            done: false,
        };
        let highlight = self.blocks.highlights.get(&id).cloned();
        let whole = highlight == Some(Highlight::Whole);
        let caret = self.blocks.caret.filter(|c| c.block == id).map(|c| c.at);
        let text = self.render_rich_text(cell, &look, highlight.as_ref(), caret, window, cx);
        let theme = cx.theme();
        let (border, header_tint, selected) = (theme.border, theme.muted, theme.selection.opacity(0.35));
        let BlockStyle::TableCell { column, .. } = cell.style else {
            return div().into_any_element();
        };
        div()
            .debug_selector(move || format!("table-cell-{id}"))
            .w(px(CELL_WIDTH))
            .flex_none()
            .flex()
            .px_2()
            .border_r_1()
            .border_b_1()
            .border_color(border)
            .when(first_row, |cell| cell.border_t_1())
            .when(column == 0, |cell| cell.border_l_1())
            .when(header, |cell| cell.bg(header_tint))
            .when(whole, |cell| cell.bg(selected))
            .child(text)
            .into_any_element()
    }

    /// The handle above column `column`: its menu inserts, moves and deletes columns.
    fn column_handle(
        table: BlockId,
        column: usize,
        columns: usize,
        group: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let shell = cx.entity().downgrade();
        crate::theme::button(SharedString::from(format!("table-column-{table}-{column}")))
            .debug_selector(move || format!("table-column-{table}-{column}"))
            .icon(Icon::new(IconName::Ellipsis))
            .ghost()
            .xsmall()
            .h(px(HANDLE - 4.0))
            .opacity(0.0)
            .group_hover(group.clone(), |style| style.opacity(1.0))
            .dropdown_menu(move |menu, _, _| {
                let item = |label: &'static str, edit: TableEdit, enabled: bool| {
                    let shell = shell.clone();
                    PopupMenuItem::new(label)
                        .disabled(!enabled)
                        .on_click(move |_, window, cx| {
                            let _ = shell.update(cx, |shell, cx| shell.table_edit(table, edit, window, cx));
                        })
                };
                menu_with(
                    menu,
                    [
                        item("Insert left", TableEdit::InsertColumn(column), true),
                        item("Insert right", TableEdit::InsertColumn(column + 1), true),
                        item(
                            "Move left",
                            TableEdit::MoveColumn {
                                from: column,
                                to: column.saturating_sub(1),
                            },
                            column > 0,
                        ),
                        item(
                            "Move right",
                            TableEdit::MoveColumn {
                                from: column,
                                to: column + 1,
                            },
                            column + 1 < columns,
                        ),
                        item("Delete column", TableEdit::DeleteColumn(column), columns > 1),
                    ],
                )
            })
            .into_any_element()
    }

    /// The handle before row `row`: its menu inserts, moves and deletes rows, and on the first row
    /// turns the header row on or off.
    fn row_handle(
        table: BlockId,
        row: usize,
        rows: usize,
        header_row: bool,
        group: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let shell = cx.entity().downgrade();
        crate::theme::button(SharedString::from(format!("table-row-{table}-{row}")))
            .debug_selector(move || format!("table-row-{table}-{row}"))
            .icon(Icon::new(IconName::GripVertical))
            .ghost()
            .xsmall()
            .w(px(HANDLE - 2.0))
            .opacity(0.0)
            .group_hover(group.clone(), |style| style.opacity(1.0))
            .dropdown_menu(move |menu, _, _| {
                let item = |label: &'static str, edit: TableEdit, enabled: bool| {
                    let shell = shell.clone();
                    PopupMenuItem::new(label)
                        .disabled(!enabled)
                        .on_click(move |_, window, cx| {
                            let _ = shell.update(cx, |shell, cx| shell.table_edit(table, edit, window, cx));
                        })
                };
                let mut menu = menu_with(
                    menu,
                    [
                        item("Insert above", TableEdit::InsertRow(row), true),
                        item("Insert below", TableEdit::InsertRow(row + 1), true),
                        item(
                            "Move up",
                            TableEdit::MoveRow {
                                from: row,
                                to: row.saturating_sub(1),
                            },
                            row > 0,
                        ),
                        item(
                            "Move down",
                            TableEdit::MoveRow {
                                from: row,
                                to: row + 1,
                            },
                            row + 1 < rows,
                        ),
                        item("Delete row", TableEdit::DeleteRow(row), rows > 1),
                    ],
                );
                if row == 0 {
                    menu = menu
                        .separator()
                        .item(item("Header row", TableEdit::ToggleHeaderRow, true).checked(header_row));
                }
                menu
            })
            .into_any_element()
    }

    /// Applies a control's edit through the editor, then redraws.
    pub(super) fn table_edit(
        &mut self,
        table: BlockId,
        edit: TableEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(Err(err)) = self.page().map(|page| page.edit_table(table, edit)) {
            report("could not change the table", err);
        }
        self.changed(window, cx);
    }
}

/// `menu` with `items` added in order.
fn menu_with<const N: usize>(menu: PopupMenu, items: [PopupMenuItem; N]) -> PopupMenu {
    items.into_iter().fold(menu.min_w(px(180.0)), PopupMenu::item)
}

#[cfg(test)]
mod tests;
