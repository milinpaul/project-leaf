use super::*;

const NOW: i64 = 1_793_000_000_000;

#[test]
fn under_a_minute_is_just_now() {
    assert_eq!(edited_ago(NOW, NOW), "Edited just now");
    assert_eq!(edited_ago(NOW, NOW - (MINUTE - 1)), "Edited just now");
}

#[test]
fn minutes_are_singular_then_plural() {
    assert_eq!(edited_ago(NOW, NOW - MINUTE), "Edited 1 minute ago");
    assert_eq!(edited_ago(NOW, NOW - (2 * MINUTE - 1)), "Edited 1 minute ago");
    assert_eq!(edited_ago(NOW, NOW - 3 * MINUTE), "Edited 3 minutes ago");
    assert_eq!(edited_ago(NOW, NOW - (HOUR - 1)), "Edited 59 minutes ago");
}

#[test]
fn hours_start_at_one_hour() {
    assert_eq!(edited_ago(NOW, NOW - HOUR), "Edited 1 hour ago");
    assert_eq!(edited_ago(NOW, NOW - 2 * HOUR), "Edited 2 hours ago");
    assert_eq!(edited_ago(NOW, NOW - (DAY - 1)), "Edited 23 hours ago");
}

#[test]
fn days_never_roll_into_weeks_or_months() {
    assert_eq!(edited_ago(NOW, NOW - DAY), "Edited 1 day ago");
    assert_eq!(edited_ago(NOW, NOW - 2 * DAY), "Edited 2 days ago");
    assert_eq!(edited_ago(NOW, NOW - 100 * DAY), "Edited 100 days ago");
    assert_eq!(edited_ago(NOW, NOW - 800 * DAY), "Edited 800 days ago");
}

#[test]
fn a_time_in_the_future_reads_as_just_now() {
    assert_eq!(edited_ago(NOW, NOW + 5 * MINUTE), "Edited just now");
    assert_eq!(edited_ago(i64::MIN, i64::MAX), "Edited just now");
    assert_eq!(
        edited_ago(i64::MAX, i64::MIN),
        format!("Edited {} days ago", i64::MAX / DAY)
    );
}

#[test]
fn exact_time_in_utc() {
    assert_eq!(edited_on(0, 0), "1 Jan 1970, 00:00");
    // 2026-10-30 14:05:59.999 UTC.
    assert_eq!(edited_on(1_793_369_159_999, 0), "30 Oct 2026, 14:05");
}

#[test]
fn exact_time_uses_the_given_offset() {
    let at = 1_793_369_100_000; // 2026-10-30 14:05 UTC
    assert_eq!(edited_on(at, 60), "30 Oct 2026, 15:05");
    assert_eq!(edited_on(at, 330), "30 Oct 2026, 19:35");
    assert_eq!(edited_on(at, -15 * 60), "29 Oct 2026, 23:05");
}

#[test]
fn exact_time_across_leap_days_and_year_ends() {
    assert_eq!(edited_on(951_782_400_000, 0), "29 Feb 2000, 00:00");
    assert_eq!(edited_on(1_709_164_800_000, 0), "29 Feb 2024, 00:00");
    assert_eq!(edited_on(1_735_689_599_000, 0), "31 Dec 2024, 23:59");
    assert_eq!(edited_on(-1, 0), "31 Dec 1969, 23:59");
}
