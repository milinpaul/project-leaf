//! The shell's side of the column menus (E09-S06): opening and closing them, forwarding each choice, dragged
//! headers and edges, and undo. What each choice writes is `DatabaseEditor`'s (`leaf_editor` `columns.rs`).

use gpui::{Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};
use leaf_core::{ColumnId, DatabaseId};
use leaf_model::PropertyType;

use super::super::{Shell, now_ms, report};
use super::column_menu::ColumnAction;
use super::header::{self, Slot};

impl Shell {
    /// The open database and its column at table column `col_ix`, if that is one of its columns.
    fn column_at(&self, col_ix: usize, cx: &gpui::App) -> Option<(DatabaseId, ColumnId)> {
        let (id, table) = self.database_parts()?;
        match header::slot(&table.read(cx).delegate().view, col_ix) {
            Slot::Column(column) => Some((id, column.id)),
            _ => None,
        }
    }

    /// The column whose menu is open.
    fn open_column(&self, cx: &gpui::App) -> Option<(DatabaseId, ColumnId)> {
        let (id, table) = self.database_parts()?;
        let column = table.read(cx).delegate().column_menu.as_ref()?.1.column;
        Some((id, column))
    }

    /// Opens the menu of the column at table column `col_ix`, with its name ready to edit.
    pub(super) fn open_column_menu(&mut self, col_ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.close_column_menus(window, cx);
        self.cancel_cell_edit(window, cx);
        let Some((id, column)) = self.column_at(col_ix, cx) else {
            return;
        };
        let menu = match self.editor.database_editor(id).map(|e| e.column_menu(column)) {
            Some(Ok(menu)) => menu,
            Some(Err(err)) => return report("could not open the column menu", err),
            None => return,
        };
        let Some(d) = &self.database else {
            return;
        };
        let name = d.column_name.clone();
        name.update(cx, |input, cx| {
            input.set_value(menu.name.clone(), window, cx);
            input.focus(window, cx);
        });
        d.table.update(cx, |table, cx| {
            table.delegate_mut().column_menu = Some((col_ix, menu));
            cx.notify();
        });
        cx.notify();
    }

