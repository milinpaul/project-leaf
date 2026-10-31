//! The database's table as gpui-component's data table sees it: a delegate that draws the header and each cell
//! from the editor's `TableView`, and forwards clicks to the shell. It decides nothing itself.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement as _, Styled, WeakEntity, Window, anchored, deferred, div, px,
};
use gpui_component::input::{Input, InputState};
use gpui_component::table::{Column, TableDelegate, TableState};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{CellDisplay, Chip, OptionPicker, TableView};
use leaf_model::PropertyType;

use super::super::Shell;
use super::{EditKind, Editing};

/// Width of the title column, which every database has first.
pub(super) const TITLE_WIDTH: f32 = 260.0;

/// Space either side of a cell's content. The columns have no padding of their own, so each cell reaches its
/// column's edges and can draw the divider there.
const CELL_PADDING: gpui::Pixels = px(8.0);

/// A header cell's box, `width` wide: padded, with a subtle divider on its right, like the rows'. The table keeps
/// room for a sort icon on the right of a header (sorting is off here); a fixed width reaches over it, so the
/// divider sits on the column's edge, in line with the rows'.
fn divided(width: f32, cx: &App) -> gpui::Div {
    h_flex()
        .h_full()
        .w(px(width))
        .flex_shrink_0()
        .px(CELL_PADDING)
        .border_r_1()
        .border_color(cx.theme().table_row_border)
}

/// The table's data and what is being edited, for drawing.
pub(in crate::shell) struct DatabaseTable {
    pub(super) view: TableView,
    pub(super) editing: Option<Editing>,
    /// The option picker's contents while one is open.
    pub(super) picker: Option<OptionPicker>,
    pub(super) text: Entity<InputState>,
    pub(super) query: Entity<InputState>,
    pub(super) calendar: Entity<gpui_component::calendar::CalendarState>,
    pub(super) shell: WeakEntity<Shell>,
}

/// The icon for a column type, shown before its name.
fn type_icon(kind: &PropertyType) -> IconName {
    match kind {
        PropertyType::Text => IconName::TextAlignStart,
        PropertyType::Number => IconName::Hash,
        PropertyType::Select => IconName::CircleChevronDown,
        PropertyType::MultiSelect => IconName::List,
        PropertyType::Date => IconName::Calendar,
        PropertyType::Checkbox => IconName::SquareCheck,
        PropertyType::Url => IconName::Link,
        PropertyType::Unknown(_) => IconName::CircleQuestionMark,
    }
}

impl TableDelegate for DatabaseTable {
    fn columns_count(&self, _: &App) -> usize {
        self.view.columns.len() + 1
    }

    fn rows_count(&self, _: &App) -> usize {
        self.view.rows.len()
    }

    #[expect(clippy::cast_possible_truncation, reason = "column widths are small")]
    fn column(&self, col_ix: usize, _: &App) -> Column {
        match col_ix.checked_sub(1).and_then(|i| self.view.columns.get(i)) {
            None => Column::new("title", "Name")
                .width(px(TITLE_WIDTH))
                .resizable(false)
                .p_0(),
            Some(column) => Column::new(SharedString::from(column.id.to_string()), column.name.clone())
                .width(px(column.width as f32))
                .resizable(false)
                .p_0(),
        }
    }

