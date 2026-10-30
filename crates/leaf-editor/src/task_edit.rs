//! Setting an action item's date, time and priority, and the pickers' logic (E19-S03): the month
//! calendar, the quick picks, reading a typed time and the priority list. The UI draws them and
//! forwards clicks and keys; every decision is here, testable without a window.

use leaf_core::BlockId;
use leaf_model::{BlockProp, DueDate, DueTime, PageCommand, Priority};

use crate::task_due::{add_days, weekday};
use crate::{EditorError, PageEditor};

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

impl PageEditor {
    /// Sets action item `block`'s due date, or clears it (and its time) with `None`. One undo step.
    pub fn set_task_due(&mut self, block: BlockId, due: Option<DueDate>) -> Result<(), EditorError> {
        self.set_task_prop(block, BlockProp::TaskDue(due))
    }

    /// Sets action item `block`'s due time, or clears it with `None`. It needs a due date.
    pub fn set_task_time(&mut self, block: BlockId, time: Option<DueTime>) -> Result<(), EditorError> {
        self.set_task_prop(block, BlockProp::TaskTime(time))
    }

    /// Sets action item `block`'s priority, or none with `None`. One undo step.
    pub fn set_task_priority(
        &mut self,
        block: BlockId,
        priority: Option<Priority>,
    ) -> Result<(), EditorError> {
        self.set_task_prop(block, BlockProp::TaskPriority(priority))
    }

    fn set_task_prop(&mut self, id: BlockId, prop: BlockProp) -> Result<(), EditorError> {
        self.doc.apply(PageCommand::SetProp { id, prop })?;
        Ok(())
    }
}

/// A month on the picker's calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatePicker {
    year: u16,
    month: u8,
    /// The task's date now, marked on the calendar.
    pub selected: Option<DueDate>,
    /// Today, marked too, and where the quick picks start.
    pub today: DueDate,
}

impl DatePicker {
    /// The picker opened on the month of `selected`, or of `today` when the task has no date.
    #[must_use]
    pub fn new(selected: Option<DueDate>, today: DueDate) -> Self {
        let shown = selected.unwrap_or(today);
        Self {
            year: shown.year(),
            month: shown.month(),
            selected,
            today,
        }
    }

    /// The heading, such as "October 2026".
    #[must_use]
    pub fn title(&self) -> String {
        format!("{} {}", MONTH_NAMES[usize::from(self.month - 1)], self.year)
    }

    /// The month's days as week rows, Monday first; days outside the month are `None`.
    #[must_use]
    pub fn weeks(&self) -> Vec<[Option<DueDate>; 7]> {
        let Ok(first) = DueDate::new(self.year, self.month, 1) else {
            return Vec::new();
        };
        let lead = usize::try_from(weekday(first)).unwrap_or(0);
        let mut weeks = Vec::new();
        let mut week = [None; 7];
        let mut column = lead;
        let mut day = Some(first);
        while let Some(date) = day.filter(|d| d.month() == self.month) {
            week[column] = Some(date);
            column += 1;
            if column == 7 {
                weeks.push(week);
                week = [None; 7];
                column = 0;
            }
            day = add_days(date, 1);
        }
        if column > 0 {
            weeks.push(week);
        }
        weeks
    }

    /// Shows the month before.
    pub fn previous_month(&mut self) {
        if self.month == 1 {
            if self.year > 1 {
                (self.year, self.month) = (self.year - 1, 12);
            }
        } else {
            self.month -= 1;
        }
    }

    /// Shows the month after.
    pub fn next_month(&mut self) {
        if self.month == 12 {
            if self.year < 9999 {
                (self.year, self.month) = (self.year + 1, 1);
            }
        } else {
            self.month += 1;
        }
    }

    /// The quick picks above the calendar: Today, Tomorrow, and Next week (seven days on).
    #[must_use]
    pub fn quick_picks(&self) -> Vec<(&'static str, DueDate)> {
        [("Today", 0), ("Tomorrow", 1), ("Next week", 7)]
            .into_iter()
            .filter_map(|(label, n)| add_days(self.today, n).map(|date| (label, date)))
            .collect()
    }
}

/// Reads a typed time: `9`, `9:30`, `09:30`, `0930`, `9pm`, `9:30 am` and the like. `None` when it
/// isn't a time of day.
#[must_use]
pub fn parse_time(text: &str) -> Option<DueTime> {
    let text = text.trim().to_ascii_lowercase();
    let (clock, half) = if let Some(rest) = text.strip_suffix("am") {
        (rest.trim_end(), Some(false))
    } else if let Some(rest) = text.strip_suffix("pm") {
        (rest.trim_end(), Some(true))
    } else {
        (text.as_str(), None)
    };
    let (hour, minute) = match clock.split_once([':', '.']) {
        Some((h, m)) if m.len() == 2 => (h, m),
        Some(_) => return None,
        None if clock.len() > 2 => clock.split_at(clock.len() - 2),
        None => (clock, "00"),
    };
    if hour.is_empty() || !hour.bytes().chain(minute.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut hour: u8 = hour.parse().ok()?;
    let minute: u8 = minute.parse().ok()?;
    if let Some(pm) = half {
        if !(1..=12).contains(&hour) {
            return None;
        }
        hour = match (hour, pm) {
            (12, false) => 0,
            (12, true) => 12,
            (h, true) => h + 12,
            (h, false) => h,
        };
    }
    DueTime::new(hour, minute).ok()
}

/// The priority menu's entries, top to bottom, with their words: High, Medium, Low, then None.
#[must_use]
pub fn priority_choices() -> [(Option<Priority>, &'static str); 4] {
    [
        (Some(Priority::High), "High"),
        (Some(Priority::Medium), "Medium"),
        (Some(Priority::Low), "Low"),
        (None, "No priority"),
    ]
}

#[cfg(test)]
mod tests;
