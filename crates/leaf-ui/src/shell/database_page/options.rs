//! The option picker's side of editing a cell (select and multi-select): listing the options for what is typed,
//! picking one, and creating one. What each writes is `DatabaseEditor`'s.

use gpui::{Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};
use leaf_core::{ColumnId, DatabaseId, OptionId};

use super::super::{Shell, report};

impl Shell {
    pub(super) fn refresh_option_picker(&mut self, cx: &mut Context<Self>) {
        let Some((id, page, column)) = self.editing_cell(cx) else {
            return;
        };
        let Some(query) = self
            .database
            .as_ref()
            .map(|d| d.query.read(cx).value().to_string())
        else {
            return;
        };
        let picker = match self
            .editor
            .database_editor(id)
            .map(|e| e.option_picker(page, column, &query))
        {
            Some(Ok(picker)) => picker,
            Some(Err(err)) => return report("could not list the options", err),
            None => return,
        };
        if let Some((_, table)) = self.database_parts() {
            table.update(cx, |table, cx| {
                table.delegate_mut().picker = Some(picker);
                cx.notify();
            });
        }
    }

    pub(super) fn on_option_query_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => self.refresh_option_picker(cx),
            // Enter picks the best match, or creates the typed name.
            InputEvent::PressEnter { .. } => {
                let picker = self
                    .database_parts()
                    .and_then(|(_, table)| table.read(cx).delegate().picker.clone());
                match picker {
                    Some(p) if !p.choices.is_empty() => {
                        self.pick_database_option(p.choices[0].option, window, cx);
                    }
                    Some(p) => {
                        if let Some(name) = p.create {
                            self.create_database_option(&name, window, cx);
                        }
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }

    /// Picks `option` in the cell being edited. A select closes its picker; a multi-select stays open.
    pub(super) fn pick_database_option(
        &mut self,
        option: OptionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, page, column)) = self.editing_cell(cx) else {
            return;
        };
        if let Some(Err(err)) = self
            .editor
            .database_editor(id)
            .map(|e| e.toggle_option(page, column, option))
        {
            report("could not pick the option", err);
        }
        self.after_option_picked(id, column, window, cx);
    }

    /// A multi-select's picker stays open for the next option; a select's closes.
    pub(super) fn after_option_picked(
        &mut self,
        id: DatabaseId,
        column: ColumnId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let multiple = matches!(
            self.editor
                .database(id)
                .and_then(|db| db.column(column).ok())
                .map(|c| c.kind),
            Some(leaf_model::PropertyType::MultiSelect)
        );
        self.refresh_database_table(cx);
        if multiple {
            self.refresh_option_picker(cx);
        } else {
            self.set_editing(None, cx);
            self.focus_table(window, cx);
        }
    }

    /// Creates an option named `name` and picks it in the cell being edited.
    pub(super) fn create_database_option(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, page, column)) = self.editing_cell(cx) else {
            return;
        };
        if let Some(Err(err)) = self
            .editor
            .database_editor(id)
            .map(|e| e.create_option(page, column, name))
        {
            return report("could not add the option", err);
        }
        if let Some(d) = &self.database {
            let query = d.query.clone();
            query.update(cx, |q, cx| q.set_value("", window, cx));
        }
        self.after_option_picked(id, column, window, cx);
    }
}
