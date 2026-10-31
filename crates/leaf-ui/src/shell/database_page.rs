//! A database shown as a full page (E09-S05): its title, then its table, then a "New" line for another row.
//!
//! The table is gpui-component's data table, filled from `WorkspaceEditor::database_view` (columns, rows and what
//! each cell shows) through the delegate in `table`. The first screen of rows is read when the page opens; the rest
//! are read a chunk at a time on a timer until every row is in (E09-S04), so a big database opens at once.
//!
//! Editing: double-click a cell, or select it with the arrows and press Enter. Text, URL and number cells (and
//! the title) edit in place, committed with Enter or by clicking away and dropped with Escape. A checkbox
//! toggles on click or Enter. Select and multi-select cells open an option picker, date cells a calendar. What
//! each input writes is `DatabaseEditor`'s (`leaf_editor::CellEditor`); this module only draws and forwards.
//!
//! Columns (E09-S06): a header opens its column's menu (name, type, move, hide, delete); the "+" at the end adds a
//! column or shows a hidden one. Headers drag to reorder and their edges drag to resize. Cmd/Ctrl+Z undoes the
//! database's last step.
//!
//! Sorting and filtering (E09-S07): a bar above the table shows a chip for each sort and filter, with "Sort" and
//! "Filter" buttons; each opens a pop-up (`query_bar.rs`). A filter's typed value shows in the table at once and is
//! saved when the pop-up closes.
//!
//! A row opens as a page (E09-S08): the "Open" button on its title cell, and its properties under the page's title
//! (`row_page.rs`).
//!
//! A database can also sit inside a page as a block (E09-S09, `inline.rs`). Each database shown has its own view;
//! the actions above work on the active one.

mod cells;
mod column_menu;
mod columns;
mod editing;
mod header;
mod inline;
mod options;
mod query;
mod query_bar;
pub(super) mod row_page;
mod table;
mod views;

use std::time::Duration;

use gpui::{
    AnyElement, Context, Entity, InteractiveElement as _, IntoElement, KeyBinding, ParentElement,
    StatefulInteractiveElement as _, Styled, Subscription, Task, Window, actions, div, px,
};
use gpui_component::calendar::CalendarState;
use gpui_component::input::InputState;
use gpui_component::table::{DataTable, TableEvent, TableState};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, Size, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_core::{ColumnId, DatabaseId, PageId};

use super::{Shell, now_ms, report};
use table::DatabaseTable;

/// Rows read per tick after the first screen.
const READ_CHUNK: usize = 500;
/// How often the rest of a database is read.
const READ_TICK: Duration = Duration::from_millis(16);
/// Height of each row and of the header, in points.
const ROW_HEIGHT: f32 = 32.0;
/// The table's top and bottom border.
const TABLE_BORDERS: f32 = 2.0;
/// Key context around the table, for Enter and Escape.
const CONTEXT: &str = "DatabasePage";
/// Key context around a cell's open editor. It sits inside the table, which takes Escape for itself while a cell
/// is selected, so the editor binds Escape here, closer in.
const CELL_CONTEXT: &str = "DatabaseCell";
/// Key context around a row page's properties (E09-S08), for Escape.
const ROW_CONTEXT: &str = "DatabaseRowPage";

actions!(database_page, [EditCell, StopEditing, UndoDatabase, RedoDatabase]);

/// Enter edits the selected cell; Escape stops editing or closes a menu; Cmd/Ctrl+Z and Shift+Cmd/Ctrl+Z undo and
/// redo the database (Ctrl+Y too, off macOS).
pub(crate) fn bind(cx: &mut gpui::App) {
    let (undo, redo) = if cfg!(target_os = "macos") {
        ("cmd-z", "cmd-shift-z")
    } else {
        ("ctrl-z", "ctrl-shift-z")
    };
    cx.bind_keys([
        KeyBinding::new("enter", EditCell, Some(CONTEXT)),
        KeyBinding::new("escape", StopEditing, Some(CONTEXT)),
        KeyBinding::new("escape", StopEditing, Some(CELL_CONTEXT)),
        KeyBinding::new("escape", StopEditing, Some(ROW_CONTEXT)),
        KeyBinding::new(undo, UndoDatabase, Some(CONTEXT)),
        KeyBinding::new(redo, RedoDatabase, Some(CONTEXT)),
    ]);
    if !cfg!(target_os = "macos") {
        cx.bind_keys([KeyBinding::new("ctrl-y", RedoDatabase, Some(CONTEXT))]);
    }
    inline::bind(cx);
}

