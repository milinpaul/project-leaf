//! Words for fixture text: plain English of the kind people write in notes, so text is a realistic
//! length, wraps like real text, and gives search something to find. Ends with a few accented and
//! non-Latin words so search and layout meet them too.

use crate::Rng;

/// The vocabulary. Order matters: changing it changes every fixture.
#[rustfmt::skip]
const WORDS: &[&str] = &[
    "meeting", "notes", "project", "plan", "garden", "recipe", "travel", "budget", "review", "draft", "idea",
    "list", "week", "month", "goal", "team", "design", "launch", "book", "reading", "summary", "question",
    "answer", "follow", "up", "call", "email", "client", "report", "research", "the", "a", "and", "of", "to",
    "for", "with", "on", "in", "about", "today", "tomorrow", "next", "last", "first", "quick", "long",
    "small", "big", "new", "write", "check", "send", "read", "fix", "move", "share", "start", "finish",
    "keep", "tomatoes", "basil", "bread", "coffee", "train", "museum", "beach", "mountain", "city", "river",
    "release", "checklist", "bug", "feature", "sketch", "outline", "chapter", "lesson", "course", "habit",
    "café", "naïve", "résumé", "Zürich", "日本", "東京", "नमस्ते", "привет",
];

/// Fenced-code lines for code blocks.
const CODE: &[&str] = &[
    "fn main() {\n    println!(\"hello\");\n}",
    "let total: u32 = items.iter().map(|i| i.price).sum();",
    "SELECT title FROM pages WHERE trashed = 0;",
    "for line in lines {\n    process(line)?;\n}",
];

/// Emoji used as page icons.
pub(crate) const ICONS: &[&str] = &["📓", "🌱", "🧭", "📌", "🍅", "✈️", "💡", "📚"];

/// `count` words joined by spaces, the first capitalised.
pub(crate) fn words(rng: &mut Rng, count: usize) -> String {
    let mut out = String::new();
    for i in 0..count {
        let word = rng.pick(WORDS).copied().unwrap_or("note");
        if i > 0 {
            out.push(' ');
            out.push_str(word);
        } else {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(chars.as_str());
            }
        }
    }
    out
}

/// A sentence of `low..=high` words, ending in a full stop.
pub(crate) fn sentence(rng: &mut Rng, low: usize, high: usize) -> String {
    let count = rng.between(low, high);
    let mut text = words(rng, count);
    text.push('.');
    text
}

/// A short code snippet.
pub(crate) fn code(rng: &mut Rng) -> String {
    rng.pick(CODE).copied().unwrap_or("x").to_owned()
}
