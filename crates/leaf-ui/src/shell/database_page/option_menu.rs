//! An option's menu inside the option picker (E09-S11): its name, the nine colours and "Delete", in place of the
//! list while it is open; and dragging an option to a new place in the list. The same for a table cell's picker
//! and a row page's. What each choice writes is `DatabaseEditor`'s (`leaf_editor` `database_view/options.rs`); this
//! module draws and forwards.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement as _, Styled, WeakEntity, Window, div, px,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit_assets::IconName;
use leaf_core::{ColumnId, DatabaseId, OptionId};
use leaf_editor::{Chip, OptionMenu};

use super::super::{Shell, report};
use super::cells::{PickerFor, panel};
use super::table::render_chip;

/// An option being dragged in the picker, and where it was.
#[derive(Debug, Clone)]
pub(super) struct OptionDrag {
    pub(super) option: OptionId,
    chip: Chip,
}

impl OptionDrag {
    pub(super) fn new(chip: Chip) -> Self {
        Self {
            option: chip.option,
            chip,
        }
    }
}

impl Render for OptionDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().opacity(0.85).child(render_chip(&self.chip, cx))
    }
}

/// The menu of the option `menu` names, with its name field.
pub(super) fn render_option_menu(
    menu: &OptionMenu,
    name: &Entity<InputState>,
    shell: &WeakEntity<Shell>,
    picking_for: PickerFor,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let (muted, hover, danger) = (theme.muted_foreground, theme.muted, theme.danger);
    let act = |shell: &WeakEntity<Shell>, f: fn(&mut Shell, PickerFor, &mut Window, &mut Context<Shell>)| {
        let shell = shell.clone();
        move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
            if let Some(shell) = shell.upgrade() {
                shell.update(cx, |shell, cx| f(shell, picking_for, window, cx));
            }
        }
    };
    let mut list = panel(cx)
        .debug_selector(|| "database-option-menu".into())
        .child(Input::new(name).small())
        .child(div().px_1p5().pt_1().text_xs().text_color(muted).child("Colours"));
    for (ix, (color, current)) in menu.colors.iter().enumerate() {
        let color = *color;
        let shell = shell.clone();
        let swatch = Chip {
            option: menu.option,
            name: label(color),
            color: color.into(),
        };
        list = list.child(
            h_flex()
                .id(SharedString::from(format!("database-option-colour-{ix}")))
                .debug_selector(move || format!("database-option-colour-{ix}"))
                .px_1p5()
                .py_0p5()
                .gap_2()
                .rounded_sm()
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child(render_chip(&swatch, cx))
                .child(div().flex_1())
                .when(*current, |row| {
                    row.child(Icon::new(IconName::Check).xsmall().text_color(muted))
                })
                .on_click(move |_, window, cx| {
                    if let Some(shell) = shell.upgrade() {
                        shell.update(cx, |shell, cx| {
                            shell.recolour_option(color, picking_for, window, cx);
                        });
                    }
                }),
        );
    }
    list.child(div().my_0p5().h(px(1.0)).bg(theme.border))
        .child(
            h_flex()
                .id("database-option-delete")
                .debug_selector(|| "database-option-delete".into())
                .px_1p5()
                .py_1()
                .gap_2()
                .rounded_sm()
                .cursor_pointer()
                .text_color(danger)
                .hover(move |s| s.bg(hover))
                .child(Icon::new(IconName::Trash).xsmall())
                .child("Delete")
                .on_click(act(shell, Shell::delete_open_option)),
        )
        .into_any_element()
}

/// The menu's name for a palette colour.
fn label(color: &str) -> String {
    let mut chars = color.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The field an option is renamed in, for a view or a row page.
pub(super) fn name_input(window: &mut Window, cx: &mut Context<Shell>) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).placeholder("Option name"))
}

impl Shell {
    /// The database and column whose picker is open.
    fn option_target(&self, picking_for: PickerFor, cx: &App) -> Option<(DatabaseId, ColumnId)> {
        match picking_for {
            PickerFor::Table => self.editing_cell(cx).map(|(id, _, column)| (id, column)),
            PickerFor::RowPage => Some((self.row_page.as_ref()?.id, self.editing_column()?)),
        }
    }

    /// The open option menu and its name field.
    fn option_menu_state(
        &self,
        picking_for: PickerFor,
        cx: &App,
    ) -> Option<(Option<OptionMenu>, Entity<InputState>)> {
        match picking_for {
            PickerFor::Table => {
                let (_, table) = self.database_parts()?;
                let delegate = table.read(cx).delegate();
                Some((delegate.option_menu.clone(), delegate.option_name.clone()))
            }
            PickerFor::RowPage => {
                let row = self.row_page.as_ref()?;
                Some((row.option_menu.clone(), row.option_name.clone()))
            }
        }
    }

    fn set_option_menu(&mut self, picking_for: PickerFor, menu: Option<OptionMenu>, cx: &mut Context<Self>) {
        match picking_for {
            PickerFor::Table => {
                if let Some((_, table)) = self.database_parts() {
                    table.update(cx, |table, cx| {
                        table.delegate_mut().option_menu = menu;
                        cx.notify();
                    });
                }
            }
            PickerFor::RowPage => {
                if let Some(row) = &mut self.row_page {
                    row.option_menu = menu;
                }
            }
        }
        cx.notify();
    }

