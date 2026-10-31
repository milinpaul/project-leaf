//! Editing a row page's properties (E09-S08): opening a property's editor, and handing what was typed, ticked,
//! picked or chosen to `DatabaseEditor` with the row's page as the row. Nothing here decides what a value becomes.

use gpui::{Context, Entity, Window};
use gpui_component::calendar::{CalendarEvent, CalendarState, Date};
use gpui_component::input::{InputEvent, InputState};
use leaf_core::{ColumnId, OptionId};
use leaf_editor::{CellEditor, OptionPicker, TextCommit};
use leaf_model::PropertyType;

use super::super::super::{Shell, report};
use super::super::EditKind;

impl Shell {
    /// The row's page changed something the table and this page show: redraw both, and measure the page's
    /// header again, as the properties under the title may have changed height.
    pub(super) fn row_changed(&mut self, cx: &mut Context<Self>) {
        self.blocks.list.state.remeasure_items(0..1);
        self.refresh_database_table(cx);
        cx.notify();
    }

    pub(super) fn set_row_editing(&mut self, editing: Option<(ColumnId, EditKind)>, cx: &mut Context<Self>) {
        if let Some(row) = &mut self.row_page {
            row.editing = editing;
            if !matches!(editing, Some((_, EditKind::Options))) {
                row.picker = None;
                row.option_menu = None;
            }
        }
        self.row_changed(cx);
    }

    /// Opens the editor of property `column` (or ticks a checkbox).
    pub(in crate::shell) fn begin_row_edit(
        &mut self,
        column: ColumnId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .row_page
            .as_ref()
            .and_then(|r| r.editing)
            .is_some_and(|(c, _)| c == column)
        {
            return;
        }
        self.close_row_edit(window, cx);
        let editor = match self.row_editor().map(|(page, e)| e.cell_editor(page, column)) {
            Some(Ok(editor)) => editor,
            Some(Err(err)) => return report("could not edit the property", err),
            None => return,
        };
        let Some(row) = &self.row_page else {
            return;
        };
        let (text, query, calendar) = (row.text.clone(), row.query.clone(), row.calendar.clone());
        match editor {
            CellEditor::Text { initial, .. } => {
                text.update(cx, |input, cx| {
                    input.set_value(initial, window, cx);
                    input.focus(window, cx);
                });
                self.set_row_editing(Some((column, EditKind::Text)), cx);
            }
            CellEditor::Checkbox => {
                if let Some(Err(err)) = self.row_editor().map(|(page, e)| e.toggle_checkbox(page, column)) {
                    report("could not tick the box", err);
                }
                self.row_changed(cx);
            }
            CellEditor::Options { .. } => {
                query.update(cx, |q, cx| {
                    q.set_value("", window, cx);
                    q.focus(window, cx);
                });
                self.set_row_editing(Some((column, EditKind::Options)), cx);
                self.refresh_row_picker(cx);
            }
            CellEditor::Date { initial } => {
                let day = initial.and_then(|at| {
                    chrono::DateTime::from_timestamp_millis(at)
                        .map(|t| t.with_timezone(&chrono::Local).date_naive())
                });
                calendar.update(cx, |c, cx| c.set_date(Date::Single(day), window, cx));
                calendar.read(cx).focus_handle.clone().focus(window, cx);
                self.set_row_editing(Some((column, EditKind::Date)), cx);
            }
            CellEditor::ReadOnly => {}
        }
    }

