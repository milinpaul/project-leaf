//! What Cmd/Ctrl+K and Cmd/Ctrl+P do. Cmd/Ctrl+K means two things in the spec: over selected text it
//! makes a link (as in every editor), otherwise it opens search (as in Notion). Cmd/Ctrl+P always opens
//! search.

/// A shortcut that can open the search bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    /// Cmd+K on macOS, Ctrl+K elsewhere.
    ModK,
    /// Cmd+P on macOS, Ctrl+P elsewhere.
    ModP,
}

/// What a [`Shortcut`] does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    /// Add or edit a link on the selected text.
    Link,
    /// Open the search bar.
    OpenSearch,
}

/// What `shortcut` does when `text_selected` says whether some text (not whole blocks) is selected in
/// the page.
#[must_use]
pub fn shortcut_action(shortcut: Shortcut, text_selected: bool) -> ShortcutAction {
    match shortcut {
        Shortcut::ModK if text_selected => ShortcutAction::Link,
        Shortcut::ModK | Shortcut::ModP => ShortcutAction::OpenSearch,
    }
}
