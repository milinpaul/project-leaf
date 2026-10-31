//! The values an action item carries (E19, ADR-068): its due date, an optional time of day and a
//! priority.
//!
//! A due date is a calendar date in the owner's own sense of "Friday", stored as `YYYY-MM-DD`, and the
//! time a wall-clock `HH:MM`. Neither is ever converted between time zones: a task due on Friday stays
//! due on Friday on a trip. That is also why there is no date library here; nothing needs one.

use std::fmt;
use std::str::FromStr;

use crate::ModelError;

/// The day an action item is due.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DueDate {
    year: u16,
    month: u8,
    day: u8,
}

impl DueDate {
    /// The date, if it exists on the calendar (years 1 to 9999).
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, ModelError> {
        let valid = (1..=9999).contains(&year)
            && (1..=12).contains(&month)
            && day >= 1
            && day <= days_in(year, month);
        if valid {
            Ok(Self { year, month, day })
        } else {
            Err(ModelError::InvalidDate(format!("{year:04}-{month:02}-{day:02}")))
        }
    }

    /// The year.
    #[must_use]
    pub fn year(self) -> u16 {
        self.year
    }

    /// The month, 1 to 12.
    #[must_use]
    pub fn month(self) -> u8 {
        self.month
    }

    /// The day of the month, from 1.
    #[must_use]
    pub fn day(self) -> u8 {
        self.day
    }
}

/// How many days `month` of `year` has.
fn days_in(year: u16, month: u8) -> u8 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl fmt::Display for DueDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl FromStr for DueDate {
    type Err = ModelError;

    /// Reads exactly `YYYY-MM-DD`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bad = || ModelError::InvalidDate(text.to_owned());
        let bytes = text.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return Err(bad());
        }
        let number = |range: std::ops::Range<usize>| -> Result<u16, ModelError> {
            let part = &text[range];
            if part.bytes().all(|b| b.is_ascii_digit()) {
                part.parse().map_err(|_| bad())
            } else {
                Err(bad())
            }
        };
        let month = u8::try_from(number(5..7)?).map_err(|_| bad())?;
        let day = u8::try_from(number(8..10)?).map_err(|_| bad())?;
        Self::new(number(0..4)?, month, day).map_err(|_| bad())
    }
}

/// The time of day an action item is due, on its due date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DueTime {
    hour: u8,
    minute: u8,
}

impl DueTime {
    /// The time, if it is a real time of day (00:00 to 23:59).
    pub fn new(hour: u8, minute: u8) -> Result<Self, ModelError> {
        if hour < 24 && minute < 60 {
            Ok(Self { hour, minute })
        } else {
            Err(ModelError::InvalidTime(format!("{hour:02}:{minute:02}")))
        }
    }

    /// The hour, 0 to 23.
    #[must_use]
    pub fn hour(self) -> u8 {
        self.hour
    }

    /// The minute, 0 to 59.
    #[must_use]
    pub fn minute(self) -> u8 {
        self.minute
    }
}

impl fmt::Display for DueTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

impl FromStr for DueTime {
    type Err = ModelError;

    /// Reads exactly `HH:MM`, 24-hour.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bad = || ModelError::InvalidTime(text.to_owned());
        let bytes = text.as_bytes();
        if bytes.len() != 5
            || bytes[2] != b':'
            || !text[..2]
                .bytes()
                .chain(text[3..].bytes())
                .all(|b| b.is_ascii_digit())
        {
            return Err(bad());
        }
        let hour = text[..2].parse().map_err(|_| bad())?;
        let minute = text[3..].parse().map_err(|_| bad())?;
        Self::new(hour, minute).map_err(|_| bad())
    }
}

/// How important an action item is. No priority at all is `None` where a priority is optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    /// Do first.
    High,
    /// The usual.
    Medium,
    /// When there's time.
    Low,
}

impl Priority {
    /// The stored name: `high`, `medium` or `low`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

impl FromStr for Priority {
    type Err = ModelError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            other => Err(ModelError::InvalidKind(format!("unknown priority {other:?}"))),
        }
    }
}

#[cfg(test)]
mod tests;
