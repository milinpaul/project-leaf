//! Commands: the only way to change a document.
//!
//! The editor turns key presses and clicks into commands; the model applies each one as exactly one
//! Loro commit, which later makes it exactly one undo step. Keeping every change in an enum means
//! each one can be tested, logged and replayed on its own.

use leaf_core::PageId;

/// A change to the workspace doc: the page tree and each page's title, icon and trash state.
///
/// Applied with [`Workspace::apply`](crate::Workspace::apply). Trash arrives in E01-S05.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceCommand {
    /// Adds a page under `parent` (or at the top level when `None`) at position `index` among its
    /// siblings. The model mints the new page's ID and returns it in [`Outcome::PageCreated`].
    CreatePage {
        /// Page to nest under, or `None` for a top-level page.
        parent: Option<PageId>,
        /// Position among the parent's children; equal to the child count appends.
        index: usize,
        /// Initial title; may be empty.
        title: String,
        /// Creation time in Unix milliseconds, supplied by the caller. The model never reads the
        /// clock: that would fail in WebAssembly and make tests depend on the time of day.
        created_at: i64,
    },
    /// Changes a page's title. Any text is allowed, including an empty title.
    RenamePage {
        /// Page to rename.
        page: PageId,
        /// New title.
        title: String,
    },
    /// Sets or clears a page's icon.
    SetPageIcon {
        /// Page to change.
        page: PageId,
        /// The new icon (a single emoji, chosen by the emoji picker), or `None` to remove it.
        /// An empty string is rejected: use `None` to clear.
        icon: Option<String>,
    },
    /// Moves a page, with everything inside it, under `new_parent` (or to the top level when
    /// `None`) at position `index`.
    MovePage {
        /// Page to move.
        page: PageId,
        /// Page to move it into, or `None` for the top level. Cannot be the page itself or a page
        /// inside it.
        new_parent: Option<PageId>,
        /// The page's final position among its new siblings, counted without the page itself:
        /// `0` makes it first and the sibling count makes it last, even within the same parent.
        index: usize,
    },
}

/// What applying a command produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// A page was created with this ID.
    PageCreated(PageId),
    /// The document changed; nothing new to report.
    Changed,
}
