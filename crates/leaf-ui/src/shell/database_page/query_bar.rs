//! The bar above a database's table (E09-S07): a chip for each sort and filter, "Sort" and "Filter" buttons, and how
//! many rows the filters leave. Each opens a pop-up under it: columns to add, the sorts, or one filter's condition
//! and value. Drawn from `leaf_editor::QueryBar`; every choice goes to the shell (`query.rs`).

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, WeakEntity, div, px,
};
use gpui_component::calendar::{Calendar, CalendarState};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_core::ColumnId;
use leaf_editor::{FilterChip, FilterInput, QueryBar, direction_label};
use leaf_model::{CellValue, PropertyType, SelectOption};

use super::super::Shell;
use super::cells::panel;
use super::column_menu::{closes_outside, entry, heading};
use super::header::{type_icon, under};
use super::offset_minutes;
use super::query::QueryMenu;
use super::table::render_chip;

/// A rounded button in the bar.
fn pill(id: impl Into<SharedString>, cx: &App) -> gpui::Stateful<gpui::Div> {
    let id = id.into();
    let selector = id.to_string();
    let hover = cx.theme().muted;
    h_flex()
        .id(id)
        .debug_selector(move || selector.clone())
        .relative()
        .h(px(26.0))
        .px_2()
        .gap_1()
        .items_center()
        .rounded_full()
        .text_sm()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
}

/// Opens `menu` when clicked.
fn opens(
    pill: gpui::Stateful<gpui::Div>,
    menu: QueryMenu,
    shell: &WeakEntity<Shell>,
) -> gpui::Stateful<gpui::Div> {
    let shell = shell.clone();
    pill.on_click(move |_, window, cx| {
        if let Some(shell) = shell.upgrade() {
            shell.update(cx, |shell, cx| shell.open_query_menu(menu, window, cx));
        }
    })
}

