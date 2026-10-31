//! The shell's side of sorting and filtering (E09-S07): which pop-up of the bar above the table is open, and each
//! choice in it handed to `DatabaseEditor` (`leaf_editor` `database_view/query.rs`), which decides what it writes.

use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::calendar::{CalendarEvent, CalendarState, Date};
use gpui_component::input::{InputEvent, InputState};
use leaf_core::ColumnId;
use leaf_editor::{DatabaseEditor, FilterInput};
use leaf_model::{CellValue, FilterOp};

use super::super::{Shell, report};
use super::offset_minutes;

/// The bar's open pop-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QueryMenu {
    /// Columns to sort by.
    AddSort,
    /// The sorts, to turn round or remove.
    Sorts,
    /// Columns to filter by.
    AddFilter,
    /// One filter: its condition and value.
    Filter(usize),
}

impl Shell {
    fn query_editor(&mut self) -> Option<&mut DatabaseEditor> {
        let id = self.database_view()?.id;
        self.editor.database_editor(id)
    }

    /// The bar's open pop-up, if any.
    pub(super) fn query_menu(&self) -> Option<QueryMenu> {
        self.database_view().and_then(|d| d.query_menu)
    }

    /// Opens one of the bar's pop-ups, closing any other menu first. A filter's pop-up starts with its value.
    pub(super) fn open_query_menu(&mut self, menu: QueryMenu, window: &mut Window, cx: &mut Context<Self>) {
        self.close_menus(window, cx);
        self.cancel_cell_edit(window, cx);
        if let QueryMenu::Filter(index) = menu {
            let input = self
                .query_editor()
                .and_then(|e| e.query_bar(offset_minutes()).ok())
                .and_then(|bar| bar.filters.get(index).map(|f| f.input.clone()));
            let Some(d) = self.database_view() else {
                return;
            };
            match input {
                Some(FilterInput::Text(text) | FilterInput::Number(text)) => {
                    let field = d.filter_text.clone();
                    field.update(cx, |field, cx| {
                        field.set_value(text, window, cx);
                        field.focus(window, cx);
                    });
                }
                Some(FilterInput::Date(at)) => {
                    let day = at.and_then(|at| {
                        chrono::DateTime::from_timestamp_millis(at)
                            .map(|t| t.with_timezone(&chrono::Local).date_naive())
                    });
                    d.filter_calendar
                        .update(cx, |c, cx| c.set_date(Date::Single(day), window, cx));
                }
                _ => {}
            }
        }
        if let Some(d) = self.database_view_mut() {
            d.query_menu = Some(menu);
        }
        cx.notify();
    }

    /// Closes the bar's pop-up, writing a filter that was being typed.
    pub(super) fn close_query_menu(&mut self, cx: &mut Context<Self>) {
        if self.query_menu().is_none() {
            return;
        }
        if let Some(Err(err)) = self.query_editor().map(DatabaseEditor::commit_filter_text) {
            report("could not save the filter", err);
        }
        if let Some(d) = self.database_view_mut() {
            d.query_menu = None;
        }
        self.refresh_database_table(cx);
    }

    /// Closes every menu over the table: the column menus and the bar's pop-up.
    pub(super) fn close_menus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_column_menus(window, cx);
        self.close_query_menu(cx);
    }

    /// Runs `change` on the open database's editor, reports a failure as `what`, and redraws the table.
    fn change_query(
        &mut self,
        what: &str,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut DatabaseEditor) -> Result<(), leaf_model::ModelError>,
    ) {
        if let Some(Err(err)) = self.query_editor().map(change) {
            report(what, err);
        }
        self.refresh_database_table(cx);
    }

    /// Sorts by `column` too, then shows the sorts.
    pub(super) fn add_sort_by(&mut self, column: ColumnId, window: &mut Window, cx: &mut Context<Self>) {
        self.change_query("could not sort", cx, |e| e.add_sort(column).map(drop));
        self.open_query_menu(QueryMenu::Sorts, window, cx);
    }

    /// Turns sort `index` round.
    pub(super) fn turn_sort(&mut self, index: usize, descending: bool, cx: &mut Context<Self>) {
        self.change_query("could not sort", cx, |e| e.set_sort_descending(index, descending));
    }

    /// Removes sort `index`; the pop-up closes with the last one.
    pub(super) fn remove_sort_at(&mut self, index: usize, cx: &mut Context<Self>) {
        self.change_query("could not remove the sort", cx, |e| e.remove_sort(index));
        let none_left = self
            .query_editor()
            .is_some_and(|e| e.database().sort().is_empty());
        if none_left {
            self.close_query_menu(cx);
        }
    }

    /// Adds a filter on `column` and opens it to be filled in.
    pub(super) fn add_filter_on(&mut self, column: ColumnId, window: &mut Window, cx: &mut Context<Self>) {
        let index = match self.query_editor().map(|e| e.add_filter(column)) {
            Some(Ok(index)) => index,
            Some(Err(err)) => return report("could not add the filter", err),
            None => return,
        };
        self.refresh_database_table(cx);
        self.open_query_menu(QueryMenu::Filter(index), window, cx);
    }

    /// Changes filter `index`'s condition.
    pub(super) fn choose_filter_op(&mut self, index: usize, op: FilterOp, cx: &mut Context<Self>) {
        self.change_query("could not change the filter", cx, |e| e.set_filter_op(index, op));
    }

    /// Sets filter `index`'s value (an option, ticked or not).
    pub(super) fn choose_filter_value(
        &mut self,
        index: usize,
        value: Option<CellValue>,
        cx: &mut Context<Self>,
    ) {
        self.change_query("could not change the filter", cx, |e| {
            e.set_filter_value(index, value)
        });
    }

    /// Removes filter `index` and closes its pop-up.
    pub(super) fn remove_filter_at(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(e) = self.query_editor() {
            e.cancel_filter_text();
        }
        self.change_query("could not remove the filter", cx, |e| e.remove_filter(index));
        if let Some(d) = self.database_view_mut() {
            d.query_menu = None;
        }
        cx.notify();
    }

    pub(super) fn on_filter_text_event(
        &mut self,
        field: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(QueryMenu::Filter(index)) = self.query_menu() else {
            return;
        };
        match event {
            InputEvent::Change => {
                let text = field.read(cx).value().to_string();
                if let Some(Err(err)) = self.query_editor().map(|e| e.type_filter_text(index, &text)) {
                    report("could not filter", err);
                }
                self.refresh_database_table(cx);
            }
            InputEvent::PressEnter { .. } => self.close_query_menu(cx),
            _ => {}
        }
    }

    pub(super) fn on_filter_calendar_event(
        &mut self,
        _: &Entity<CalendarState>,
        event: &CalendarEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(QueryMenu::Filter(index)) = self.query_menu() else {
            return;
        };
        let CalendarEvent::Selected(date) = event;
        let at = match date {
            Date::Single(Some(day)) => day
                .and_hms_opt(0, 0, 0)
                .and_then(|t| t.and_local_timezone(chrono::Local).earliest())
                .map(|t| t.timestamp_millis()),
            _ => None,
        };
        self.choose_filter_value(index, at.map(CellValue::Date), cx);
    }
}

/// The bar's own inputs, made with the page: the field a filter's text is typed into and its calendar.
pub(super) fn inputs(
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> (Entity<InputState>, Entity<CalendarState>) {
    let text = cx.new(|cx| InputState::new(window, cx).placeholder("Type a value"));
    let calendar = cx.new(|cx| CalendarState::new(window, cx));
    (text, calendar)
}
