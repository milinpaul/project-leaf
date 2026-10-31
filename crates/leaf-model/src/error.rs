//! Errors from the document model.

use leaf_core::{BlockId, ColumnId, OptionId, PageId};
use thiserror::Error;

/// Everything that can go wrong reading or changing Leaf documents.
///
/// Loro's two error types are wrapped rather than exposed piecemeal, so callers (store, editor)
/// handle one type.
#[derive(Debug, Error)]
pub enum ModelError {
    /// The page is not in this workspace (never existed, or permanently deleted).
    #[error("page {0} not found")]
    PageNotFound(PageId),

    /// A page node in the saved workspace has no usable identity, so it cannot be addressed.
    /// Loading fails rather than hiding the page, so no data is silently lost.
    #[error("invalid page node {node}: {reason}")]
    InvalidPage {
        /// Loro's internal node ID, for debugging only.
        node: String,
        /// What was wrong with it.
        reason: String,
    },

    /// Two page nodes claim the same `PageId`.
    #[error("page {0} appears twice in the workspace")]
    DuplicatePage(PageId),

    /// A page's metadata field is missing or has the wrong type.
    #[error("page {page}: field {field:?} {reason}")]
    InvalidPageMeta {
        /// The page whose metadata is wrong.
        page: PageId,
        /// The metadata key, for example `"title"`.
        field: &'static str,
        /// What was wrong with it.
        reason: String,
    },

    /// A position is past the end of a list of siblings.
    #[error("index {index} is past the end ({len} items)")]
    IndexOutOfRange {
        /// The position asked for.
        index: usize,
        /// How many items there are; `index` may be at most this.
        len: usize,
    },

    /// A page cannot be moved into itself or into a page nested inside it: that would cut the
    /// branch it sits on off from the tree.
    #[error("cannot move page {page} into {new_parent}, which is the page itself or inside it")]
    MoveIntoOwnSubtree {
        /// The page being moved.
        page: PageId,
        /// The parent that was asked for.
        new_parent: PageId,
    },

    /// The page is in the trash (or inside a trashed page), so it can't be trashed again, moved,
    /// or given new pages. Restore it first.
    #[error("page {0} is in the trash")]
    PageInTrash(PageId),

    /// Restore and permanent delete only apply to pages in the trash.
    #[error("page {0} is not in the trash")]
    NotInTrash(PageId),

    /// An icon was set to empty text; clearing an icon uses `None`.
    #[error("page {0}: icon cannot be empty (use None to clear it)")]
    EmptyIcon(PageId),

    /// A cover was set to empty text; clearing a cover uses `None`.
    #[error("page {0}: cover cannot be empty (use None to clear it)")]
    EmptyCover(PageId),

    /// The block is not on this page (never existed, or deleted).
    #[error("block {0} not found")]
    BlockNotFound(BlockId),

    /// A block node in the saved page has no usable identity. Loading fails rather than hiding it.
    #[error("invalid block node {node}: {reason}")]
    InvalidBlock {
        /// Loro's internal node ID, for debugging only.
        node: String,
        /// What was wrong with it.
        reason: String,
    },

    /// Two block nodes claim the same `BlockId`.
    #[error("block {0} appears twice on the page")]
    DuplicateBlock(BlockId),

    /// A block's type or properties are missing or have the wrong type.
    #[error("block {block}: {field:?} {reason}")]
    InvalidBlockData {
        /// The block whose data is wrong.
        block: BlockId,
        /// The meta or props key, for example `"level"`.
        field: &'static str,
        /// What was wrong with it.
        reason: String,
    },

    /// The block kind can't be created as given (unknown type, or a heading level outside 1 to 3).
    #[error("invalid block kind: {0}")]
    InvalidKind(String),

    /// This kind of block has no text (dividers), so it can't be given any.
    #[error("{kind} blocks have no text")]
    TextNotAllowed {
        /// The block type.
        kind: String,
    },

    /// This kind of block can't contain other blocks (headings, code, dividers, images, unknown).
    #[error("block {parent} is a {kind}, which can't contain other blocks")]
    ChildrenNotAllowed {
        /// The block asked to be the parent.
        parent: BlockId,
        /// Its type.
        kind: String,
    },

    /// A block can't be moved into itself or into a block nested inside it.
    #[error("cannot move block {block} into {new_parent}, which is the block itself or inside it")]
    BlockMoveIntoOwnSubtree {
        /// The moved block whose subtree contains the destination.
        block: BlockId,
        /// The parent that was asked for.
        new_parent: BlockId,
    },

    /// A text position or range reaches past the end of the block's text.
    #[error("text position {end} is past the end of the text ({len} characters)")]
    TextOutOfRange {
        /// The furthest position asked for.
        end: usize,
        /// The text's length in Unicode scalar values.
        len: usize,
    },