/// A column list for adding a sort or a filter.
fn column_list(
    title: &'static str,
    prefix: &'static str,
    columns: &[(ColumnId, String, PropertyType)],
    pick: fn(&mut Shell, ColumnId, &mut gpui::Window, &mut Context<Shell>),
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> AnyElement {
    let mut list = closes_outside(panel(cx), shell)
        .debug_selector(move || format!("{prefix}-menu"))
        .child(heading(title, cx));
    for (ix, (column, name, kind)) in columns.iter().enumerate() {
        let column = *column;
        list = list.child(entry(
            format!("{prefix}-{ix}"),
            type_icon(kind),
            name.clone(),
            None,
            move |shell, window, cx| pick(shell, column, window, cx),
            shell,
            cx,
        ));
    }
    if columns.is_empty() {
        list = list.child(
            div()
                .px_1p5()
                .py_1()
                .text_color(cx.theme().muted_foreground)
                .child("Nothing left to add"),
        );
    }
    list.into_any_element()
}

/// The sorts: each with its direction, which turns it round, and a remove button; then "Add a sort".
fn sorts_menu(bar: &QueryBar, shell: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let mut list = closes_outside(panel(cx), shell)
        .debug_selector(|| "database-sorts-menu".into())
        .child(heading("Sort", cx));
    for (ix, sort) in bar.sorts.iter().enumerate() {
        let (turn, remove) = (shell.clone(), shell.clone());
        let descending = sort.descending;
        list = list.child(
            h_flex()
                .px_1p5()
                .py_0p5()
                .gap_2()
                .items_center()
                .child(
                    Icon::new(type_icon(&sort.kind))
                        .xsmall()
                        .text_color(theme.muted_foreground),
                )
                .child(div().flex_1().truncate().child(sort.name.clone()))
                .child(
                    pill(format!("database-sort-turn-{ix}"), cx)
                        .border_1()
                        .border_color(theme.border)
                        .child(direction_label(&sort.kind, descending))
                        .on_click(move |_, _, cx| {
                            if let Some(shell) = turn.upgrade() {
                                shell.update(cx, |shell, cx| shell.turn_sort(ix, !descending, cx));
                            }
                        }),
                )
                .child(
                    pill(format!("database-sort-remove-{ix}"), cx)
                        .child(Icon::new(IconName::X).xsmall())
                        .on_click(move |_, _, cx| {
                            if let Some(shell) = remove.upgrade() {
                                shell.update(cx, |shell, cx| shell.remove_sort_at(ix, cx));
                            }
                        }),
                ),
        );
    }
    if !bar.sortable.is_empty() {
        let shell_add = shell.clone();
        list = list.child(entry(
            "database-sort-add".to_owned(),
            IconName::Plus,
            "Add a sort",
            Some(theme.muted_foreground),
            |shell, window, cx| shell.open_query_menu(QueryMenu::AddSort, window, cx),
            &shell_add,
            cx,
        ));
    }
    list.into_any_element()
}

/// What a filter's value editor draws with: the column's options, the text field and the calendar.
struct ValueInputs {
    options: Vec<SelectOption>,
    text: Entity<InputState>,
    calendar: Entity<CalendarState>,
}

/// One filter: its conditions as pills, then its value's editor, then "Delete filter".
fn filter_menu(
    index: usize,
    chip: &FilterChip,
    inputs: &ValueInputs,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let mut ops = h_flex().px_1p5().py_1().gap_1().flex_wrap();
    for (ix, op) in chip.ops.iter().enumerate() {
        let shell = shell.clone();
        let chosen = *op == chip.op;
        let pick = op.clone();
        ops = ops.child(
            pill(format!("database-filter-op-{ix}"), cx)
                .border_1()
                .border_color(theme.border)
                .when(chosen, |p| p.bg(theme.secondary).border_color(theme.ring))
                .child(leaf_editor::op_label(op, &chip.kind))
                .on_click(move |_, _, cx| {
                    if let Some(shell) = shell.upgrade() {
                        shell.update(cx, |shell, cx| shell.choose_filter_op(index, pick.clone(), cx));
                    }
                }),
        );
    }
    let mut list = closes_outside(panel(cx), shell)
        .debug_selector(|| "database-filter-menu".into())
        .child(
            h_flex()
                .px_1p5()
                .pt_1()
                .gap_1p5()
                .items_center()
                .text_color(theme.muted_foreground)
                .child(Icon::new(type_icon(&chip.kind)).xsmall())
                .child(chip.name.clone()),
        )
        .child(ops);
    if let Some(editor) = filter_value(index, chip, inputs, shell, cx) {
        list = list.child(editor);
    }
    list.child(div().my_1().h_px().bg(theme.border))
        .child(entry(
            "database-filter-delete".to_owned(),
            IconName::Trash,
            "Delete filter",
            Some(theme.danger),
            move |shell, _, cx| shell.remove_filter_at(index, cx),
            shell,
            cx,
        ))
        .into_any_element()
}

/// The editor for filter `index`'s value: a text field, the column's options, a calendar, or ticked / not ticked.
fn filter_value(
    index: usize,
    chip: &FilterChip,
    inputs: &ValueInputs,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Option<AnyElement> {
    let theme = cx.theme();
    match &chip.input {
        FilterInput::Text(_) | FilterInput::Number(_) => Some(
            div()
                .px_1()
                .pb_1()
                .child(Input::new(&inputs.text).small())
                .into_any_element(),
        ),
        FilterInput::Option(picked) => {
            let mut list = v_flex();
            for (ix, option) in inputs.options.iter().enumerate() {
                let id = option.id;
                let shell = shell.clone();
                let chosen = *picked == Some(id);
                let chip = leaf_editor::Chip {
                    option: id,
                    name: option.name.clone(),
                    color: option.color.clone(),
                };
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("database-filter-option-{ix}")))
                        .debug_selector(move || format!("database-filter-option-{ix}"))
                        .px_1p5()
                        .py_1()
                        .gap_2()
                        .rounded_sm()
                        .cursor_pointer()
                        .hover(|s| s.bg(cx.theme().muted))
                        .child(render_chip(&chip, cx))
                        .child(div().flex_1())
                        .when(chosen, |row| {
                            row.child(
                                Icon::new(IconName::Check)
                                    .xsmall()
                                    .text_color(theme.muted_foreground),
                            )
                        })
                        .on_click(move |_, _, cx| {
                            if let Some(shell) = shell.upgrade() {
                                let value = (!chosen).then_some(CellValue::Option(id));
                                shell.update(cx, |shell, cx| shell.choose_filter_value(index, value, cx));
                            }
                        }),
                );
            }
            Some(list.into_any_element())
        }
        FilterInput::Date(_) => Some(Calendar::new(&inputs.calendar).small().into_any_element()),
        FilterInput::Checkbox(on) => {
            let mut row = h_flex().px_1p5().pb_1().gap_1();
            for (ix, (label, value)) in [("Ticked", true), ("Not ticked", false)].into_iter().enumerate() {
                let shell = shell.clone();
                row = row.child(
                    pill(format!("database-filter-check-{ix}"), cx)
                        .border_1()
                        .border_color(theme.border)
                        .when(*on == value, |p| p.bg(theme.secondary).border_color(theme.ring))
                        .child(label)
                        .on_click(move |_, _, cx| {
                            if let Some(shell) = shell.upgrade() {
                                let value = Some(CellValue::Checkbox(value));
                                shell.update(cx, |shell, cx| shell.choose_filter_value(index, value, cx));
                            }
                        }),
                );
            }
            Some(row.into_any_element())
        }
        FilterInput::None => None,
    }
}

