//! Drawing the action item pickers (E19-S03): the floating panel, the month calendar with its quick
//! picks and time field, and the priority menu. What a click does lives in the parent module.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, Div, Entity, Hsla, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, Pixels, SharedString, Stateful, Styled as _, anchored, deferred, div,
    px,
};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{DatePicker, priority_choices};

use super::{CONTEXT, PickerDown, PickerEnter, PickerEscape, PickerState, PickerUp};
use crate::shell::Shell;

/// The weekday initials over the calendar, Monday first.
const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

impl Shell {
    /// The open picker, floating under its chip.
    pub(in crate::shell) fn render_task_picker(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let open = self.blocks.task_picker.as_ref()?;
        let body = match &open.state {
            PickerState::Date { calendar, time } => Self::render_calendar(calendar, time, cx),
            PickerState::Priority { highlighted } => Self::render_priorities(*highlighted, cx),
        };
        let theme = cx.theme();
        let panel = v_flex()
            .id("task-picker")
            .debug_selector(|| "task-picker".into())
            .track_focus(&open.focus)
            .key_context(CONTEXT)
            .on_action(cx.listener(|s, _: &PickerUp, _, cx| s.picker_step(false, cx)))
            .on_action(cx.listener(|s, _: &PickerDown, _, cx| s.picker_step(true, cx)))
            .on_action(cx.listener(|s, _: &PickerEnter, w, cx| s.picker_enter(w, cx)))
            .on_action(cx.listener(|s, _: &PickerEscape, w, cx| s.close_task_picker(w, cx)))
            .on_mouse_down_out(cx.listener(|s, _: &MouseDownEvent, w, cx| s.close_task_picker(w, cx)))
            .p_2()
            .gap_2()
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .border_1()
            .border_color(theme.border)
            .rounded(theme.radius)
            .shadow_lg()
            .text_sm()
            .child(body);
        Some(
            deferred(
                anchored()
                    .position(open.at)
                    .snap_to_window_with_margin(px(8.0))
                    .child(panel),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    fn render_calendar(
        calendar: &DatePicker,
        time: &Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let (hover, muted, radius, border) =
            (theme.accent, theme.muted_foreground, theme.radius, theme.border);
        let button = |id: SharedString| picker_button(id, radius, hover);
        let quick = h_flex()
            .gap_1()
            .children(calendar.quick_picks().into_iter().map(|(label, date)| {
                button(format!("quick-{}", label.to_lowercase().replace(' ', "-")).into())
                    .border_1()
                    .border_color(border)
                    .child(label)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |s, _: &MouseDownEvent, w, cx| {
                            cx.stop_propagation();
                            s.date_picked(Some(date), w, cx);
                        }),
                    )
            }));
        let header = h_flex()
            .justify_between()
            .child(
                button("month-back".into())
                    .child(Icon::new(IconName::ChevronLeft).small())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|s, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            s.shift_month(false, cx);
                        }),
                    ),
            )
            .child(
                div()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(calendar.title()),
            )
            .child(
                button("month-next".into())
                    .child(Icon::new(IconName::ChevronRight).small())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|s, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            s.shift_month(true, cx);
                        }),
                    ),
            );
        let days = Self::render_days(calendar, cx);
        let footer = h_flex()
            .gap_2()
            .child(div().flex_1().child(Input::new(time).small()))
            .child(
                button("clear-date".into())
                    .text_color(muted)
                    .child("Clear")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|s, _: &MouseDownEvent, w, cx| {
                            cx.stop_propagation();
                            s.date_picked(None, w, cx);
                        }),
                    ),
            );
        v_flex()
            .w(px(244.0))
            .gap_2()
            .child(quick)
            .child(header)
            .child(days)
            .child(footer)
            .into_any_element()
    }

    /// The month's grid: weekday letters, then a row per week; today in green, the chosen day filled.
    fn render_days(calendar: &DatePicker, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (accent, on_accent, hover, muted, radius) = (
            theme.primary,
            theme.primary_foreground,
            theme.accent,
            theme.muted_foreground,
            theme.radius,
        );
        let cell = px(32.0);
        v_flex()
            .child(h_flex().children(WEEKDAYS.map(|d| {
                div()
                    .w(cell)
                    .flex()
                    .justify_center()
                    .text_xs()
                    .text_color(muted)
                    .child(d)
            })))
            .children(calendar.weeks().into_iter().map(|week| {
                h_flex().children(week.into_iter().map(|day| match day {
                    None => div().w(cell).h(cell).into_any_element(),
                    Some(date) => {
                        let selected = calendar.selected == Some(date);
                        let today = calendar.today == date;
                        picker_button(format!("day-{date}").into(), radius, hover)
                            .w(cell)
                            .h(cell)
                            .px_0()
                            .when(today && !selected, |d| {
                                d.text_color(accent).font_weight(gpui::FontWeight::BOLD)
                            })
                            .when(selected, |d| d.bg(accent).text_color(on_accent))
                            .child(date.day().to_string())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |s, _: &MouseDownEvent, w, cx| {
                                    cx.stop_propagation();
                                    s.date_picked(Some(date), w, cx);
                                }),
                            )
                            .into_any_element()
                    }
                }))
            }))
            .into_any_element()
    }

    fn render_priorities(highlighted: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (hover, muted, radius) = (theme.accent, theme.muted_foreground, theme.radius);
        let dark = theme.mode.is_dark();
        v_flex()
            .w(px(168.0))
            .children(
                priority_choices()
                    .into_iter()
                    .enumerate()
                    .map(|(ix, (priority, label))| {
                        let colour = priority.map_or(muted, |p| crate::theme::priority_colour(p, dark));
                        div()
                            .id(("priority", ix))
                            .debug_selector(move || format!("priority-{ix}"))
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .h(px(30.0))
                            .rounded(radius)
                            .cursor_pointer()
                            .when(ix == highlighted, |row| row.bg(hover))
                            .hover(|row| row.bg(hover))
                            .child(Icon::new(IconName::Flag).small().text_color(colour))
                            .child(label)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |s, _: &MouseDownEvent, w, cx| {
                                    cx.stop_propagation();
                                    s.priority_picked(priority, w, cx);
                                }),
                            )
                            .into_any_element()
                    }),
            )
            .into_any_element()
    }
}

/// A small button in the date picker: the hand pointer and a soft fill on hover.
fn picker_button(id: SharedString, radius: Pixels, hover: Hsla) -> Stateful<Div> {
    div()
        .id(id.clone())
        .debug_selector(move || id.to_string())
        .px_2()
        .h(px(26.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(radius)
        .cursor_pointer()
        .hover(|s| s.bg(hover))
}