/// A cell being edited: its view row and table column (0 is the title).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Editing {
    pub(super) row: usize,
    pub(super) col: usize,
    pub(super) kind: EditKind,
}

/// Which editor a cell has open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditKind {
    Title,
    Text,
    Options,
    Date,
}

/// The open database page's table and editors.
pub(super) struct DatabasePage {
    page: PageId,
    id: DatabaseId,
    table: Entity<TableState<DatabaseTable>>,
    text: Entity<InputState>,
    query: Entity<InputState>,
    calendar: Entity<CalendarState>,
    /// The column menu's name field.
    column_name: Entity<InputState>,
    /// The bar's open pop-up (E09-S07).
    query_menu: Option<query::QueryMenu>,
    /// The field a filter's value is typed into, and the calendar a date filter picks from.
    filter_text: Entity<InputState>,
    filter_calendar: Entity<CalendarState>,
    editing: Option<Editing>,
    reading: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

/// The device's time zone, in minutes east of UTC, for dates.
fn offset_minutes() -> i32 {
    chrono::Local::now().offset().local_minus_utc() / 60
}

impl Shell {
    /// The open page's database, when the open page is a database page.
    fn open_database(&self) -> Option<(PageId, DatabaseId)> {
        let page = self.editor.selected()?;
        let database = self.editor.workspace().page(page).ok()?.database?;
        Some((page, database))
    }

    /// Whether the open page is a database page, drawn as a table.
    pub(super) fn showing_database(&self) -> bool {
        self.open_database().is_some()
    }

    /// Adds an empty database page at the end of the top level and opens it (the sidebar's table button).
    pub fn new_database(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.editor.new_database_page(None, now_ms()) {
            Ok((page, _)) => self.open(page, window, cx),
            Err(err) => report("could not create a database", err),
        }
    }

    /// Makes sure the open database page has its table, loading the database from disk the first time.
    fn ensure_database_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<DatabaseId> {
        let (page, id) = self.open_database()?;
        self.ensure_database_view(page, id, window, cx);
        self.activate_database(page);
        Some(id)
    }

    /// Hands the table the database as it is now.
    pub(super) fn refresh_database_table(&mut self, cx: &mut Context<Self>) {
        let Some((id, table)) = self.database_parts() else {
            return;
        };
        let view = match self.editor.database_view(id, offset_minutes()) {
            Ok(view) => view,
            Err(err) => return report("could not read the database", err),
        };
        table.update(cx, |table, cx| {
            table.delegate_mut().view = view;
            table.refresh(cx);
        });
        // Inline, the block grows and shrinks with its rows. This can run while the page's list is drawing (a view
        // is made on first drawing), so the list hears of it after.
        if let Some(page) = self.active_database {
            let shell = cx.entity();
            cx.defer(move |cx| {
                shell.update(cx, |shell, cx| {
                    shell.blocks.list.remeasure_database(page);
                    cx.notify();
                });
            });
        }
        cx.notify();
    }

