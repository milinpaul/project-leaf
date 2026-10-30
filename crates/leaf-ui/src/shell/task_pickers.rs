//! An action item's pickers (E19-S03): the date picker (a month calendar, Today / Tomorrow / Next week,
//! a time field and Clear) and the priority menu (High, Medium, Low, No priority). They open under the
//! chip that was clicked, or under the item with Cmd/Ctrl+Shift+D and Cmd/Ctrl+Shift+P, and close on a
//! choice, Escape or a click elsewhere. The calendar, the quick picks, reading a typed time and the
//! changes themselves are `leaf_editor`'s (`DatePicker`, `parse_time`, `set_task_*`); this module
//! draws them and forwards.

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, KeyBinding, Pixels, Point, Subscription, Window,
    actions, point, px,
};
use gpui_component::input::{InputEvent, InputState};
use leaf_core::BlockId;
use leaf_editor::{DatePicker, parse_time, priority_choices};
use leaf_model::{BlockKind, DueDate, DueTime, Priority};

use super::task_row::local_now;
use super::{Shell, report};

/// The pickers' key context, active while one has the keyboard.
const CONTEXT: &str = "LeafTaskPicker";

actions!(
    leaf_task_picker,
    [
        /// Open the caret's action item's date picker (in the page).
        OpenDatePicker,
        /// Open the caret's action item's priority menu (in the page).
        OpenPriorityMenu,
        /// Highlight the priority above.
        PickerUp,
        /// Highlight the priority below.
        PickerDown,
        /// Choose the highlighted priority.
        PickerEnter,
        /// Close without changing anything.
        PickerEscape,
    ]
);

/// Binds the pickers' keys, and the two shortcuts in the page. Called once at startup.
pub(super) fn bind(cx: &mut App, page_context: &'static str, date: &'static str, priority: &'static str) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new(date, OpenDatePicker, Some(page_context)),
        KeyBinding::new(priority, OpenPriorityMenu, Some(page_context)),
        KeyBinding::new("up", PickerUp, c),
        KeyBinding::new("down", PickerDown, c),
        KeyBinding::new("enter", PickerEnter, c),
        KeyBinding::new("escape", PickerEscape, c),
    ]);
}

/// Which picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TaskPickerKind {
    Date,
    Priority,
}

/// The open picker: for which action item, where, and its state.
pub(super) struct OpenTaskPicker {
    pub(super) block: BlockId,
    at: Point<Pixels>,
    state: PickerState,
    focus: FocusHandle,
    _events: Option<Subscription>,
}

enum PickerState {
    Date {
        calendar: DatePicker,
        time: Entity<InputState>,
    },
    Priority {
        highlighted: usize,
    },
}

impl OpenTaskPicker {
    pub(super) fn kind(&self) -> TaskPickerKind {
        match self.state {
            PickerState::Date { .. } => TaskPickerKind::Date,
            PickerState::Priority { .. } => TaskPickerKind::Priority,
        }
    }
}

/// An action item's fields, if `kind` is one.
fn task_fields(kind: &BlockKind) -> Option<(Option<DueDate>, Option<DueTime>, Option<Priority>)> {
    match kind {
        BlockKind::Task {
            due, time, priority, ..
        } => Some((*due, *time, *priority)),
        _ => None,
    }
}

