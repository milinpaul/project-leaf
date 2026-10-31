//! A database row opened as a page (E09-S08): under its title, the row's properties, each a name and its value,
//! then the page body as usual. A click on a value edits it in place, the way the table's cell does: text, URL and
//! number in a field (Enter or clicking away keeps it, Escape drops it), a checkbox toggles, select and multi-select
//! open the option picker, a date the calendar. What each writes is `DatabaseEditor`'s (`leaf_editor::CellEditor`,
//! with the row's page as the row), so the table shows the same value.

mod editing;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement as _, Styled, Subscription, Window, div, px,
};
use gpui_component::calendar::CalendarState;
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_core::{ColumnId, DatabaseId, PageId};
use leaf_editor::{CellDisplay, DatabaseEditor, OptionPicker, RowProperty};

use super::super::{Shell, report};
use super::cells::{PickerFor, render_calendar, render_picker};
use super::header::{type_icon, under};
use super::table::render_chip;
use super::{EditKind, ROW_CONTEXT, StopEditing, offset_minutes};

/// Width of the property names' column.
const NAME_WIDTH: f32 = 160.0;
/// Height of a property's line.
const LINE_HEIGHT: f32 = 32.0;

/// The open row page's property editing.
pub(in crate::shell) struct RowPage {
    page: PageId,
    pub(super) id: DatabaseId,
    /// The property being edited, and how.
    editing: Option<(ColumnId, EditKind)>,
    /// The option picker's contents while one is open.
    picker: Option<OptionPicker>,
    /// The open option's menu inside the picker, and its name field (E09-S11).
    pub(super) option_menu: Option<leaf_editor::OptionMenu>,
    pub(super) option_name: Entity<InputState>,
    text: Entity<InputState>,
    pub(super) query: Entity<InputState>,
    calendar: Entity<CalendarState>,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    /// The open page's database, when the open page is a row of one.
    fn open_row(&self) -> Option<(PageId, DatabaseId)> {
        let page = self.editor.selected()?;
        let database = self.editor.workspace().page(page).ok()?.row_of?;
        Some((page, database))
    }

    /// Makes sure the open row page has its editing state, loading its database the first time.
    fn ensure_row_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<(PageId, DatabaseId)> {
        let Some((page, id)) = self.open_row() else {
            self.row_page = None;
            return None;
        };
        if self.row_page.as_ref().is_some_and(|r| r.page == page) {
            return Some((page, id));
        }
        self.load_row_database(page);
        let text = cx.new(|cx| InputState::new(window, cx));
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search or create an option"));
        let calendar = cx.new(|cx| CalendarState::new(window, cx));
        let option_name = super::option_menu::name_input(window, cx);
        let subscriptions = vec![
            cx.subscribe_in(&option_name, window, |shell, _, event, window, cx| {
                shell.on_option_name_event(PickerFor::RowPage, event, window, cx);
            }),
            cx.subscribe_in(&text, window, Self::on_row_text_event),
            cx.subscribe_in(&query, window, Self::on_row_query_event),
            cx.subscribe_in(&calendar, window, Self::on_row_calendar_event),
        ];
        self.row_page = Some(RowPage {
            page,
            id,
            editing: None,
            picker: None,
            option_menu: None,
            option_name,
            text,
            query,
            calendar,
            _subscriptions: subscriptions,
        });
        Some((page, id))
    }

    fn row_editor(&mut self) -> Option<(PageId, &mut DatabaseEditor)> {
        let (page, id) = self.row_page.as_ref().map(|r| (r.page, r.id))?;
        self.editor.database_editor(id).map(|e| (page, e))
    }

    /// The open row page's properties, under its title; `None` for any other page.
    pub(in crate::shell) fn render_row_properties(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (page, id) = self.ensure_row_page(window, cx)?;
        let props = match self
            .editor
            .database_editor(id)?
            .row_properties(page, offset_minutes())
        {
            Ok(props) => props,
            Err(err) => {
                report("could not read the row's properties", err);
                return None;
            }
        };
        if props.is_empty() {
            return None;
        }
        let shell = cx.entity().downgrade();
        let lines: Vec<AnyElement> = props
            .iter()
            .enumerate()
            .map(|(ix, prop)| self.render_property(ix, prop, &shell, window, cx))
            .collect();
        Some(
            v_flex()
                .debug_selector(|| "row-properties".into())
                .key_context(ROW_CONTEXT)
                .on_action(cx.listener(|shell, _: &StopEditing, window, cx| {
                    if !shell.close_option_menu(PickerFor::RowPage, window, cx) {
                        shell.cancel_row_edit(cx);
                    }
                }))
                .pt_2()
                .pb_3()
                .mb_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .children(lines)
                .into_any_element(),
        )
    }