    #[expect(clippy::cast_possible_truncation, reason = "column widths are small")]
    fn render_th(
        &mut self,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let (name, icon, width) = match col_ix.checked_sub(1).and_then(|i| self.view.columns.get(i)) {
            None => ("Name".to_owned(), IconName::TextAlignStart, TITLE_WIDTH),
            Some(column) => (column.name.clone(), type_icon(&column.kind), column.width as f32),
        };
        divided(width, cx)
            .gap_1p5()
            .items_center()
            .text_color(cx.theme().muted_foreground)
            .debug_selector(move || format!("database-head-{col_ix}"))
            .child(Icon::new(icon).xsmall())
            .child(name)
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let editing = self
            .editing
            .filter(|e| e.row == row_ix && e.col == col_ix)
            .map(|e| e.kind);
        let cell = div()
            .id(SharedString::from(format!("database-cell-{row_ix}-{col_ix}")))
            .debug_selector(move || format!("database-cell-{row_ix}-{col_ix}"))
            .size_full()
            .flex()
            .items_center()
            .px(CELL_PADDING)
            .border_r_1()
            .border_color(cx.theme().table_row_border)
            .relative()
            .overflow_hidden();
        let cell = match editing {
            Some(_) => {
                let shell = self.shell.clone();
                cell.key_context(super::CELL_CONTEXT)
                    .on_action(move |_: &super::StopEditing, window, cx| {
                        if let Some(shell) = shell.upgrade() {
                            shell.update(cx, |shell, cx| shell.cancel_cell_edit(window, cx));
                        }
                    })
            }
            None => cell,
        };
        let Some(row) = self.view.rows.get(row_ix) else {
            return cell.into_any_element();
        };
        if matches!(editing, Some(EditKind::Title | EditKind::Text)) {
            return cell
                .child(Input::new(&self.text).appearance(false).small())
                .into_any_element();
        }
        let shell = self.shell.clone();
        let content: AnyElement = match col_ix.checked_sub(1) {
            None => div()
                .font_weight(FontWeight::MEDIUM)
                .truncate()
                .child(row.title.clone())
                .into_any_element(),
            Some(i) => match row.cells.get(i) {
                Some(display) => render_display(display, row_ix, col_ix, &shell, cx),
                None => div().into_any_element(),
            },
        };
        // Double-clicks reach the shell as the table's `DoubleClickedCell`.
        let cell = cell.child(content);
        let popover = match editing {
            Some(EditKind::Options) => self.picker.as_ref().map(|picker| {
                super::cells::render_picker(picker, &self.query, &self.shell, cx).into_any_element()
            }),
            Some(EditKind::Date) => Some(super::cells::render_calendar(&self.calendar, window, cx)),
            _ => None,
        };
        match popover {
            Some(panel) => cell
                // Anchored to the cell's bottom-left corner: a flex child of the cell would be centred on it.
                .child(
                    div().absolute().left_0().top_full().child(
                        deferred(
                            anchored()
                                .snap_to_window_with_margin(px(8.0))
                                .child(div().mt_1().child(panel)),
                        )
                        .with_priority(1),
                    ),
                )
                .into_any_element(),
            None => cell.into_any_element(),
        }
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _: &App) -> String {
        let Some(row) = self.view.rows.get(row_ix) else {
            return String::new();
        };
        match col_ix.checked_sub(1).and_then(|i| row.cells.get(i)) {
            None => row.title.clone(),
            Some(
                CellDisplay::Text(t) | CellDisplay::Number(t) | CellDisplay::Date(t) | CellDisplay::Url(t),
            ) => t.clone(),
            Some(CellDisplay::Checkbox(on)) => if *on { "Yes" } else { "No" }.to_owned(),
            Some(CellDisplay::Options(chips)) => chips
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            Some(CellDisplay::Empty | CellDisplay::Unknown) => String::new(),
        }
    }
}

/// One cell's display value.
fn render_display(
    display: &CellDisplay,
    row_ix: usize,
    col_ix: usize,
    shell: &WeakEntity<Shell>,
    cx: &mut Context<TableState<DatabaseTable>>,
) -> AnyElement {
    let theme = cx.theme();
    match display {
        CellDisplay::Empty => div().into_any_element(),
        CellDisplay::Text(text) | CellDisplay::Date(text) => {
            div().truncate().child(text.clone()).into_any_element()
        }
        CellDisplay::Number(text) => div()
            .w_full()
            .text_right()
            .truncate()
            .child(text.clone())
            .into_any_element(),
        CellDisplay::Url(text) => div()
            .truncate()
            .text_color(theme.link)
            .underline()
            .child(text.clone())
            .into_any_element(),
        CellDisplay::Checkbox(on) => {
            let shell = shell.clone();
            div()
                .id(SharedString::from(format!("database-check-{row_ix}-{col_ix}")))
                .debug_selector(move || format!("database-check-{row_ix}-{col_ix}"))
                .size(px(16.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .border_1()
                .border_color(theme.border)
                .cursor_pointer()
                .when(*on, |b| b.bg(theme.primary).border_color(theme.primary))
                .when(*on, |b| {
                    b.child(
                        Icon::new(IconName::Check)
                            .xsmall()
                            .text_color(theme.primary_foreground),
                    )
                })
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    if let Some(shell) = shell.upgrade() {
                        shell.update(cx, |shell, cx| {
                            shell.toggle_database_checkbox(row_ix, col_ix, window, cx);
                        });
                    }
                })
                .into_any_element()
        }
        CellDisplay::Options(chips) => h_flex()
            .gap_1()
            .overflow_hidden()
            .children(chips.iter().map(|chip| render_chip(chip, cx)))
            .into_any_element(),
        CellDisplay::Unknown => div()
            .text_color(theme.muted_foreground)
            .child("—")
            .into_any_element(),
    }
}

/// An option as a small coloured label.
pub(super) fn render_chip(chip: &Chip, cx: &App) -> AnyElement {
    let (background, foreground) = crate::theme::option_colors(&chip.color, cx);
    div()
        .px_1p5()
        .rounded_sm()
        .text_sm()
        .bg(background)
        .text_color(foreground)
        .whitespace_nowrap()
        .child(chip.name.clone())
        .into_any_element()
}
