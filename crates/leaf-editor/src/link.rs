//! Following links: which link sits at a place in the text, and which addresses Leaf hands to the
//! system to open (E07-S07, ADR 0052). Adding, changing and removing links is in `format`.

use leaf_model::Caret;

use crate::format::char_marks;
use crate::{EditorError, PageEditor};

/// Schemes Leaf opens. Anything else (`javascript:`, `file:`, an app's own scheme) is refused: a
/// link pasted from elsewhere must not be able to run something on the owner's computer.
const SCHEMES: [&str; 3] = ["http://", "https://", "mailto:"];

impl PageEditor {
    /// The address of the link at `at`: the linked character after it, or else the one before.
    /// What Cmd/Ctrl+click follows.
    pub fn link_at(&self, at: Caret) -> Result<Option<String>, EditorError> {
        let marks = char_marks(&self.doc, at.block)?;
        Ok([Some(at.at), at.at.checked_sub(1)]
            .into_iter()
            .flatten()
            .find_map(|i| marks.get(i).and_then(|m| m.link.clone())))
    }
}

/// The link to make from pasted `text`, when it is one address on its own (E17-S10): a scheme Leaf
/// opens (`https://`, `http://`, `mailto:`) or a `www.` address, with spaces and line breaks around it
/// ignored. Stricter than [`openable_url`], which also takes `example.com`: a pasted `notes.txt` stays
/// text.
#[must_use]
pub fn pasted_link(text: &str) -> Option<String> {
    let text = text.trim();
    if text.contains(char::is_whitespace) {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    let addressed = SCHEMES.iter().any(|scheme| lower.starts_with(scheme)) || lower.starts_with("www.");
    addressed.then(|| openable_url(text)).flatten()
}

/// What to hand the system's browser or mail app for a link's `href`, or `None` when Leaf won't open
/// it. An address without a scheme that looks like a web address (`example.com/page`) gets
/// `https://`.
#[must_use]
pub fn openable_url(href: &str) -> Option<String> {
    let href = href.trim();
    let lower = href.to_ascii_lowercase();
    if let Some(scheme) = SCHEMES.iter().find(|scheme| lower.starts_with(*scheme)) {
        // Something must follow the scheme.
        return (href.len() > scheme.len()).then(|| href.to_owned());
    }
    // No scheme: a web address has a dot in its host and no spaces, and no other scheme before it.
    let host = href.split(['/', '?', '#']).next().unwrap_or_default();
    let looks_like_web = !href.is_empty()
        && !href.contains(char::is_whitespace)
        && host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !host.contains(':');
    looks_like_web.then(|| format!("https://{href}"))
}

#[cfg(test)]
mod tests;