    /// Opens the "+" menu: add a column, or show a hidden one.
    pub(super) fn open_add_column_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_column_menus(window, cx);
        self.cancel_cell_edit(window, cx);
        let Some((id, table)) = self.database_parts() else {
            return;
        };
        let hidden = match self.editor.database_editor(id).map(|e| e.hidden_columns()) {
            Some(Ok(hidden)) => hidden,
            Some(Err(err)) => return report("could not list the hidden columns", err),
            None => return,
        };
        table.update(cx, |table, cx| {
            table.delegate_mut().add_menu = Some(hidden);
            cx.notify();
        });
        // The table takes the keyboard, so Escape reaches the page and closes the menu.
        self.focus_table(window, cx);
        cx.notify();
    }

    /// Whether a column menu is open.
    pub(super) fn column_menu_open(&self, cx: &gpui::App) -> bool {
        self.database_parts().is_some_and(|(_, table)| {
            let delegate = table.read(cx).delegate();
            delegate.column_menu.is_some() || delegate.add_menu.is_some()
        })
    }

    /// Closes the column menus, keeping a name typed into a column's menu.
    pub(super) fn close_column_menus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.column_menu_open(cx) {
            return;
        }
        self.rename_open_column(cx);
        if let Some((_, table)) = self.database_parts() {
            table.update(cx, |table, cx| {
                let delegate = table.delegate_mut();
                delegate.column_menu = None;
                delegate.add_menu = None;
                cx.notify();
            });
        }
        self.refresh_database_table(cx);
        self.focus_table(window, cx);
    }

    /// Writes the name in the open column menu, when it changed.
    fn rename_open_column(&mut self, cx: &mut Context<Self>) {
        let Some((id, column)) = self.open_column(cx) else {
            return;
        };
        let Some(name) = self
            .database
            .as_ref()
            .map(|d| d.column_name.read(cx).value().to_string())
        else {
            return;
        };
        let unchanged = self
            .editor
            .database(id)
            .and_then(|db| db.column(column).ok())
            .is_some_and(|c| c.name == name.trim());
        if unchanged {
            return;
        }
        if let Some(Err(err)) = self
            .editor
            .database_editor(id)
            .map(|e| e.rename_column(column, &name))
        {
            report("could not rename the column", err);
        }
    }

    pub(super) fn on_column_name_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::PressEnter { .. } => self.close_column_menus(window, cx),
            InputEvent::Blur => {
                self.rename_open_column(cx);
                self.refresh_database_table(cx);
            }
            _ => {}
        }
    }

    /// Does what a column menu entry says. Moving keeps the menu open on the moved column; the rest close it.
    pub(super) fn column_menu_action(
        &mut self,
        action: &ColumnAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, column)) = self.open_column(cx) else {
            return;
        };
        self.rename_open_column(cx);
        let Some(editor) = self.editor.database_editor(id) else {
            return;
        };
        let done = match action {
            ColumnAction::Retype(kind) => editor.set_column_type(column, kind.clone()),
            ColumnAction::Left => editor.step_column(column, true).map(drop),
            ColumnAction::Right => editor.step_column(column, false).map(drop),
            ColumnAction::Hide => editor.set_column_hidden(column, true),
            ColumnAction::Delete => editor.delete_column(column),
        };
        if let Err(err) = done {
            report("could not change the column", err);
        }
        self.close_column_menus(window, cx);
        if matches!(action, ColumnAction::Left | ColumnAction::Right) {
            let at = self.database_parts().and_then(|(_, table)| {
                table
                    .read(cx)
                    .delegate()
                    .view
                    .columns
                    .iter()
                    .position(|c| c.id == column)
            });
            if let Some(at) = at {
                self.open_column_menu(at + 1, window, cx);
            }
        }
    }

    /// Adds a column of `kind` at the end and opens its menu to name it.
    pub(super) fn add_database_column(
        &mut self,
        kind: PropertyType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, _)) = self.database_parts() else {
            return;
        };
        self.close_column_menus(window, cx);
        match self.editor.database_editor(id).map(|e| e.add_column(kind)) {
            Some(Ok(_)) => {}
            Some(Err(err)) => return report("could not add a column", err),
            None => return,
        }
        self.refresh_database_table(cx);
        let shown = self
            .database_parts()
            .map_or(0, |(_, table)| table.read(cx).delegate().view.columns.len());
        self.open_column_menu(shown, window, cx);
    }

    /// Shows the hidden `column` again.
    pub(super) fn show_database_column(
        &mut self,
        column: ColumnId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, _)) = self.database_parts() else {
            return;
        };
        if let Some(Err(err)) = self
            .editor
            .database_editor(id)
            .map(|e| e.set_column_hidden(column, false))
        {
            report("could not show the column", err);
        }
        self.close_column_menus(window, cx);
    }

    /// A header dragged from table column `from` to `to`. The table has already moved it on screen; the database
    /// decides, and the table is redrawn from it either way.
    pub(super) fn move_dragged_column(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if let (Some((id, _)), Some(from), Some(to)) =
            (self.database_parts(), from.checked_sub(1), to.checked_sub(1))
            && let Some(Err(err)) = self
                .editor
                .database_editor(id)
                .map(|e| e.move_shown_column(from, to))
        {
            report("could not move the column", err);
        }
        self.refresh_database_table(cx);
    }

    /// Column edges dragged: saves each column whose width changed.
    pub(super) fn resize_columns(&mut self, widths: &[gpui::Pixels], cx: &mut Context<Self>) {
        let Some((id, table)) = self.database_parts() else {
            return;
        };
        let columns: Vec<_> = table
            .read(cx)
            .delegate()
            .view
            .columns
            .iter()
            .map(|c| (c.id, c.width))
            .collect();
        for ((column, width), new) in columns.into_iter().zip(widths.iter().skip(1)) {
            let new = f64::from(f32::from(*new));
            if (new - width).abs() < 0.5 {
                continue;
            }
            if let Some(Err(err)) = self
                .editor
                .database_editor(id)
                .map(|e| e.set_column_width(column, new))
            {
                report("could not resize the column", err);
            }
        }
        self.refresh_database_table(cx);
    }

    /// Undo or redo on a database page: the database's own history.
    pub(super) fn database_history(&mut self, undo: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some((page, _)) = self.open_database() else {
            return;
        };
        self.close_column_menus(window, cx);
        self.cancel_cell_edit(window, cx);
        let done = if undo {
            self.editor.undo_database(page, now_ms())
        } else {
            self.editor.redo_database(page, now_ms())
        };
        if let Err(err) = done {
            report("could not undo or redo", err);
        }
        self.refresh_database_table(cx);
    }
}
