//! How an action item looks in the page (E19-S03, ADR-068): a round check before the text, and after
//! it a date chip (red once overdue) and a priority flag. An item with no date or priority shows faint
//! "add" buttons only while the pointer is over it, so a plain task stays calm. Clicking the check
//! ticks it; clicking the chip or the flag opens its picker (`task_pickers`). The words and the
//! overdue flag come from `leaf_editor::due_label` and `is_overdue`, given today from the clock.

use chrono::{Datelike as _, Local, Timelike as _};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    SharedString, Styled, div, point, px,
};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit_assets::IconName;
use leaf_core::BlockId;
use leaf_editor::{due_label, is_overdue};
use leaf_model::{DueDate, DueTime, Priority};

use super::Shell;
use super::block_menu::ROW_GROUP;
use super::task_pickers::TaskPickerKind;
use crate::theme;

/// The round check's size, in pixels: the same footprint as a to-do's box.
const CHECK_PX: f32 = 16.0;

/// Today's date and the time now on this computer's clock, for the date words and overdue. `None`
/// only for a clock outside the years a due date can have.
#[must_use]
pub(super) fn local_now() -> Option<(DueDate, DueTime)> {
    let now = Local::now();
    let date = DueDate::new(
        u16::try_from(now.year()).ok()?,
        u8::try_from(now.month()).ok()?,
        u8::try_from(now.day()).ok()?,
    );
    let time = DueTime::new(u8::try_from(now.hour()).ok()?, u8::try_from(now.minute()).ok()?);
    Some((date.ok()?, time.ok()?))
}

impl Shell {
    /// The round check before an action item's text. Clicking it ticks the item off, or back on.
    pub(super) fn render_task_check(id: BlockId, done: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (accent, border, on_accent) = (theme.primary, theme.muted_foreground, theme.primary_foreground);
        div()
            .id(SharedString::from(format!("task-check-{id}")))
            .debug_selector(move || format!("task-check-{id}"))
            .size(px(CHECK_PX))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .cursor_pointer()
            .map(|check| {
                if done {
                    check
                        .bg(accent)
                        .border_color(accent)
                        .child(Icon::new(IconName::Check).xsmall().text_color(on_accent))
                } else {
                    check
                        .border_color(border.opacity(0.7))
                        .hover(|s| s.border_color(accent))
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    shell.toggle_todo(id, cx);
                }),
            )
            .into_any_element()
    }

    /// After an action item's text: its date chip and priority flag, or faint buttons to add them.
    pub(super) fn render_task_chips(
        &self,
        id: BlockId,
        task: TaskChips,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let (muted, faint, danger, hover, radius) = (
            theme.muted_foreground,
            theme.muted_foreground.opacity(0.55),
            theme.danger,
            theme.accent,
            theme.radius,
        );
        let dark = theme.mode.is_dark();
        let clock = local_now();
        let open = self
            .blocks
            .task_picker
            .as_ref()
            .filter(|p| p.block == id)
            .map(super::task_pickers::OpenTaskPicker::kind);
        let chip = |name: &'static str| {
            div()
                .id(SharedString::from(format!("{name}-{id}")))
                .debug_selector(move || format!("{name}-{id}"))
                .flex()
                .items_center()
                .gap_1()
                .px_1p5()
                .h(px(22.0))
                .rounded(radius)
                .text_xs()
                .cursor_pointer()
                .hover(|s| s.bg(hover))
        };
        let date = match task.due {
            Some(due) => {
                let overdue =
                    clock.is_some_and(|(today, now)| is_overdue(task.done, Some(due), task.time, today, now));
                let colour = if overdue { danger } else { muted };
                chip("task-date")
                    .text_color(colour)
                    .child(Icon::new(IconName::Calendar).xsmall().text_color(colour))
                    .child(
                        clock.map_or_else(|| due.to_string(), |(today, _)| due_label(due, task.time, today)),
                    )
            }
            None => chip("task-date")
                .text_color(faint)
                .child(Icon::new(IconName::Calendar).xsmall().text_color(faint))
                .child("Add date")
                .when(open != Some(TaskPickerKind::Date), |c| {
                    c.opacity(0.0).group_hover(ROW_GROUP, |s| s.opacity(1.0))
                }),
        };
        let flag = match task.priority {
            Some(priority) => {
                let colour = theme::priority_colour(priority, dark);
                chip("task-priority")
                    .text_color(colour)
                    .child(Icon::new(IconName::Flag).xsmall().text_color(colour))
                    .child(priority_word(priority))
            }
            None => chip("task-priority")
                .child(Icon::new(IconName::Flag).xsmall().text_color(faint))
                .when(open != Some(TaskPickerKind::Priority), |c| {
                    c.opacity(0.0).group_hover(ROW_GROUP, |s| s.opacity(1.0))
                }),
        };
        h_flex()
            .flex_none()
            .gap_1()
            .ml_2()
            .when(task.done, |chips| chips.opacity(0.6))
            .child(date.on_mouse_down(
                MouseButton::Left,
                Self::picker_opener(id, TaskPickerKind::Date, cx),
            ))
            .child(flag.on_mouse_down(
                MouseButton::Left,
                Self::picker_opener(id, TaskPickerKind::Priority, cx),
            ))
            .into_any_element()
    }
}

impl Shell {
    /// A chip's click: opens its picker just under the pointer, without placing the caret in the text.
    fn picker_opener(
        id: BlockId,
        kind: TaskPickerKind,
        cx: &mut Context<Self>,
    ) -> impl Fn(&MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static {
        cx.listener(move |shell, event: &MouseDownEvent, window, cx| {
            cx.stop_propagation();
            let at = point(event.position.x - px(12.0), event.position.y + px(16.0));
            shell.open_task_picker(id, kind, at, window, cx);
        })
    }
}

/// What the chips after an action item show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TaskChips {
    pub done: bool,
    pub due: Option<DueDate>,
    pub time: Option<DueTime>,
    pub priority: Option<Priority>,
}

/// The word beside the flag.
pub(super) fn priority_word(priority: Priority) -> &'static str {
    match priority {
        Priority::High => "High",
        Priority::Medium => "Medium",
        Priority::Low => "Low",
    }
}
