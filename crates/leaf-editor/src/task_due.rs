//! Words and flags for an action item's due date (E19-S02): "Today", "Fri 31 Oct 14:30", and whether
//! it is overdue.
//!
//! Plain functions of the date and time the caller passes as "now", like `edited`: the app reads the
//! clock and the local time zone, so this stays testable and builds for WebAssembly. Rows of the page
//! view carry only the stored fields; these turn them into words when drawing, so nothing cached
//! goes stale at midnight.

use leaf_model::{DueDate, DueTime};

use crate::edited::MONTHS;

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// The date chip's words for a task due on `due` (at `time`, if set), seen on `today`: "Today",
/// "Tomorrow" or "Yesterday" nearby, else the weekday, day and month ("Fri 31 Oct"), with the year
/// when it isn't this year's ("Sat 30 Oct 2027"). A time follows as 24-hour "14:30".
#[must_use]
pub fn due_label(due: DueDate, time: Option<DueTime>, today: DueDate) -> String {
    let date = match days(due) - days(today) {
        0 => "Today".to_owned(),
        1 => "Tomorrow".to_owned(),
        -1 => "Yesterday".to_owned(),
        _ => {
            let weekday = WEEKDAYS[usize::try_from(weekday(due)).unwrap_or(0)];
            let month = MONTHS[usize::from(due.month() - 1)];
            if due.year() == today.year() {
                format!("{weekday} {} {month}", due.day())
            } else {
                format!("{weekday} {} {month} {}", due.day(), due.year())
            }
        }
    };
    match time {
        Some(time) => format!("{date} {time}"),
        None => date,
    }
}

/// Whether a task is overdue at `now` on `today`: not done, and due on an earlier day, or due today
/// at a time that has passed. A task due today without a time is due by the end of the day, so it
/// isn't overdue yet; one without a date never is.
#[must_use]
pub fn is_overdue(
    done: bool,
    due: Option<DueDate>,
    time: Option<DueTime>,
    today: DueDate,
    now: DueTime,
) -> bool {
    let Some(due) = due else {
        return false;
    };
    !done && (due < today || (due == today && time.is_some_and(|time| time < now)))
}

/// Days from 1 January 1970 to `date`: Howard Hinnant's `days_from_civil`, the inverse of the one in
/// `edited`.
pub(crate) fn days(date: DueDate) -> i64 {
    let (month, day) = (i64::from(date.month()), i64::from(date.day()));
    let year = i64::from(date.year()) - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `date` moved by `n` days (back for negative `n`), or `None` past the years a due date can have.
#[must_use]
pub fn add_days(date: DueDate, n: i64) -> Option<DueDate> {
    let (year, month, day) = crate::edited::civil_from_days(days(date) + n);
    DueDate::new(
        u16::try_from(year).ok()?,
        u8::try_from(month).ok()?,
        u8::try_from(day).ok()?,
    )
    .ok()
}

/// The day of the week, 0 for Monday to 6 for Sunday. 1 January 1970 was a Thursday.
pub(crate) fn weekday(date: DueDate) -> i64 {
    (days(date) + 3).rem_euclid(7)
}

#[cfg(test)]
mod tests;
