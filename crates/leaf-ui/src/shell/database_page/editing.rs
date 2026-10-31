//! Editing a database cell in place: opening the right editor for the cell, and handing what was typed, ticked,
//! picked or chosen to `DatabaseEditor`. Nothing here decides what a value becomes.

use gpui::{Context, Entity, Focusable as _, Window};
use gpui_component::calendar::{CalendarEvent, CalendarState, Date};
use gpui_component::input::InputEvent;
use gpui_component::input::InputState;
use leaf_core::{ColumnId, DatabaseId, PageId};
use leaf_editor::{CellEditor, TextCommit};

use super::super::{Shell, report};
use super::{EditKind, Editing};

impl Shell {
    pub(super) fn editing(&self) -> Option<Editing> {
        self.database_view().and_then(|d| d.editing)
    }

    /// Enter on the selected cell.
    pub(super) fn edit_selected_cell(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // An open editor takes its own Enter (a text field's `PressEnter`), which then bubbles up here.
        if self.editing().is_some() {
            return;
        }
        let Some((_, table)) = self.database_parts() else {
            return;
        };
        if let Some((row, col)) = table.read(cx).selected_cell() {
            self.begin_cell_edit(row, col, window, cx);
        }
    }

    /// Opens the cell's editor (or toggles a checkbox).
    pub(super) fn begin_cell_edit(
        &mut self,
        row: usize,
        col: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, _)) = self.database_parts() else {
            return;
        };
        let Some((page, column)) = self.cell_at(row, col, cx) else {
            return;
        };
        let Some(column) = column else {
            let title = self
                .editor
                .workspace()
                .page(page)
                .map(|m| m.title)
                .unwrap_or_default();
            return self.open_text_editor(row, col, EditKind::Title, title, window, cx);
        };
        let editor = match self
            .editor
            .database_editor(id)
            .map(|e| e.cell_editor(page, column))
        {
            Some(Ok(editor)) => editor,
            Some(Err(err)) => return report("could not edit the cell", err),
            None => return,
        };
        match editor {
            CellEditor::Text { initial, .. } => {
                self.open_text_editor(row, col, EditKind::Text, initial, window, cx);
            }
            CellEditor::Checkbox => self.toggle_database_checkbox(row, col, window, cx),
            CellEditor::Options { .. } => {
                self.set_editing(
                    Some(Editing {
                        row,
                        col,
                        kind: EditKind::Options,
                    }),
                    cx,
                );
                if let Some(d) = self.database_view() {
                    let query = d.query.clone();
                    query.update(cx, |q, cx| {
                        q.set_value("", window, cx);
                        q.focus(window, cx);
                    });
                }
                self.refresh_option_picker(cx);
            }
            CellEditor::Date { initial } => {
                if let Some(d) = self.database_view() {
                    let date = initial.and_then(|at| {
                        chrono::DateTime::from_timestamp_millis(at)
                            .map(|t| t.with_timezone(&chrono::Local).date_naive())
                    });
                    d.calendar
                        .update(cx, |c, cx| c.set_date(Date::Single(date), window, cx));
                    // The calendar takes the keyboard, so Escape reaches the cell's editor.
                    d.calendar.read(cx).focus_handle.clone().focus(window, cx);
                }
                self.set_editing(
                    Some(Editing {
                        row,
                        col,
                        kind: EditKind::Date,
                    }),
                    cx,
                );
            }
            CellEditor::ReadOnly => {}
        }
    }

    pub(super) fn open_text_editor(
        &mut self,
        row: usize,
        col: usize,
        kind: EditKind,
        initial: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(d) = self.database_view() {
            let text = d.text.clone();
            text.update(cx, |input, cx| {
                input.set_value(initial, window, cx);
                input.focus(window, cx);
            });
        }
        self.set_editing(Some(Editing { row, col, kind }), cx);
    }

    pub(super) fn set_editing(&mut self, editing: Option<Editing>, cx: &mut Context<Self>) {
        let Some(database) = self.database_view_mut() else {
            return;
        };
        database.editing = editing;
        database.table.update(cx, |table, cx| {
            let delegate = table.delegate_mut();
            delegate.editing = editing;
            if editing.is_none_or(|e| e.kind != EditKind::Options) {
                delegate.picker = None;
            }
            cx.notify();
        });
        cx.notify();
    }

    /// Writes what was typed into the cell being edited and closes the editor.
    pub(super) fn commit_cell_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.editing() else {
            return;
        };
        if matches!(editing.kind, EditKind::Title | EditKind::Text) {
            let Some(text) = self.database_view().map(|d| d.text.read(cx).value().to_string()) else {
                return;
            };
            let Some((page, column)) = self.cell_at(editing.row, editing.col, cx) else {
                return self.set_editing(None, cx);
            };
            match column {
                None => {
                    if let Err(err) = self.editor.rename(page, &text) {
                        report("could not rename the row", err);
                    }
                }
                Some(column) => {
                    let Some((id, _)) = self.database_parts() else {
                        return;
                    };
                    match self
                        .editor
                        .database_editor(id)
                        .map(|e| e.commit_text(page, column, &text))
                    {
                        // Not a number: the editor stays open so it can be fixed.
                        Some(Ok(TextCommit::NotANumber)) => return,
                        Some(Err(err)) => report("could not save the cell", err),
                        _ => {}
                    }
                }
            }
        }
        self.set_editing(None, cx);
        self.refresh_database_table(cx);
        self.focus_table(window, cx);
    }

    pub(super) fn cancel_cell_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing().is_some() {
            self.set_editing(None, cx);
            self.focus_table(window, cx);
        }
    }

    pub(super) fn focus_table(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, table)) = self.database_parts() {
            table.update(cx, |table, cx| table.focus_handle(cx).focus(window, cx));
        }
    }

    pub(super) fn on_cell_text_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::PressEnter { .. } => self.commit_cell_edit(window, cx),
            InputEvent::Blur
                if self
                    .editing()
                    .is_some_and(|e| matches!(e.kind, EditKind::Title | EditKind::Text)) =>
            {
                self.commit_cell_edit(window, cx);
            }
            _ => {}
        }
    }

    pub(super) fn toggle_database_checkbox(
        &mut self,
        row: usize,
        col: usize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some((id, _)), Some((page, Some(column)))) = (self.database_parts(), self.cell_at(row, col, cx))
        else {
            return;
        };
        if let Some(Err(err)) = self
            .editor
            .database_editor(id)
            .map(|e| e.toggle_checkbox(page, column))
        {
            report("could not tick the box", err);
        }
        self.refresh_database_table(cx);
    }

    pub(super) fn editing_cell(&self, cx: &gpui::App) -> Option<(DatabaseId, PageId, ColumnId)> {
        let editing = self.editing()?;
        let (id, _) = self.database_parts()?;
        let (page, column) = self.cell_at(editing.row, editing.col, cx)?;
        Some((id, page, column?))
    }

    pub(super) fn on_calendar_event(
        &mut self,
        _: &Entity<CalendarState>,
        event: &CalendarEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let CalendarEvent::Selected(date) = event;
        let Some((id, page, column)) = self.editing_cell(cx) else {
            return;
        };
        let at = match date {
            Date::Single(Some(day)) => day
                .and_hms_opt(0, 0, 0)
                .and_then(|t| t.and_local_timezone(chrono::Local).earliest())
                .map(|t| t.timestamp_millis()),
            _ => None,
        };
        if let Some(Err(err)) = self
            .editor
            .database_editor(id)
            .map(|e| e.set_date(page, column, at))
        {
            report("could not set the date", err);
        }
        self.set_editing(None, cx);
        self.refresh_database_table(cx);
        self.focus_table(window, cx);
    }
}
