//! The pop-up editors of a database cell: the option picker (select and multi-select) and the date calendar. They
//! draw the editor's lists and forward choices to the shell; what a choice does is `DatabaseEditor`'s.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Entity, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, WeakEntity, Window, div, px,
};
use gpui_component::calendar::{Calendar, CalendarState};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{Chip, OptionPicker};

use super::super::Shell;

/// Width of the pop-up editors.
const PANEL_WIDTH: f32 = 260.0;

fn panel(cx: &App) -> gpui::Div {
    let theme = cx.theme();
    v_flex()
        .w(px(PANEL_WIDTH))
        .p_1()
        .gap_0p5()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .bg(theme.popover)
        .shadow_md()
        .occlude()
}

/// The option picker: a search field, the matching options (ticked when the cell has them), and "Create …" for
/// a name that isn't there.
pub(super) fn render_picker(
    picker: &OptionPicker,
    query: &Entity<InputState>,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let hover = theme.muted;
    let mut list = panel(cx)
        .debug_selector(|| "database-picker".into())
        .child(Input::new(query).small());
    for (ix, choice) in picker.choices.iter().enumerate() {
        let shell = shell.clone();
        let option = choice.option;
        let chip = Chip {
            option: choice.option,
            name: choice.name.clone(),
            color: choice.color.clone(),
        };
        list = list.child(
            h_flex()
                .id(SharedString::from(format!("database-option-{ix}")))
                .debug_selector(move || format!("database-option-{ix}"))
                .px_1p5()
                .py_1()
                .gap_2()
                .rounded_sm()
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child(super::table::render_chip(&chip, cx))
                .child(div().flex_1())
                .when(choice.picked, |row| {
                    row.child(Icon::new(IconName::Check).xsmall().text_color(muted))
                })
                .on_click(move |_, window, cx| {
                    if let Some(shell) = shell.upgrade() {
                        shell.update(cx, |shell, cx| shell.pick_database_option(option, window, cx));
                    }
                }),
        );
    }
    if let Some(name) = &picker.create {
        let shell = shell.clone();
        let name = name.clone();
        list = list.child(
            h_flex()
                .id("database-create-option")
                .debug_selector(|| "database-create-option".into())
                .px_1p5()
                .py_1()
                .gap_2()
                .rounded_sm()
                .cursor_pointer()
                .text_color(muted)
                .hover(move |s| s.bg(hover))
                .child(Icon::new(IconName::Plus).xsmall())
                .child(format!("Create \u{201c}{name}\u{201d}"))
                .on_click(move |_, window, cx| {
                    if let Some(shell) = shell.upgrade() {
                        shell.update(cx, |shell, cx| shell.create_database_option(&name, window, cx));
                    }
                }),
        );
    }
    if picker.choices.is_empty() && picker.create.is_none() {
        list = list.child(
            div()
                .px_1p5()
                .py_1()
                .text_color(muted)
                .child("Type to add an option"),
        );
    }
    list
}

/// The date calendar, with a "Clear" entry.
pub(super) fn render_calendar(calendar: &Entity<CalendarState>, _: &mut Window, cx: &App) -> AnyElement {
    panel(cx)
        .debug_selector(|| "database-calendar".into())
        .child(Calendar::new(calendar).small())
        .into_any_element()
}
