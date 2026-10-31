//! GitHub's alert syntax, which Leaf uses for callouts in Markdown (E21, ADR 0070): a quote whose first
//! line is `[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]` or `[!CAUTION]`, its other lines the
//! callout's lines.

use leaf_model::{BlockKind, CalloutTone, Marks, TextSpan};

use super::inline;
use crate::FragmentBlock;

/// The alert a callout's tone is written as. A tone from a newer version reads as a note.
pub(super) fn name(tone: &CalloutTone) -> &'static str {
    match tone {
        CalloutTone::Success => "TIP",
        CalloutTone::Warning => "WARNING",
        CalloutTone::Error => "CAUTION",
        CalloutTone::Note => "IMPORTANT",
        CalloutTone::Info | CalloutTone::Other(_) => "NOTE",
    }
}

/// The tone an alert name (any case) stands for: the inverse of [`name`]. Pasted Markdown and the
/// `> [!WARNING] ` shortcut both read names with it.
pub(crate) fn tone(name: &str) -> Option<CalloutTone> {
    Some(match name.to_ascii_uppercase().as_str() {
        "NOTE" => CalloutTone::Info,
        "TIP" => CalloutTone::Success,
        "IMPORTANT" => CalloutTone::Note,
        "WARNING" => CalloutTone::Warning,
        "CAUTION" => CalloutTone::Error,
        _ => return None,
    })
}

/// A quote line's text that opens an alert, `[!WARNING]`, maybe followed by text on the same line:
/// the tone and that text.
fn start(text: &str) -> Option<(CalloutTone, &str)> {
    let rest = text.trim_start().strip_prefix("[!")?;
    let close = rest.find(']')?;
    let tone = tone(&rest[..close])?;
    Some((tone, rest[close + 1..].trim_start()))
}

/// A pasted quote line with text `text` at indentation `columns`: the next line of the callout `open`
/// says is being read at that indentation, or the start of a new one. Returns whether it took the
/// line; a quote line it doesn't take ends the open callout.
pub(super) fn take(
    rows: &mut Vec<(usize, FragmentBlock)>,
    open: &mut Option<usize>,
    columns: usize,
    text: &str,
) -> bool {
    if *open == Some(columns)
        && let Some((_, callout)) = rows.last_mut()
    {
        if !callout.text.is_empty() {
            callout.text.push(TextSpan {
                text: "\n".into(),
                marks: Marks::default(),
            });
        }
        callout.text.extend(inline::parse(text.trim_end()));
        callout.text = inline::merge(std::mem::take(&mut callout.text));
        return true;
    }
    let Some((tone, first)) = start(text) else {
        return false;
    };
    rows.push((
        columns / 2,
        FragmentBlock {
            kind: BlockKind::Callout { tone },
            text: inline::parse(first.trim_end()),
            children: Vec::new(),
        },
    ));
    *open = Some(columns);
    true
}