    /// This kind of block's text takes no marks (code blocks).
    #[error("{kind} blocks take no formatting")]
    MarksNotAllowed {
        /// The block type.
        kind: String,
    },

    /// A due date that isn't a real `YYYY-MM-DD` calendar date (E19).
    #[error("not a date: {0:?}")]
    InvalidDate(String),

    /// A due time that isn't a real `HH:MM` time of day (E19).
    #[error("not a time of day: {0:?}")]
    InvalidTime(String),

    /// An action item can't have a due time without a due date (E19).
    #[error("a due time needs a due date")]
    TimeWithoutDate,

    /// A web bookmark's address isn't one it can hold (not `http`/`https`, no host, spaces, a password).
    #[error("not a web address: {0}")]
    InvalidWebAddress(String),

    /// A link needs an address.
    #[error("a link needs an address")]
    EmptyLink,

    /// Plain text can't hold the character page links occupy: a bare one would be a link to nothing.
    #[error("text can't contain U+FFFC, the character a page link occupies")]
    PageLinkCharInText,

    /// The page-link mark only re-points existing page links; text can't become one.
    #[error("only page links can point at a page")]
    PageLinkOnText,

    /// A page link can't lose its target and stay; it goes by deleting it.
    #[error("a page link is removed by deleting it")]
    PageLinkNotRemovable,

    /// A property was set on a kind of block it doesn't belong to (a checkbox on a heading).
    #[error("{kind} blocks have no {prop}")]
    PropNotForKind {
        /// The property's name.
        prop: &'static str,
        /// The block type.
        kind: String,
    },

    /// Backspace at the start of a block can't merge into the block above because it has no text
    /// (a divider, image or unknown block). The editor selects that block instead.
    #[error("block {target} has no text to merge into")]
    MergeTargetNotText {
        /// The block above, which the editor should select.
        target: BlockId,
    },

    /// Rows and cells only change through the table commands (and cells' text through the text
    /// commands), so a table always stays a whole grid: moving, deleting, duplicating, converting,
    /// splitting or merging a row or cell on its own is refused.
    #[error("block {block} is part of a table; use the table commands")]
    TablePart {
        /// The row or cell.
        block: BlockId,
    },

    /// The block is not a table, so the table commands don't apply to it.
    #[error("block {0} is not a table")]
    NotATable(BlockId),

    /// A table needs at least one row and one column: making one empty, or removing its last row or
    /// column, is refused (delete the table instead).
    #[error("a table needs at least one row and one column")]
    EmptyTable,

    /// The column is not in this database (never existed, or deleted).
    #[error("column {0} not found")]
    ColumnNotFound(ColumnId),

    /// The row is not in this database.
    #[error("row {0} not found in this database")]
    RowNotFound(PageId),

    /// The page is already a row of this database: a page is one row, once.
    #[error("page {0} is already a row of this database")]
    DuplicateRow(PageId),

    /// The option is not one of the column's select options.
    #[error("column {column} has no option {option}")]
    OptionNotFound {
        /// The select or multi-select column.
        column: ColumnId,
        /// The option asked for.
        option: OptionId,
    },

    /// A column or row node in a saved database has no usable identity, or two claim the same one.
    /// Loading fails rather than hiding it, so no data is silently lost.
    #[error("invalid database {part} node {node}: {reason}")]
    InvalidDatabaseNode {
        /// `"column"` or `"row"`.
        part: &'static str,
        /// Loro's internal node ID, for debugging only.
        node: String,
        /// What was wrong with it.
        reason: String,
    },

    /// The column's type comes from a newer version of Leaf, so this version can't change its cells or
    /// its type without losing what that version stored (ADR-066).
    #[error("column {column} has a type this version doesn't know ({type_name}); it is read-only")]
    UnknownColumnType {
        /// The column.
        column: ColumnId,
        /// The stored type name.
        type_name: String,
    },

    /// Columns can only be created with a type this version knows.
    #[error("unknown column type {0:?}")]
    UnknownPropertyType(String),

    /// The value doesn't fit the column's type (a number into a checkbox column, say).
    #[error("column {column} holds {expected} values")]
    CellTypeMismatch {
        /// The column.
        column: ColumnId,
        /// The column's type name.
        expected: String,
    },

    /// The page is not a database page, so it can't have rows.
    #[error("page {0} is not a database")]
    NotADatabasePage(PageId),

    /// The column doesn't have select options (it isn't a select or multi-select column).
    #[error("column {0} has no options")]
    NotASelectColumn(ColumnId),

    /// Loro rejected an operation or could not read the bytes.
    #[error(transparent)]
    Loro(#[from] loro::LoroError),

    /// Loro could not export the document.
    #[error(transparent)]
    Export(#[from] loro::LoroEncodeError),
}
