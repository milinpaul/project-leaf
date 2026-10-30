//! Tests for an action item's due date, time and priority (E19-S01).

use super::{DueDate, DueTime, Priority};

#[test]
fn dates_read_and_write_as_year_month_day() {
    let date: DueDate = "2026-10-31".parse().unwrap();
    assert_eq!((date.year(), date.month(), date.day()), (2026, 10, 31));
    assert_eq!(date.to_string(), "2026-10-31");
    assert_eq!(DueDate::new(7, 1, 2).unwrap().to_string(), "0007-01-02");
    assert!(
        DueDate::new(2026, 1, 1).unwrap() < DueDate::new(2026, 1, 2).unwrap(),
        "earlier sorts first"
    );
}

#[test]
fn only_real_calendar_dates_are_accepted() {
    assert!("2024-02-29".parse::<DueDate>().is_ok(), "a leap year");
    assert!("2000-02-29".parse::<DueDate>().is_ok(), "divisible by 400");
    for bad in [
        "2026-02-29",
        "1900-02-29",
        "2026-04-31",
        "2026-13-01",
        "2026-00-10",
        "2026-10-00",
        "0000-01-01",
        "2026-1-01",
        "2026/10/31",
        "26-10-31",
        " 2026-10-31",
        "2026-10-3x",
        "+026-10-31",
        "",
    ] {
        assert!(bad.parse::<DueDate>().is_err(), "{bad:?}");
    }
}

#[test]
fn times_read_and_write_as_hours_and_minutes() {
    let time: DueTime = "09:05".parse().unwrap();
    assert_eq!((time.hour(), time.minute()), (9, 5));
    assert_eq!(time.to_string(), "09:05");
    assert_eq!(DueTime::new(23, 59).unwrap().to_string(), "23:59");
    for bad in ["24:00", "12:60", "9:05", "09-05", "09:5", "ab:cd", "+9:05", ""] {
        assert!(bad.parse::<DueTime>().is_err(), "{bad:?}");
    }
}

#[test]
fn priorities_have_stable_names() {
    for priority in [Priority::High, Priority::Medium, Priority::Low] {
        assert_eq!(priority.as_str().parse::<Priority>().unwrap(), priority);
    }
    assert!("urgent".parse::<Priority>().is_err());
    assert!(Priority::High < Priority::Low, "high sorts first");
}
