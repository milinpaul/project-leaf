//! The Leaf document model: Loro documents, the block schema, commands, undo and invariants.
//!
//! This is the only crate that writes to Loro. Every change to a page, the page tree or a database
//! (Phase 1, ADR-066) is a `Command` defined here, and each command is one Loro commit and one undo
//! step. Funnelling all writes through one place is what lets us test every edit, check invariants
//! after it, and later sync documents between devices without surprises.
//!
//! **Boundaries:** core crate, wasm-ok. No GPUI, no `leaf-ui`, no file system, threads or SQLite.
//! Depends only on `leaf-core`. Unknown data from newer app versions is preserved, never dropped.

mod block_kind;
mod callout;
mod command;
mod database;
mod error;
mod page_command;
mod page_doc;
mod page_meta;
mod saved;
mod task;
mod text;
mod workspace;

pub use block_kind::{BlockKind, BlockProp};
pub use callout::CalloutTone;
pub use command::{Outcome, WorkspaceCommand};
pub use database::{
    CellValue, Column, Database, DatabaseChanges, DatabaseCommand, DatabaseOutcome, Extra, Filter, FilterOp,
    PropertyType, SelectOption, Sort,
};
pub use error::ModelError;
pub use page_command::{PageCommand, PageOutcome};
pub use page_doc::{Anchor, Caret, Changes, History, InvariantViolation, PageDoc, SavedSelection};
pub use page_meta::PageMeta;
pub use saved::fold_saved;
pub use task::{DueDate, DueTime, Priority};
pub use text::{Mark, MarkKind, Marks, PAGE_LINK, TextSpan};
pub use workspace::{DEFAULT_UNDO_MERGE_MS, Workspace};
