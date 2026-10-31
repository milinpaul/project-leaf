//! The table's header (E09-S06): each column's name and type icon, which opens its menu, and a "+" column at the
//! end, which opens the menu that adds a column or shows a hidden one. The menus themselves are `column_menu`'s.

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Window, anchored, deferred, div, px,
};
use gpui_component::table::TableState;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{TableView, ViewColumn};
use leaf_model::PropertyType;

use super::column_menu;
use super::table::{CELL_PADDING, DatabaseTable};

/// Width of the "+" column.
pub(super) const ADD_WIDTH: f32 = 40.0;
/// The narrowest a column can be dragged.
pub(super) const MIN_WIDTH: f32 = 60.0;

/// What a table column is.
pub(super) enum Slot<'a> {
    /// The title column, first.
    Title,
    /// One of the database's shown columns.
    Column(&'a ViewColumn),
    /// The "+" column, last.
    Add,
}

/// What table column `col_ix` is.
pub(super) fn slot(view: &TableView, col_ix: usize) -> Slot<'_> {
    match col_ix.checked_sub(1) {
        None => Slot::Title,
        Some(i) => view.columns.get(i).map_or(Slot::Add, Slot::Column),
    }
}

/// The icon for a column type, shown before its name.
pub(super) fn type_icon(kind: &PropertyType) -> IconName {
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

/// A menu hung under its header cell, over everything else.
fn under(panel: AnyElement) -> AnyElement {
    div()
        .absolute()
        .left_0()
        .top_full()
        .child(
            deferred(
                anchored()
                    .snap_to_window_with_margin(px(8.0))
                    .child(div().mt_1().child(panel)),
            )
            .with_priority(1),
        )
        .into_any_element()
}

/// Header cell `col_ix`. It fills the whole cell, over the room the table keeps for a sort icon (sorting is off
/// here), so its divider sits on the column's edge, in line with the rows', however the column is resized.
pub(super) fn render(
    table: &DatabaseTable,
    col_ix: usize,
    _: &mut Window,
    cx: &mut Context<TableState<DatabaseTable>>,
) -> AnyElement {
    let theme = cx.theme();
    let name = match slot(&table.view, col_ix) {
        Slot::Add => "database-add-column".to_owned(),
        _ => format!("database-head-{col_ix}"),
    };
    let selector = name.clone();
    let cell = h_flex()
        .id(SharedString::from(name))
        .debug_selector(move || selector.clone())
        .absolute()
        .top_0()
        .bottom_0()
        .left_0()
        .right_0()
        .px(CELL_PADDING)
        .gap_1p5()
        .items_center()
        .border_r_1()
        .border_color(theme.table_row_border)
        .text_color(theme.muted_foreground)
        .overflow_hidden();
    let shell = table.shell.clone();
    match slot(&table.view, col_ix) {
        Slot::Title => cell
            .child(Icon::new(IconName::TextAlignStart).xsmall())
            .child("Name")
            .into_any_element(),
        Slot::Column(column) => {
            let menu = table
                .column_menu
                .as_ref()
                .filter(|(at, _)| *at == col_ix)
                .map(|(_, menu)| {
                    under(column_menu::render_column_menu(
                        menu,
                        &table.column_name,
                        &shell,
                        cx,
                    ))
                });
            cell.cursor_pointer()
                .hover(|s| s.bg(cx.theme().muted))
                .child(Icon::new(type_icon(&column.kind)).xsmall())
                .child(div().truncate().child(column.name.clone()))
                .on_click(move |_, window, cx| {
                    if let Some(shell) = shell.upgrade() {
                        shell.update(cx, |shell, cx| shell.open_column_menu(col_ix, window, cx));
                    }
                })
                .children(menu)
                .into_any_element()
        }
        Slot::Add => {
            let menu = table
                .add_menu
                .as_ref()
                .map(|hidden| under(column_menu::render_add_menu(hidden, &shell, cx)));
            cell.justify_center()
                .border_r_0()
                .cursor_pointer()
                .hover(|s| s.bg(cx.theme().muted))
                .child(Icon::new(IconName::Plus).small())
                .on_click(move |_, window, cx| {
                    if let Some(shell) = shell.upgrade() {
                        shell.update(cx, |shell, cx| shell.open_add_column_menu(window, cx));
                    }
                })
                .children(menu)
                .into_any_element()
        }
    }
}
