//! An action item in Markdown (E19-S04, ADR 0032): a task list line with its fields in a group at the
//! end, readable in any app, e.g. `- [ ] Send invoice (due 2026-10-31 14:00, high)`. An item with no
//! date or priority ends in `(action item)`, so it still reads back as one rather than as a to-do.
//!
//! Reading is strict: only a final group whose every part is one of these words makes an action item,
//! so a to-do like `- [ ] Call Sam (after lunch)` stays a to-do. A to-do whose own text would read as
//! such a group has its bracket escaped when written, so it comes back as a to-do.

use leaf_model::{BlockKind, DueDate, DueTime, Priority};

/// What marks an action item that has neither a date nor a priority.
const MARKER: &str = "action item";

/// An action item's fields, as the group at the end of its line says.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Fields {
    pub due: Option<DueDate>,
    pub time: Option<DueTime>,
    pub priority: Option<Priority>,
}

/// The fields in the group at the end of a to-do line's text, and the text before it; `None` when the
/// text doesn't end in such a group.
pub(super) fn read(text: &str) -> Option<(Fields, &str)> {
    let start = group_start(text)?;
    let inside = text[start + 1..].strip_suffix(')')?;
    let mut fields = Fields::default();
    let mut marked = false;
    for part in inside.split(',').map(str::trim) {
        let lower = part.to_ascii_lowercase();
        if lower == MARKER && !marked {
            marked = true;
        } else if let Ok(priority) = lower.parse::<Priority>()
            && fields.priority.is_none()
        {
            fields.priority = Some(priority);
        } else if let Some(when) = lower.strip_prefix("due ")
            && fields.due.is_none()
        {
            let mut words = when.split_whitespace();
            fields.due = Some(words.next()?.parse().ok()?);
            fields.time = words.next().map(str::parse).transpose().ok()?;
            if words.next().is_some() {
                return None;
            }
        } else {
            return None;
        }
    }
    Some((fields, text[..start].trim_end()))
}

/// A to-do line whose text ends in an action item's fields is that action item, without the group;
/// any other line stays as it is.
pub(super) fn from_todo((kind, text): (BlockKind, &str)) -> (BlockKind, &str) {
    match kind {
        BlockKind::Todo { checked } => match read(text.trim_end()) {
            Some((fields, rest)) => (
                BlockKind::Task {
                    done: checked,
                    due: fields.due,
                    time: fields.time,
                    priority: fields.priority,
                },
                rest,
            ),
            None => (kind, text),
        },
        _ => (kind, text),
    }
}

/// Where the final `(…)` group starts: at the very start, or after a space.
fn group_start(text: &str) -> Option<usize> {
    if !text.ends_with(')') {
        return None;
    }
    if text.starts_with('(') && !text[1..].contains('(') {
        return Some(0);
    }
    text.rfind(" (").map(|at| at + 1)
}

/// The group written after an action item's text (with the space before it when there is text).
pub(super) fn write(text: &str, fields: Fields) -> String {
    let mut parts = Vec::new();
    if let Some(due) = fields.due {
        parts.push(match fields.time {
            Some(time) => format!("due {due} {time}"),
            None => format!("due {due}"),
        });
    }
    if let Some(priority) = fields.priority {
        parts.push(priority.as_str().to_owned());
    }
    if parts.is_empty() {
        parts.push(MARKER.to_owned());
    }
    let space = if text.is_empty() { "" } else { " " };
    format!("{text}{space}({})", parts.join(", "))
}

/// A to-do's Markdown text, with the bracket of a final group escaped if it would read as an action
/// item's fields.
pub(super) fn escape_todo(text: &str) -> String {
    match read(text).and_then(|_| group_start(text)) {
        Some(at) => format!("{}\\{}", &text[..at], &text[at..]),
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(due: &str, time: Option<&str>, priority: Option<Priority>) -> Fields {
        Fields {
            due: due.parse().ok(),
            time: time.and_then(|t| t.parse().ok()),
            priority,
        }
    }

    #[test]
    fn the_group_reads_every_field_in_any_order_and_case() {
        let all = fields("2026-10-31", Some("14:00"), Some(Priority::High));
        assert_eq!(
            read("Send invoice (due 2026-10-31 14:00, high)"),
            Some((all, "Send invoice"))
        );
        assert_eq!(
            read("Send invoice (High, Due 2026-10-31 14:00)"),
            Some((all, "Send invoice"))
        );
        assert_eq!(read("x (low)").map(|r| r.0.priority), Some(Some(Priority::Low)));
        assert_eq!(read("x (action item)"), Some((Fields::default(), "x")));
        assert_eq!(read("(medium)").map(|r| r.1), Some(""));
    }

    #[test]
    fn anything_else_in_brackets_is_just_text() {
        for text in [
            "Call Sam (after lunch)",
            "x (high, after lunch)",
            "x (high, low)",
            "x (due tomorrow)",
            "x (due 2026-02-30)",
            "x (due 2026-10-31 25:00)",
            "x (due 2026-10-31 14:00 sharp)",
            "x ()",
            "x(high)",
            "x \\(high)",
            "no brackets",
        ] {
            assert_eq!(read(text), None, "{text}");
        }
    }

    #[test]
    fn writing_then_reading_gives_the_fields_back() {
        for f in [
            Fields::default(),
            fields("2026-10-31", None, None),
            fields("2026-10-31", Some("09:05"), Some(Priority::Medium)),
            fields("", None, Some(Priority::Low)),
        ] {
            for text in ["Renew passport", "", "Buy (high) things"] {
                let line = write(text, f);
                assert_eq!(read(&line), Some((f, text)), "{line}");
            }
        }
        assert_eq!(
            write(
                "Send invoice",
                fields("2026-10-31", Some("14:00"), Some(Priority::High))
            ),
            "Send invoice (due 2026-10-31 14:00, high)"
        );
        assert_eq!(write("", Fields::default()), "(action item)");
    }

    #[test]
    fn a_to_do_that_looks_like_an_action_item_is_escaped() {
        assert_eq!(escape_todo("Pick a level (high)"), "Pick a level \\(high)");
        assert_eq!(escape_todo("(low)"), "\\(low)");
        assert_eq!(escape_todo("Call Sam (after lunch)"), "Call Sam (after lunch)");
        assert_eq!(read(&escape_todo("Pick a level (high)")), None);
    }
}