impl Shell {
    /// A chip for each filter, the open one with its pop-up.
    #[expect(
        clippy::too_many_arguments,
        reason = "the bar's pieces, passed down rather than read twice"
    )]
    fn filter_chips(
        &self,
        id: leaf_core::DatabaseId,
        bar: &QueryBar,
        menu: Option<QueryMenu>,
        text: &Entity<InputState>,
        calendar: &Entity<CalendarState>,
        shell: &WeakEntity<Shell>,
        cx: &App,
    ) -> Vec<AnyElement> {
        let theme = cx.theme();
        let mut chips = Vec::new();
        for (ix, chip) in bar.filters.iter().enumerate() {
            let label = if chip.value_label.is_empty() && !matches!(chip.input, FilterInput::None) {
                chip.name.clone()
            } else {
                format!("{} {} {}", chip.name, chip.op_label, chip.value_label)
                    .trim_end()
                    .to_owned()
            };
            let options = match &chip.input {
                FilterInput::Option(_) => self
                    .editor
                    .database(id)
                    .and_then(|db| db.column(chip.column).ok())
                    .map(|c| c.options)
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            let popover = (menu == Some(QueryMenu::Filter(ix))).then(|| {
                let inputs = ValueInputs {
                    options,
                    text: text.clone(),
                    calendar: calendar.clone(),
                };
                under(filter_menu(ix, chip, &inputs, shell, cx))
            });
            chips.push(
                opens(
                    pill(format!("database-filter-chip-{ix}"), cx),
                    QueryMenu::Filter(ix),
                    shell,
                )
                .bg(theme.secondary)
                .child(Icon::new(IconName::ListFilter).xsmall())
                .child(div().max_w(px(260.0)).truncate().child(label))
                .children(popover)
                .into_any_element(),
            );
        }
        chips
    }

    /// The bar above the table, with its open pop-up.
    pub(super) fn render_query_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some((id, table)) = self.database_parts() else {
            return div().into_any_element();
        };
        let Some(bar) = self
            .editor
            .database_editor(id)
            .and_then(|e| e.query_bar(offset_minutes()).ok())
        else {
            return div().into_any_element();
        };
        let Some(d) = self.database_view() else {
            return div().into_any_element();
        };
        let (text, calendar, menu) = (d.filter_text.clone(), d.filter_calendar.clone(), d.query_menu);
        let (shown, total) = {
            let view = &table.read(cx).delegate().view;
            (view.rows.len(), view.total_rows)
        };
        let shell = cx.entity().downgrade();
        let theme = cx.theme();
        let mut chips = h_flex().flex_1().gap_1().flex_wrap().items_center();
        for (ix, sort) in bar.sorts.iter().enumerate() {
            let arrow = if sort.descending {
                IconName::ArrowDown
            } else {
                IconName::ArrowUp
            };
            chips = chips.child(
                opens(
                    pill(format!("database-sort-chip-{ix}"), cx),
                    QueryMenu::Sorts,
                    &shell,
                )
                .bg(theme.secondary)
                .child(Icon::new(arrow).xsmall())
                .child(sort.name.clone()),
            );
        }
        chips = chips.children(self.filter_chips(id, &bar, menu, &text, &calendar, &shell, cx));
        let count = (!bar.filters.is_empty() && shown < total).then(|| {
            div()
                .debug_selector(|| "database-row-count".into())
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(format!("{shown} of {total}"))
        });
        let sort_menu = match menu {
            Some(QueryMenu::AddSort) => Some(under(column_list(
                "Sort by",
                "database-sort-column",
                &bar.sortable,
                Shell::add_sort_by,
                &shell,
                cx,
            ))),
            Some(QueryMenu::Sorts) => Some(under(sorts_menu(&bar, &shell, cx))),
            _ => None,
        };
        let filter_menu = (menu == Some(QueryMenu::AddFilter)).then(|| {
            under(column_list(
                "Filter by",
                "database-filter-column",
                &bar.filterable,
                Shell::add_filter_on,
                &shell,
                cx,
            ))
        });
        let sort_target = if bar.sorts.is_empty() {
            QueryMenu::AddSort
        } else {
            QueryMenu::Sorts
        };
        let muted = theme.muted_foreground;
        h_flex()
            .debug_selector(|| "database-query-bar".into())
            .min_h(px(30.0))
            .gap_2()
            .items_center()
            .child(chips)
            .children(count)
            .child(
                opens(pill("database-sort-button", cx), sort_target, &shell)
                    .text_color(muted)
                    .child(Icon::new(IconName::ArrowUpDown).xsmall())
                    .child("Sort")
                    .children(sort_menu),
            )
            .child(
                opens(pill("database-filter-button", cx), QueryMenu::AddFilter, &shell)
                    .text_color(muted)
                    .child(Icon::new(IconName::ListFilter).xsmall())
                    .child("Filter")
                    .children(filter_menu),
            )
            .into_any_element()
    }
}