    /// Redraws the picker, and the table or the row's properties, after an option changed.
    fn option_changed(&mut self, picking_for: PickerFor, cx: &mut Context<Self>) {
        match picking_for {
            PickerFor::Table => {
                self.refresh_database_table(cx);
                self.refresh_option_picker(cx);
            }
            PickerFor::RowPage => self.refresh_row_picker(cx),
        }
        cx.notify();
    }

    /// The "⋯" on an option: opens its menu, with its name ready to edit.
    pub(super) fn open_option_menu(
        &mut self,
        option: OptionId,
        picking_for: PickerFor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, column)) = self.option_target(picking_for, cx) else {
            return;
        };
        let menu = match self
            .editor
            .database_editor(id)
            .map(|e| e.option_menu(column, option))
        {
            Some(Ok(menu)) => menu,
            Some(Err(err)) => return report("could not open the option's menu", err),
            None => return,
        };
        if let Some((_, name)) = self.option_menu_state(picking_for, cx) {
            name.update(cx, |input, cx| {
                input.set_value(menu.name.clone(), window, cx);
                input.focus(window, cx);
            });
        }
        self.set_option_menu(picking_for, Some(menu), cx);
    }

    /// Closes the option menu, keeping a name typed into it, and gives the picker's search field the keyboard back.
    /// Returns whether one was open.
    pub(super) fn close_option_menu(
        &mut self,
        picking_for: PickerFor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(self.option_menu_state(picking_for, cx), Some((Some(_), _))) {
            return false;
        }
        self.rename_open_option(picking_for, cx);
        self.back_to_option_list(picking_for, window, cx);
        true
    }

    /// Closes the option menu as it is and gives the picker's search field the keyboard back.
    fn back_to_option_list(&mut self, picking_for: PickerFor, window: &mut Window, cx: &mut Context<Self>) {
        self.set_option_menu(picking_for, None, cx);
        let query = match picking_for {
            PickerFor::Table => self.database_view().map(|d| d.query.clone()),
            PickerFor::RowPage => self.row_page.as_ref().map(|r| r.query.clone()),
        };
        if let Some(query) = query {
            query.update(cx, |input, cx| input.focus(window, cx));
        }
        self.option_changed(picking_for, cx);
    }

    /// Writes the name in the option menu, when it changed (a blank or taken name keeps the old one).
    fn rename_open_option(&mut self, picking_for: PickerFor, cx: &mut Context<Self>) {
        let (Some((id, column)), Some((Some(menu), name))) = (
            self.option_target(picking_for, cx),
            self.option_menu_state(picking_for, cx),
        ) else {
            return;
        };
        let typed = name.read(cx).value().to_string();
        match self
            .editor
            .database_editor(id)
            .map(|e| e.rename_option(column, menu.option, &typed))
        {
            Some(Ok(true)) => self.option_changed(picking_for, cx),
            Some(Err(err)) => report("could not rename the option", err),
            _ => {}
        }
    }

    pub(super) fn on_option_name_event(
        &mut self,
        picking_for: PickerFor,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::PressEnter { .. } => {
                self.close_option_menu(picking_for, window, cx);
            }
            InputEvent::Blur => self.rename_open_option(picking_for, cx),
            _ => {}
        }
    }

    /// A colour in the option menu.
    fn recolour_option(
        &mut self,
        color: &str,
        picking_for: PickerFor,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some((id, column)), Some((Some(menu), _))) = (
            self.option_target(picking_for, cx),
            self.option_menu_state(picking_for, cx),
        ) else {
            return;
        };
        match self
            .editor
            .database_editor(id)
            .map(|e| e.set_option_color(column, menu.option, color))
        {
            Some(Err(err)) => return report("could not change the option's colour", err),
            Some(Ok(true)) => {}
            _ => return,
        }
        if let Some(Ok(menu)) = self
            .editor
            .database_editor(id)
            .map(|e| e.option_menu(column, menu.option))
        {
            self.set_option_menu(picking_for, Some(menu), cx);
        }
        self.option_changed(picking_for, cx);
    }

    /// "Delete" in the option menu: the option leaves the list and every cell.
    fn delete_open_option(&mut self, picking_for: PickerFor, window: &mut Window, cx: &mut Context<Self>) {
        let (Some((id, column)), Some((Some(menu), _))) = (
            self.option_target(picking_for, cx),
            self.option_menu_state(picking_for, cx),
        ) else {
            return;
        };
        if let Some(Err(err)) = self
            .editor
            .database_editor(id)
            .map(|e| e.delete_option(column, menu.option))
        {
            report("could not delete the option", err);
        }
        // Not `close_option_menu`: the name field's text belongs to an option that is gone.
        self.back_to_option_list(picking_for, window, cx);
    }

    /// An option dropped on the option at `index` of the picker's list: it takes that place.
    pub(super) fn drop_option(
        &mut self,
        option: OptionId,
        index: usize,
        picking_for: PickerFor,
        cx: &mut Context<Self>,
    ) {
        let Some((id, column)) = self.option_target(picking_for, cx) else {
            return;
        };
        match self
            .editor
            .database_editor(id)
            .map(|e| e.move_option(column, option, index))
        {
            Some(Ok(true)) => self.option_changed(picking_for, cx),
            Some(Err(err)) => report("could not move the option", err),
            _ => {}
        }
    }
}