    /// The page's title, then the table, then "New".
    pub(super) fn render_database_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.ensure_database_page(window, cx).is_none() {
            return div().into_any_element();
        }
        let Some(database) = self.database_view() else {
            return div().into_any_element();
        };
        let table = database.table.clone();
        // As tall as its rows, so "New" sits under the last one; a long table scrolls inside the page.
        let rows = u16::try_from(table.read(cx).delegate().view.rows.len()).unwrap_or(u16::MAX);
        let height = ROW_HEIGHT * (f32::from(rows) + 1.0) + TABLE_BORDERS;
        let header = self.render_page_header(false, cx);
        let query_bar = self.render_query_bar(cx);
        let theme = cx.theme();
        let new_row = h_flex()
            .id("database-new-row")
            .debug_selector(|| "database-new-row".into())
            .h(px(ROW_HEIGHT))
            .px_3()
            .gap_2()
            .items_center()
            .text_color(theme.muted_foreground)
            .cursor_pointer()
            .hover(|s| s.bg(cx.theme().muted))
            .child(Icon::new(IconName::Plus).small())
            .child("New")
            .on_click(cx.listener(|shell, _, window, cx| shell.add_database_row(window, cx)));
        v_flex()
            .size_full()
            .px_12()
            .pt_8()
            .gap_2()
            .child(header)
            .child(
                Self::with_database_actions(v_flex().flex_1().min_h_0().key_context(CONTEXT), cx)
                    .child(query_bar)
                    .child(
                        div()
                            .h(px(height))
                            .flex_shrink(1.0)
                            .min_h(px(ROW_HEIGHT * 2.0))
                            .child(
                                DataTable::new(&table)
                                    .with_size(Size::Medium)
                                    .bordered(true)
                                    .header_row_height(px(ROW_HEIGHT))
                                    .scrollbar_visible(true, true),
                            ),
                    )
                    .child(new_row),
            )
            .into_any_element()
    }

    /// Attaches the table's actions (Enter, Escape, undo, redo) to `element`, the wrapper with the table's key context.
    fn with_database_actions<E: gpui::InteractiveElement>(element: E, cx: &mut Context<Self>) -> E {
        element
            .on_action(cx.listener(|shell, _: &EditCell, window, cx| shell.edit_selected_cell(window, cx)))
            .on_action(cx.listener(|shell, _: &StopEditing, window, cx| shell.stop_editing(window, cx)))
            .on_action(cx.listener(|shell, _: &UndoDatabase, window, cx| {
                shell.database_history(true, window, cx);
            }))
            .on_action(cx.listener(|shell, _: &RedoDatabase, window, cx| {
                shell.database_history(false, window, cx);
            }))
    }

    fn database_parts(&self) -> Option<(DatabaseId, Entity<TableState<DatabaseTable>>)> {
        self.database_view().map(|d| (d.id, d.table.clone()))
    }

    /// The row page and column of a table cell (`None` column for the title).
    fn cell_at(&self, row: usize, col: usize, cx: &gpui::App) -> Option<(PageId, Option<ColumnId>)> {
        let (_, table) = self.database_parts()?;
        let view = &table.read(cx).delegate().view;
        let page = view.rows.get(row)?.row;
        let column = match col.checked_sub(1) {
            None => None,
            Some(i) => Some(view.columns.get(i)?.id),
        };
        Some((page, column))
    }

    fn on_table_event(
        &mut self,
        _: &Entity<TableState<DatabaseTable>>,
        event: &TableEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            TableEvent::DoubleClickedCell(row, col) => self.begin_cell_edit(*row, *col, window, cx),
            TableEvent::SelectCell(..) if self.editing().is_some() => self.commit_cell_edit(window, cx),
            TableEvent::MoveColumn(from, to) => self.move_dragged_column(*from, *to, cx),
            TableEvent::ColumnWidthsChanged(widths) => self.resize_columns(widths, cx),
            _ => {}
        }
    }

    /// Escape: closes a column menu if one is open, otherwise drops the cell being edited.
    pub(super) fn stop_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.query_menu().is_some() {
            self.close_query_menu(cx);
        } else if self.column_menu_open(cx) {
            self.close_column_menus(window, cx);
        } else {
            self.cancel_cell_edit(window, cx);
        }
    }

    /// The "New" line: adds a row at the end and starts typing its title.
    fn add_database_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((page, id)) = self.database_view().map(|d| (d.page, d.id)) else {
            return;
        };
        let rows = self.editor.database(id).map_or(0, |db| db.rows().len());
        if let Err(err) = self.editor.add_row(page, rows, now_ms()) {
            return report("could not add a row", err);
        }
        self.refresh_database_table(cx);
        let last = self.database_parts().map_or(0, |(_, table)| {
            table.read(cx).delegate().view.rows.len().saturating_sub(1)
        });
        self.begin_cell_edit(last, 0, window, cx);
    }
}

#[cfg(test)]
mod tests;