    /// Closes the open property editor, keeping what was typed into a text one.
    pub(in crate::shell) fn close_row_edit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some((column, kind)) = self.row_page.as_ref().and_then(|r| r.editing) else {
            return;
        };
        if kind == EditKind::Text && !self.commit_row_text(column, cx) {
            return;
        }
        self.set_row_editing(None, cx);
    }

    /// Writes the text field into property `column`. `false` when it doesn't read as a number, so the field stays.
    pub(super) fn commit_row_text(&mut self, column: ColumnId, cx: &mut Context<Self>) -> bool {
        let Some(text) = self
            .row_page
            .as_ref()
            .map(|r| r.text.read(cx).value().to_string())
        else {
            return true;
        };
        match self
            .row_editor()
            .map(|(page, e)| e.commit_text(page, column, &text))
        {
            Some(Ok(TextCommit::NotANumber)) => false,
            Some(Err(err)) => {
                report("could not save the property", err);
                true
            }
            _ => true,
        }
    }

    pub(super) fn cancel_row_edit(&mut self, cx: &mut Context<Self>) {
        self.set_row_editing(None, cx);
    }

    pub(super) fn on_row_text_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::PressEnter { .. } | InputEvent::Blur => self.close_row_edit(window, cx),
            _ => {}
        }
    }

    pub(in crate::shell::database_page) fn editing_column(&self) -> Option<ColumnId> {
        self.row_page.as_ref()?.editing.map(|(column, _)| column)
    }

    pub(in crate::shell::database_page) fn refresh_row_picker(&mut self, cx: &mut Context<Self>) {
        let Some(column) = self.editing_column() else {
            return;
        };
        let Some(query) = self
            .row_page
            .as_ref()
            .map(|r| r.query.read(cx).value().to_string())
        else {
            return;
        };
        let picker = match self
            .row_editor()
            .map(|(page, e)| e.option_picker(page, column, &query))
        {
            Some(Ok(picker)) => picker,
            Some(Err(err)) => return report("could not list the options", err),
            None => return,
        };
        if let Some(row) = &mut self.row_page {
            row.picker = Some(picker);
        }
        cx.notify();
    }

    pub(super) fn on_row_query_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => self.refresh_row_picker(cx),
            // Enter picks the best match, or creates the typed name.
            InputEvent::PressEnter { .. } => {
                let picker = self.row_page.as_ref().and_then(|r| r.picker.clone());
                match picker {
                    Some(p) if !p.choices.is_empty() => self.pick_row_option(p.choices[0].option, window, cx),
                    Some(OptionPicker {
                        create: Some(name), ..
                    }) => self.create_row_option(&name, window, cx),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// After an option is picked or made: a multi-select's picker stays open for the next; a select's closes.
    pub(super) fn after_row_option(&mut self, column: ColumnId, cx: &mut Context<Self>) {
        let multiple = self
            .row_editor()
            .and_then(|(_, e)| e.database().column(column).ok())
            .is_some_and(|c| c.kind == PropertyType::MultiSelect);
        if multiple {
            self.refresh_row_picker(cx);
            self.row_changed(cx);
        } else {
            self.cancel_row_edit(cx);
        }
    }

    /// Picks `option` in the property being edited.
    pub(in crate::shell) fn pick_row_option(
        &mut self,
        option: OptionId,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(column) = self.editing_column() else {
            return;
        };
        if let Some(Err(err)) = self
            .row_editor()
            .map(|(page, e)| e.toggle_option(page, column, option))
        {
            report("could not pick the option", err);
        }
        self.after_row_option(column, cx);
    }

    /// Creates an option named `name` and picks it in the property being edited.
    pub(in crate::shell) fn create_row_option(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(column) = self.editing_column() else {
            return;
        };
        if let Some(Err(err)) = self
            .row_editor()
            .map(|(page, e)| e.create_option(page, column, name))
        {
            return report("could not add the option", err);
        }
        if let Some(query) = self.row_page.as_ref().map(|r| r.query.clone()) {
            query.update(cx, |q, cx| q.set_value("", window, cx));
        }
        self.after_row_option(column, cx);
    }

    pub(super) fn on_row_calendar_event(
        &mut self,
        _: &Entity<CalendarState>,
        event: &CalendarEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(column) = self.editing_column() else {
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
        if let Some(Err(err)) = self.row_editor().map(|(page, e)| e.set_date(page, column, at)) {
            report("could not set the date", err);
        }
        self.cancel_row_edit(cx);
    }
}
