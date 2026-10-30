//! Tests for an action item's date words and overdue flag (E19-S02).

use leaf_model::{DueDate, DueTime};

use super::{days, due_label, is_overdue, weekday};

fn d(text: &str) -> DueDate {
    text.parse().unwrap()
}

fn t(text: &str) -> DueTime {
    text.parse().unwrap()
}

#[test]
fn day_counts_and_weekdays_match_the_calendar() {
    assert_eq!(days(d("1970-01-01")), 0);
    assert_eq!(
        days(d("2000-03-01")) - days(d("2000-02-28")),
        2,
        "2000 was a leap year"
    );
    assert_eq!(days(d("1900-03-01")) - days(d("1900-02-28")), 1, "1900 wasn't");
    assert_eq!(weekday(d("2026-10-30")), 4, "a Friday");
    assert_eq!(weekday(d("0001-01-01")), 0, "a Monday");
}

#[test]
fn labels_say_today_and_tomorrow_nearby_then_the_date() {
    let today = d("2026-10-30");
    assert_eq!(due_label(d("2026-10-30"), None, today), "Today");
    assert_eq!(
        due_label(d("2026-10-31"), Some(t("14:30")), today),
        "Tomorrow 14:30"
    );
    assert_eq!(due_label(d("2026-10-29"), None, today), "Yesterday");
    assert_eq!(due_label(d("2026-11-06"), None, today), "Fri 6 Nov");
    assert_eq!(
        due_label(d("2026-01-01"), Some(t("09:05")), today),
        "Thu 1 Jan 09:05"
    );
    assert_eq!(due_label(d("2027-10-30"), None, today), "Sat 30 Oct 2027");
    assert_eq!(
        due_label(d("2027-01-01"), None, d("2026-12-31")),
        "Tomorrow",
        "across the new year"
    );
}

#[test]
fn overdue_means_not_done_and_past_due() {
    let (today, now) = (d("2026-10-30"), t("12:00"));
    let overdue =
        |done, due: Option<&str>, time: Option<&str>| is_overdue(done, due.map(d), time.map(t), today, now);
    assert!(overdue(false, Some("2026-10-29"), None), "an earlier day");
    assert!(
        overdue(false, Some("2026-10-30"), Some("11:59")),
        "today, the time has passed"
    );
    assert!(
        !overdue(false, Some("2026-10-30"), Some("12:00")),
        "due this very minute: not yet"
    );
    assert!(
        !overdue(false, Some("2026-10-30"), None),
        "today without a time: by the end of the day"
    );
    assert!(!overdue(false, Some("2026-10-31"), None), "tomorrow");
    assert!(!overdue(true, Some("2026-10-01"), None), "done");
    assert!(!overdue(false, None, None), "no date");
}