impl Shell {
    /// Opens action item `block`'s date picker or priority menu with its top-left corner at `at`.
    pub(super) fn open_task_picker(
        &mut self,
        block: BlockId,
        kind: TaskPickerKind,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Ok(Some((due, time, priority)))) = self
            .page()
            .map(|page| page.doc().kind(block).map(|k| task_fields(&k)))
        else {
            return;
        };
        let focus = cx.focus_handle();
        let (state, events) = match kind {
            TaskPickerKind::Date => {
                let Some((today, _)) = local_now() else { return };
                let time_field = cx.new(|cx| {
                    let mut input = InputState::new(window, cx).placeholder("Time, e.g. 9:30");
                    if let Some(time) = time {
                        input.set_value(time.to_string(), window, cx);
                        // Selected, so a new time typed straight away replaces it.
                        input.select_all(window, cx);
                    }
                    input
                });
                let events = cx.subscribe_in(&time_field, window, |shell, input, event, window, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        let typed = input.read(cx).value().to_string();
                        shell.time_entered(&typed, window, cx);
                    }
                });
                let state = PickerState::Date {
                    calendar: DatePicker::new(due, today),
                    time: time_field,
                };
                (state, Some(events))
            }
            TaskPickerKind::Priority => {
                let highlighted = priority_choices()
                    .iter()
                    .position(|(p, _)| *p == priority)
                    .unwrap_or(0);
                (PickerState::Priority { highlighted }, None)
            }
        };
        // The date picker's time field takes the keyboard, so a time can be typed straight away; the
        // priority menu takes it for its arrow keys.
        match &state {
            PickerState::Date { time, .. } => time.update(cx, |input, cx| input.focus(window, cx)),
            PickerState::Priority { .. } => window.focus(&focus, cx),
        }
        self.blocks.task_picker = Some(OpenTaskPicker {
            block,
            at,
            state,
            focus,
            _events: events,
        });
        cx.notify();
    }

    /// Opens a picker for the caret's action item, under it as drawn in the last frame.
    fn open_task_picker_at_caret(
        &mut self,
        kind: TaskPickerKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(block) = self.caret_block() else { return };
        let at = self.blocks.drawn.bounds_of(block).map_or_else(
            || point(px(0.0), px(0.0)),
            |b| point(b.right() - px(260.0), b.bottom()),
        );
        self.open_task_picker(block, kind, at, window, cx);
    }

    /// Whether an action item's picker is open, and which: for tests.
    #[cfg(test)]
    #[must_use]
    pub(super) fn task_picker_open(&self) -> Option<TaskPickerKind> {
        self.blocks.task_picker.as_ref().map(OpenTaskPicker::kind)
    }

    /// Closes the picker and gives the page the keyboard back.
    fn close_task_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.blocks.task_picker.take().is_some() {
            self.focus_page(window, cx);
            // Enter in the time field redraws only the field; the row's new chip needs the page.
            cx.notify();
        }
    }

    /// Applies a change to the open picker's action item, reports a failure, and closes the picker.
    fn task_picked(
        &mut self,
        change: impl FnOnce(&mut leaf_editor::PageEditor, BlockId) -> Result<(), leaf_editor::EditorError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(block) = self.blocks.task_picker.as_ref().map(|p| p.block) else {
            return;
        };
        if let Some(Err(err)) = self.page().map(|page| change(page, block)) {
            report("could not change the action item", err);
        }
        self.close_task_picker(window, cx);
    }

    fn date_picked(&mut self, date: Option<DueDate>, window: &mut Window, cx: &mut Context<Self>) {
        self.task_picked(|page, block| page.set_task_due(block, date), window, cx);
    }

    /// The time field's Enter: a time sets it (and today's date, when the item has none); an empty
    /// field clears the time; anything else leaves the picker open.
    fn time_entered(&mut self, typed: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(PickerState::Date { calendar, .. }) = self.blocks.task_picker.as_ref().map(|p| &p.state)
        else {
            return;
        };
        let date = calendar.selected.unwrap_or(calendar.today);
        if typed.trim().is_empty() {
            self.task_picked(|page, block| page.set_task_time(block, None), window, cx);
        } else if let Some(time) = parse_time(typed) {
            self.task_picked(
                |page, block| {
                    page.set_task_due(block, Some(date))?;
                    page.set_task_time(block, Some(time))
                },
                window,
                cx,
            );
        }
    }

    fn priority_picked(&mut self, priority: Option<Priority>, window: &mut Window, cx: &mut Context<Self>) {
        self.task_picked(|page, block| page.set_task_priority(block, priority), window, cx);
    }

    fn picker_step(&mut self, down: bool, cx: &mut Context<Self>) {
        if let Some(OpenTaskPicker {
            state: PickerState::Priority { highlighted },
            ..
        }) = self.blocks.task_picker.as_mut()
        {
            let count = priority_choices().len();
            *highlighted = if down {
                (*highlighted + 1) % count
            } else {
                (*highlighted + count - 1) % count
            };
            cx.notify();
        }
    }

    fn picker_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(OpenTaskPicker {
            state: PickerState::Priority { highlighted },
            ..
        }) = self.blocks.task_picker.as_ref()
        {
            let choice = priority_choices()[*highlighted].0;
            self.priority_picked(choice, window, cx);
        }
    }

    fn shift_month(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let Some(OpenTaskPicker {
            state: PickerState::Date { calendar, .. },
            ..
        }) = self.blocks.task_picker.as_mut()
        {
            if forward {
                calendar.next_month();
            } else {
                calendar.previous_month();
            }
            cx.notify();
        }
    }

    /// Attaches the two shortcuts to the page element.
    pub(super) fn with_task_picker_actions<E: gpui::InteractiveElement>(
        element: E,
        cx: &mut Context<Self>,
    ) -> E {
        element
            .on_action(cx.listener(|s, _: &OpenDatePicker, w, cx| {
                s.open_task_picker_at_caret(TaskPickerKind::Date, w, cx);
            }))
            .on_action(cx.listener(|s, _: &OpenPriorityMenu, w, cx| {
                s.open_task_picker_at_caret(TaskPickerKind::Priority, w, cx);
            }))
    }
}

mod view;

#[cfg(test)]
mod tests;
