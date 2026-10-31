//! The column menus (E09-S06), drawn from `leaf_editor::ColumnMenu`: a column's own menu (its name, its type, move
//! left or right, hide, delete) and the "+" menu (add a column of a type, or show a hidden one). Each choice is
//! forwarded to the shell; what it writes is `DatabaseEditor`'s.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Entity, Hsla, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, WeakEntity, div,
};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{COLUMN_TYPES, ColumnMenu, HiddenColumn, type_label};

use super::super::Shell;
use super::cells::panel;
use super::header::type_icon;

/// What a column menu entry does.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::shell) enum ColumnAction {
    /// Change the type.
    Retype(leaf_model::PropertyType),
    /// Move one shown column left.
    Left,
    /// Move one shown column right.
    Right,
    /// Hide from the table.
    Hide,
    /// Delete.
    Delete,
}

/// A small grey heading inside a menu.
fn heading(text: &'static str, cx: &App) -> AnyElement {
    div()
        .px_1p5()
        .pt_1p5()
        .pb_0p5()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text)
        .into_any_element()
}

/// One clickable menu entry.
fn entry(
    id: String,
    icon: IconName,
    label: impl Into<SharedString>,
    color: Option<Hsla>,
    on_click: impl Fn(&mut Shell, &mut gpui::Window, &mut gpui::Context<Shell>) + 'static,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let shell = shell.clone();
    let hover = cx.theme().muted;
    let selector = id.clone();
    h_flex()
        .id(SharedString::from(id))
        .debug_selector(move || selector.clone())
        .px_1p5()
        .py_1()
        .gap_2()
        .items_center()
        .rounded_sm()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .when_some(color, gpui::Styled::text_color)
        .child(Icon::new(icon).xsmall())
        .child(div().flex_1().child(label.into()))
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            if let Some(shell) = shell.upgrade() {
                shell.update(cx, |shell, cx| on_click(shell, window, cx));
            }
        })
}

/// Closes the menus when the pointer goes down anywhere else, or on Escape (bound in the cell editors' context,
/// inside the table's, which takes Escape for itself while a cell is selected).
fn closes_outside(panel: gpui::Div, shell: &WeakEntity<Shell>) -> gpui::Div {
    let outside = shell.clone();
    let escape = shell.clone();
    panel
        .key_context(super::CELL_CONTEXT)
        .on_action(move |_: &super::StopEditing, window, cx| {
            if let Some(shell) = escape.upgrade() {
                shell.update(cx, |shell, cx| shell.close_column_menus(window, cx));
            }
        })
        .on_mouse_down_out(move |_, window, cx| {
            if let Some(shell) = outside.upgrade() {
                shell.update(cx, |shell, cx| shell.close_column_menus(window, cx));
            }
        })
}

/// A column's menu: its name (edited in place), its type, then moving, hiding and deleting it.
pub(super) fn render_column_menu(
    menu: &ColumnMenu,
    name: &Entity<InputState>,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let mut list = closes_outside(panel(cx), shell)
        .debug_selector(|| "database-column-menu".into())
        .child(Input::new(name).small());
    if menu.retypable {
        list = list.child(heading("Type", cx));
        for (ix, kind) in COLUMN_TYPES.iter().enumerate() {
            let current = *kind == menu.kind;
            let action = ColumnAction::Retype(kind.clone());
            list = list.child(
                entry(
                    format!("database-column-type-{ix}"),
                    type_icon(kind),
                    type_label(kind),
                    None,
                    move |shell, window, cx| shell.column_menu_action(&action, window, cx),
                    shell,
                    cx,
                )
                .when(current, |row| {
                    row.child(Icon::new(IconName::Check).xsmall().text_color(muted))
                }),
            );
        }
    }
    list = list.child(div().my_1().h_px().bg(cx.theme().border));
    let actions = [
        (
            menu.can_move_left,
            "left",
            IconName::ArrowLeft,
            "Move left",
            ColumnAction::Left,
        ),
        (
            menu.can_move_right,
            "right",
            IconName::ArrowRight,
            "Move right",
            ColumnAction::Right,
        ),
        (true, "hide", IconName::EyeOff, "Hide", ColumnAction::Hide),
    ];
    for (shown, id, icon, label, action) in actions {
        if shown {
            list = list.child(entry(
                format!("database-column-{id}"),
                icon,
                label,
                None,
                move |shell, window, cx| shell.column_menu_action(&action, window, cx),
                shell,
                cx,
            ));
        }
    }
    list.child(entry(
        "database-column-delete".to_owned(),
        IconName::Trash,
        "Delete",
        Some(cx.theme().danger),
        |shell, window, cx| shell.column_menu_action(&ColumnAction::Delete, window, cx),
        shell,
        cx,
    ))
    .into_any_element()
}

/// The "+" menu: a column of each type, then the hidden columns to show again.
pub(super) fn render_add_menu(hidden: &[HiddenColumn], shell: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let mut list = closes_outside(panel(cx), shell)
        .debug_selector(|| "database-add-menu".into())
        .child(heading("Add a column", cx));
    for (ix, kind) in COLUMN_TYPES.iter().enumerate() {
        let kind = kind.clone();
        list = list.child(entry(
            format!("database-add-type-{ix}"),
            type_icon(&kind),
            type_label(&kind),
            None,
            move |shell, window, cx| shell.add_database_column(kind.clone(), window, cx),
            shell,
            cx,
        ));
    }
    if !hidden.is_empty() {
        list = list.child(heading("Hidden", cx));
        for (ix, column) in hidden.iter().enumerate() {
            let id = column.column;
            list = list.child(
                entry(
                    format!("database-show-{ix}"),
                    type_icon(&column.kind),
                    column.name.clone(),
                    None,
                    move |shell, window, cx| shell.show_database_column(id, window, cx),
                    shell,
                    cx,
                )
                .child(
                    Icon::new(IconName::Eye)
                        .xsmall()
                        .text_color(cx.theme().muted_foreground),
                ),
            );
        }
    }
    list.into_any_element()
}
