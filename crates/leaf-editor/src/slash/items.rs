//! What the slash menu offers, and how a query picks and orders it.
//!
//! The filter is a small fuzzy match, written here rather than taken from a crate: the list is a
//! dozen entries, and the ranking rules are few enough to state and test (ADR-026). A query matches
//! an entry when its letters appear in order in the entry's name or one of its keywords. Better
//! matches come first:
//!
//! 1. the name starts with the query;
//! 2. a word in the name, or a keyword, starts with the query;
//! 3. the letters appear in order, with fewer gaps first.
//!
//! Ties keep the menu's own order, so the list never jumps around for no reason.

use leaf_model::{BlockKind, CalloutTone};

/// One entry in the slash menu.
#[derive(Debug, Clone, PartialEq)]
pub struct SlashItem {
    /// What the menu shows.
    pub label: &'static str,
    /// Other words that find it ("h1", "bullet", "checkbox").
    pub keywords: &'static [&'static str],
    /// What choosing it does.
    pub action: SlashAction,
}

/// What choosing a slash menu entry does.
#[derive(Debug, Clone, PartialEq)]
pub enum SlashAction {
    /// Converts an empty block to this kind, or adds a block of it below.
    Block(BlockKind),
    /// Asks for an image file: the UI shows a file picker and gives the stored picture to
    /// [`PageEditor::insert_image`](crate::PageEditor::insert_image) (E05-S07).
    Image,
    /// Adds a 3 × 3 table with a header row, in place of an empty line or below the block (E16-S04).
    Table,
    /// Types `[[` in place of the `/` and query and opens the page-link menu there (E10-S05).
    LinkToPage,
}

/// A callout block of `tone`.
fn callout(tone: CalloutTone) -> BlockKind {
    BlockKind::Callout { tone }
}

/// Every entry, in the order the menu shows them with no query.
pub(crate) fn all_items() -> Vec<SlashItem> {
    let item = |label, keywords, kind| SlashItem {
        label,
        keywords,
        action: SlashAction::Block(kind),
    };
    vec![
        item("Text", &["paragraph", "plain"], BlockKind::Paragraph),
        item(
            "Heading 1",
            &["h1", "title", "#"],
            BlockKind::Heading { level: 1 },
        ),
        item(
            "Heading 2",
            &["h2", "subtitle", "##"],
            BlockKind::Heading { level: 2 },
        ),
        item("Heading 3", &["h3", "###"], BlockKind::Heading { level: 3 }),
        item(
            "Bulleted list",
            &["bullet", "unordered", "ul", "-"],
            BlockKind::BulletedList,
        ),
        item("Numbered list", &["ordered", "ol", "1."], BlockKind::NumberedList),
        item(
            "To-do list",
            &["todo", "checkbox", "[]"],
            BlockKind::Todo { checked: false },
        ),
        item(
            "Action item",
            &["task", "action", "due", "deadline", "date", "priority", "[!]"],
            BlockKind::Task {
                done: false,
                due: None,
                time: None,
                priority: None,
            },
        ),
        item("Quote", &["blockquote", "citation", ">"], BlockKind::Quote),
        item(
            "Callout",
            &["info", "box", "alert", "[!note]"],
            callout(CalloutTone::Info),
        ),
        item(
            "Callout: Success",
            &["success", "done", "tip", "[!tip]"],
            callout(CalloutTone::Success),
        ),
        item(
            "Callout: Warning",
            &["warning", "careful", "[!warning]"],
            callout(CalloutTone::Warning),
        ),
        item(
            "Callout: Error",
            &["error", "danger", "problem", "[!caution]"],
            callout(CalloutTone::Error),
        ),
        item(
            "Callout: Note",
            &["note", "aside", "tip", "[!important]"],
            callout(CalloutTone::Note),
        ),
        item(
            "Code",
            &["snippet", "pre", "```"],
            BlockKind::Code {
                language: String::new(),
            },
        ),
        item("Divider", &["separator", "line", "hr", "---"], BlockKind::Divider),
        SlashItem {
            label: "Image",
            keywords: &["picture", "photo", "img", "upload"],
            action: SlashAction::Image,
        },
        SlashItem {
            label: "Table",
            keywords: &["grid", "rows", "|"],
            action: SlashAction::Table,
        },
        SlashItem {
            label: "Link to page",
            keywords: &["page", "mention", "[["],
            action: SlashAction::LinkToPage,
        },
    ]
}

/// The entries matching `query`, best first. An empty query matches everything in menu order.
pub(crate) fn filter(items: &[SlashItem], query: &str) -> Vec<SlashItem> {
    let query = query.trim().to_lowercase();
    let mut scored: Vec<(u32, usize, &SlashItem)> = items
        .iter()
        .enumerate()
        .filter_map(|(order, item)| score(item, &query).map(|s| (s, order, item)))
        .collect();
    scored.sort_by_key(|&(score, order, _)| (std::cmp::Reverse(score), order));
    scored.into_iter().map(|(_, _, item)| item.clone()).collect()
}

/// How well `query` (lowercase) matches the item; `None` if it doesn't. Higher is better.
fn score(item: &SlashItem, query: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    let label = item.label.to_lowercase();
    if label.starts_with(query) {
        return Some(3000);
    }
    let word_start = label.split([' ', '-']).any(|word| word.starts_with(query))
        || item.keywords.iter().any(|k| k.starts_with(query));
    if word_start {
        return Some(2000);
    }
    std::iter::once(label.as_str())
        .chain(item.keywords.iter().copied())
        .filter_map(|target| in_order(target, query))
        .max()
        .map(|closeness| 1000 + closeness)
}

/// Whether the query's letters appear in order in `target`; if so, a closeness score that is higher
/// the fewer letters lie between them (at most 999).
fn in_order(target: &str, query: &str) -> Option<u32> {
    let mut gaps = 0u32;
    let mut rest = target.chars();
    for wanted in query.chars() {
        let mut skipped = 0u32;
        loop {
            match rest.next() {
                Some(c) if c == wanted => break,
                Some(_) => skipped += 1,
                None => return None,
            }
        }
        gaps += skipped;
    }
    Some(999u32.saturating_sub(gaps))
}