    fn render_property(
        &self,
        ix: usize,
        prop: &RowProperty,
        shell: &gpui::WeakEntity<Shell>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let Some(row) = &self.row_page else {
            return div().into_any_element();
        };
        let editing = row
            .editing
            .filter(|(column, _)| *column == prop.column)
            .map(|(_, kind)| kind);
        let column = prop.column;
        let value: AnyElement = match editing {
            Some(EditKind::Text) => Input::new(&row.text).appearance(false).small().into_any_element(),
            _ => render_value(&prop.display, cx),
        };
        let popover = match editing {
            Some(EditKind::Options) => row.picker.as_ref().map(|picker| {
                let close = shell.clone();
                under(
                    div()
                        .on_mouse_down_out(move |_, window, cx| {
                            if let Some(shell) = close.upgrade() {
                                shell.update(cx, |shell, cx| shell.close_row_edit(window, cx));
                            }
                        })
                        .child(render_picker(
                            picker,
                            &row.query,
                            row.option_menu.as_ref().map(|menu| (menu, &row.option_name)),
                            shell,
                            PickerFor::RowPage,
                            cx,
                        ))
                        .into_any_element(),
                )
            }),
            Some(EditKind::Date) => {
                let close = shell.clone();
                Some(under(
                    div()
                        .on_mouse_down_out(move |_, window, cx| {
                            if let Some(shell) = close.upgrade() {
                                shell.update(cx, |shell, cx| shell.close_row_edit(window, cx));
                            }
                        })
                        .child(render_calendar(&row.calendar, window, cx))
                        .into_any_element(),
                ))
            }
            _ => None,
        };
        let open = shell.clone();
        h_flex()
            .min_h(px(LINE_HEIGHT))
            .items_center()
            .child(
                h_flex()
                    .w(px(NAME_WIDTH))
                    .flex_none()
                    .gap_1p5()
                    .items_center()
                    .text_color(theme.muted_foreground)
                    .child(Icon::new(type_icon(&prop.kind)).xsmall())
                    .child(div().truncate().child(prop.name.clone())),
            )
            .child(
                h_flex()
                    .id(SharedString::from(format!("row-property-{ix}")))
                    .debug_selector(move || format!("row-property-{ix}"))
                    .relative()
                    .flex_1()
                    .min_h(px(LINE_HEIGHT - 4.0))
                    .px_1p5()
                    .items_center()
                    .rounded_sm()
                    .when(editing.is_none(), |v| {
                        v.cursor_pointer().hover(|s| s.bg(cx.theme().muted))
                    })
                    .child(value)
                    .children(popover)
                    .on_click(move |_, window, cx| {
                        if let Some(shell) = open.upgrade() {
                            shell.update(cx, |shell, cx| shell.begin_row_edit(column, window, cx));
                        }
                    }),
            )
            .into_any_element()
    }
}

/// A property's value as the page shows it: like the table's cell, with "Empty" for nothing yet.
fn render_value(display: &CellDisplay, cx: &App) -> AnyElement {
    let theme = cx.theme();
    match display {
        CellDisplay::Empty => div()
            .text_color(theme.muted_foreground)
            .child("Empty")
            .into_any_element(),
        CellDisplay::Text(text) | CellDisplay::Date(text) | CellDisplay::Number(text) => {
            div().child(text.clone()).into_any_element()
        }
        CellDisplay::Url(text) => div()
            .text_color(theme.link)
            .underline()
            .child(text.clone())
            .into_any_element(),
        CellDisplay::Checkbox(on) => div()
            .size(px(16.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .border_color(theme.border)
            .when(*on, |b| {
                b.bg(theme.primary).border_color(theme.primary).child(
                    Icon::new(IconName::Check)
                        .xsmall()
                        .text_color(theme.primary_foreground),
                )
            })
            .into_any_element(),
        CellDisplay::Options(chips) => h_flex()
            .gap_1()
            .flex_wrap()
            .children(chips.iter().map(|chip| render_chip(chip, cx)))
            .into_any_element(),
        CellDisplay::Unknown => div()
            .text_color(theme.muted_foreground)
            .child("—")
            .into_any_element(),
    }
}
