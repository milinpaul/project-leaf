//! Commands: the only way to change a document.
//!
//! The editor turns key presses and clicks into commands; the model applies each one as exactly one
//! Loro commit, which later makes it exactly one undo step. Keeping every change in an enum means
//! each one can be tested, logged and replayed on its own.

use leaf_core::PageId;

/// A change to the workspace doc: the page tree and each page's title, icon, cover, width and trash state.
///
/// Applied with [`Workspace::apply`](crate::Workspace::apply); undone with [`Workspace::undo`](crate::Workspace::undo).
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
    /// Sets or clears a page's cover.
    SetPageCover {
        /// Page to change.
        page: PageId,
        /// The cover's name (for example `builtin:dawn`, chosen in the cover picker), or `None` to remove
        /// it. An empty string is rejected: use `None` to clear.
        cover: Option<String>,
    },
    /// Switches a page between the reading column and the page area's whole width (E18-S06). It
    /// changes how the page is laid out, not what it says, so it doesn't count as an edit.
    SetPageFullWidth {
        /// Page to change.
        page: PageId,
        /// `true` for the whole width, `false` for the reading column.
        full_width: bool,
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
    /// Moves a page, and everything inside it, to the trash. It stays where it is, hidden from
    /// listings, so restoring puts it back in place.
    TrashPage {
        /// Page to trash. Cannot already be in the trash.
        page: PageId,
        /// When it was trashed, in Unix milliseconds, supplied by the caller.
        at: i64,
    },
    /// Records that a page's blocks, title, icon or cover just changed (ADR 0059). The editor applies
    /// it after the change itself, as a separate commit that undo skips: undoing an edit is an edit
    /// too, so it must never bring back an older time.
    MarkEdited {
        /// Page that changed.
        page: PageId,
        /// When, in Unix milliseconds, supplied by the caller.
        at: i64,
    },
    /// Takes a page out of the trash. If the page it lived in is itself in the trash, it goes to the
    /// end of the top level instead, so it doesn't stay hidden.
    RestorePage {
        /// A page that was trashed on its own (listed by `Workspace::trash`).
        page: PageId,
    },
    /// Stars a page: adds it to the end of the favourites (ADR 0060). Already a favourite: nothing
    /// changes. A page in the trash can't be starred.
    AddFavourite {
        /// Page to star.
        page: PageId,
    },
    /// Takes a page out of the favourites. Not a favourite: nothing changes.
    RemoveFavourite {
        /// Page to unstar.
        page: PageId,
    },
    /// Deletes a page in the trash, and everything inside it, for good. Reports every deleted page in
    /// [`Outcome::PagesDeleted`] so storage can drop their documents.
    DeletePagePermanently {
        /// A page in the trash, directly or inside a trashed page.
        page: PageId,
    },
}

/// What applying a command produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A page was created with this ID.
    PageCreated(PageId),
    /// The document changed, as exactly one undo step; nothing new to report.
    Changed,
    /// The command was valid but would change nothing, so nothing was written and there is no undo
    /// step: for example, renaming a page to its current title.
    Unchanged,
    /// These pages were deleted for good: the page asked for and everything inside it, parents first.
    PagesDeleted(Vec<PageId>),
}
