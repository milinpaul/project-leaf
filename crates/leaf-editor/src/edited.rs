//! Words for a page's edited time: "Edited 3 minutes ago" for the top bar, and the exact date and
//! time for its hover label.
//!
//! Plain functions of the times they are given. The caller reads the clock and the local time zone,
//! so this stays testable and builds for WebAssembly.

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

pub(crate) const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// How long ago `at` was, as of `now` (both Unix milliseconds): "Edited just now" under a minute,
/// then whole minutes, hours and days ("Edited 100 days ago"). Days don't roll over into weeks or
/// months, so the number always means the same unit. A time in the future (a clock that moved
/// back) reads as just now.
#[must_use]
pub fn edited_ago(now: i64, at: i64) -> String {
    let ago = now.saturating_sub(at).max(0);
    let (count, unit) = match ago {
        _ if ago < MINUTE => return "Edited just now".to_owned(),
        _ if ago < HOUR => (ago / MINUTE, "minute"),
        _ if ago < DAY => (ago / HOUR, "hour"),
        _ => (ago / DAY, "day"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("Edited {count} {unit}{plural} ago")
}

/// The exact date and time of `at` (Unix milliseconds) for the hover label, such as
/// "30 Oct 2026, 14:05". `offset_minutes` is the local time zone's offset from UTC at that moment
/// (60 for UTC+1), supplied by the caller. Day, short month and 24-hour time read the same in every
/// locale.
#[must_use]
pub fn edited_on(at: i64, offset_minutes: i32) -> String {
    let local = at.saturating_add(i64::from(offset_minutes).saturating_mul(MINUTE));
    let days = local.div_euclid(DAY);
    let of_day = local.rem_euclid(DAY);
    let (year, month, day) = civil_from_days(days);
    let month = MONTHS
        .get(usize::try_from(month.saturating_sub(1)).unwrap_or(0))
        .copied()
        .unwrap_or("Jan");
    format!(
        "{day} {month} {year}, {:02}:{:02}",
        of_day / HOUR,
        of_day % HOUR / MINUTE
    )
}

/// Year, month (1 to 12) and day (1 to 31) of the day `days` after 1 January 1970, in the
/// proleptic Gregorian calendar. Howard Hinnant's `civil_from_days`, valid far beyond any real
/// timestamp.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "z saturates; doe and everything after it stay below 146_097, and era is at most \
              i64::MAX / 146_097, so era * 400 and the year sum cannot overflow"
)]
pub(crate) fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests;
